//! Configuration module for Delta Lake Sink Connector
//!
//! This module handles all configuration aspects including:
//! - Cloud provider selection (S3, Azure, or GCS)
//! - Delta Lake table schemas (user-defined)
//! - Topic-to-table mappings with per-table configurations
//! - Batch processing and performance tuning
//! - Environment variable overrides

use danube_connect_core::{ConnectorConfig, ConnectorError, ConnectorResult, SchemaType};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;

/// Cloud storage backend for Delta Lake
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StorageBackend {
    /// Amazon S3 (or S3-compatible like MinIO)
    S3,
    /// Azure Blob Storage
    Azure,
    /// Google Cloud Storage
    GCS,
}

/// Write mode for Delta Lake operations
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WriteMode {
    /// Append new data to existing table (default)
    Append,
    /// Overwrite existing table data
    Overwrite,
}

impl Default for WriteMode {
    fn default() -> Self {
        WriteMode::Append
    }
}

/// Delta Lake table schema field definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaField {
    /// Field name
    pub name: String,
    
    /// Arrow data type (e.g., "Utf8", "Int64", "Float64", "Boolean", "Timestamp")
    pub data_type: String,
    
    /// Whether the field is nullable
    #[serde(default = "default_true")]
    pub nullable: bool,
}

/// Complete configuration for the Delta Lake Sink Connector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaLakeSinkConfig {
    /// Core connector configuration (Danube connection, etc.)
    #[serde(flatten)]
    pub core: ConnectorConfig,

    /// Delta Lake-specific configuration
    pub deltalake: DeltaLakeConfig,
}

/// Delta Lake-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaLakeConfig {
    /// Storage backend (s3, azure, or gcs)
    pub storage_backend: StorageBackend,

    /// AWS S3 region (required if storage_backend = "s3")
    /// Credentials from environment: AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY
    #[serde(skip_serializing_if = "Option::is_none")]
    pub s3_region: Option<String>,

    /// S3 endpoint URL (optional, for MinIO or custom S3-compatible storage)
    /// Example: "http://localhost:9000"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub s3_endpoint: Option<String>,

    /// Allow HTTP for S3 (useful for MinIO local testing)
    #[serde(default)]
    pub s3_allow_http: bool,

    /// Azure storage account name (required if storage_backend = "azure")
    /// Credentials from environment: AZURE_STORAGE_ACCOUNT_KEY or AZURE_STORAGE_SAS_TOKEN
    #[serde(skip_serializing_if = "Option::is_none")]
    pub azure_storage_account: Option<String>,

    /// Azure container name (required if storage_backend = "azure")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub azure_container: Option<String>,

    /// GCP project ID (required if storage_backend = "gcs")
    /// Credentials from environment: GOOGLE_APPLICATION_CREDENTIALS
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gcp_project_id: Option<String>,

    /// Topic mappings: Danube topics → Delta Lake tables
    #[serde(default)]
    pub topic_mappings: Vec<TopicMapping>,

    /// Global batch size (can be overridden per topic)
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Global flush interval in milliseconds
    #[serde(default = "default_flush_interval_ms")]
    pub flush_interval_ms: u64,
}

/// Mapping from a Danube topic to a Delta Lake table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicMapping {
    /// Danube topic to consume from
    pub topic: String,

    /// Subscription name for this consumer
    pub subscription: String,

    /// Delta Lake table path (e.g., "s3://bucket/path/to/table")
    pub delta_table_path: String,

    /// Schema type for messages in this topic
    #[serde(default)]
    pub schema_type: SchemaType,

    /// Table schema definition (user-defined)
    pub schema: Vec<SchemaField>,

    /// Write mode (append or overwrite)
    #[serde(default)]
    pub write_mode: WriteMode,

    /// Include Danube metadata as a JSON column (_danube_metadata)
    #[serde(default)]
    pub include_danube_metadata: bool,

    /// Batch size for this specific topic (overrides global)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_size: Option<usize>,

    /// Flush interval for this specific topic (overrides global)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flush_interval_ms: Option<u64>,
}

impl TopicMapping {
    /// Get effective batch size (topic-specific or global)
    pub fn effective_batch_size(&self, global: usize) -> usize {
        self.batch_size.unwrap_or(global)
    }

    /// Get effective flush interval (topic-specific or global)
    pub fn effective_flush_interval_ms(&self, global: u64) -> u64 {
        self.flush_interval_ms.unwrap_or(global)
    }
}

// Default values
fn default_batch_size() -> usize {
    1000
}

fn default_flush_interval_ms() -> u64 {
    5000 // 5 seconds
}

fn default_true() -> bool {
    true
}

impl DeltaLakeSinkConfig {
    /// Load configuration from TOML file
    pub fn from_file(path: &str) -> ConnectorResult<Self> {
        let contents = fs::read_to_string(path).map_err(|e| {
            ConnectorError::config(format!("Failed to read config file '{}': {}", path, e))
        })?;

        let config: Self = toml::from_str(&contents).map_err(|e| {
            ConnectorError::config(format!("Failed to parse config file '{}': {}", path, e))
        })?;

        config.validate()?;
        Ok(config)
    }

    /// Load configuration from environment variable CONNECTOR_CONFIG_PATH
    pub fn load() -> ConnectorResult<Self> {
        let config_path = env::var("CONNECTOR_CONFIG_PATH").map_err(|_| {
            ConnectorError::config(
                "CONNECTOR_CONFIG_PATH environment variable not set. \
                 Please set it to the path of your connector.toml file.",
            )
        })?;

        let mut config = Self::from_file(&config_path)?;
        config.apply_env_overrides();
        Ok(config)
    }

    /// Apply environment variable overrides
    fn apply_env_overrides(&mut self) {
        // Core overrides (mandatory)
        if let Ok(url) = env::var("DANUBE_SERVICE_URL") {
            tracing::info!("Overriding danube_service_url from environment");
            self.core.danube_service_url = url;
        }

        if let Ok(name) = env::var("CONNECTOR_NAME") {
            tracing::info!("Overriding connector_name from environment");
            self.core.connector_name = name;
        }

        // Cloud-specific overrides (credentials should come from env)
        if let Ok(region) = env::var("AWS_REGION") {
            self.deltalake.s3_region = Some(region);
        }

        if let Ok(endpoint) = env::var("S3_ENDPOINT") {
            self.deltalake.s3_endpoint = Some(endpoint);
        }

        if let Ok(account) = env::var("AZURE_STORAGE_ACCOUNT") {
            self.deltalake.azure_storage_account = Some(account);
        }

        if let Ok(project) = env::var("GCP_PROJECT_ID") {
            self.deltalake.gcp_project_id = Some(project);
        }
    }

    /// Validate configuration
    fn validate(&self) -> ConnectorResult<()> {
        // Validate topic mappings
        if self.deltalake.topic_mappings.is_empty() {
            return Err(ConnectorError::config(
                "No topic mappings configured. Please add at least one [[deltalake.topic_mappings]] entry.",
            ));
        }

        // Validate cloud provider configuration
        match self.deltalake.storage_backend {
            StorageBackend::S3 => {
                if self.deltalake.s3_region.is_none() {
                    return Err(ConnectorError::config(
                        "s3_region is required when storage_backend = 's3'",
                    ));
                }
            }
            StorageBackend::Azure => {
                if self.deltalake.azure_storage_account.is_none() {
                    return Err(ConnectorError::config(
                        "azure_storage_account is required when storage_backend = 'azure'",
                    ));
                }
                if self.deltalake.azure_container.is_none() {
                    return Err(ConnectorError::config(
                        "azure_container is required when storage_backend = 'azure'",
                    ));
                }
            }
            StorageBackend::GCS => {
                if self.deltalake.gcp_project_id.is_none() {
                    return Err(ConnectorError::config(
                        "gcp_project_id is required when storage_backend = 'gcs'",
                    ));
                }
            }
        }

        // Validate each topic mapping
        for mapping in &self.deltalake.topic_mappings {
            if mapping.topic.is_empty() {
                return Err(ConnectorError::config("Topic cannot be empty"));
            }
            if mapping.subscription.is_empty() {
                return Err(ConnectorError::config("Subscription cannot be empty"));
            }
            if mapping.delta_table_path.is_empty() {
                return Err(ConnectorError::config("Delta table path cannot be empty"));
            }
            if mapping.schema.is_empty() {
                return Err(ConnectorError::config(format!(
                    "Schema cannot be empty for topic '{}'. Please define at least one field.",
                    mapping.topic
                )));
            }

            // Validate schema field types
            for field in &mapping.schema {
                validate_arrow_type(&field.data_type)?;
            }
        }

        Ok(())
    }
}

/// Validate Arrow data type string
fn validate_arrow_type(data_type: &str) -> ConnectorResult<()> {
    let valid_types = [
        "Utf8",
        "Int8",
        "Int16",
        "Int32",
        "Int64",
        "UInt8",
        "UInt16",
        "UInt32",
        "UInt64",
        "Float32",
        "Float64",
        "Boolean",
        "Timestamp",
        "Date32",
        "Date64",
        "Binary",
    ];

    if !valid_types.contains(&data_type) {
        return Err(ConnectorError::config(format!(
            "Invalid Arrow data type '{}'. Valid types: {}",
            data_type,
            valid_types.join(", ")
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_arrow_type() {
        assert!(validate_arrow_type("Utf8").is_ok());
        assert!(validate_arrow_type("Int64").is_ok());
        assert!(validate_arrow_type("Float64").is_ok());
        assert!(validate_arrow_type("Boolean").is_ok());
        assert!(validate_arrow_type("Timestamp").is_ok());
        assert!(validate_arrow_type("InvalidType").is_err());
    }
}
