//! Configuration module for SurrealDB Sink Connector
//!
//! This module handles all configuration aspects including:
//! - SurrealDB connection settings (URL, namespace, database, credentials)
//! - Topic-to-table mappings with per-table configurations
//! - Batch processing and performance tuning
//! - Environment variable overrides

use danube_connect_core::{ConnectorConfig, ConnectorError, ConnectorResult, SubscriptionType};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;

/// Storage mode for SurrealDB records
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum StorageMode {
    /// Store as regular documents (default)
    Document,
    /// Store as time-series data with timestamp optimization
    TimeSeries,
}

/// Complete configuration for the SurrealDB Sink Connector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurrealDBSinkConfig {
    /// Core connector configuration (Danube connection, etc.)
    #[serde(flatten)]
    pub core: ConnectorConfig,

    /// SurrealDB-specific configuration
    pub surrealdb: SurrealDBConfig,
}

/// SurrealDB-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurrealDBConfig {
    /// SurrealDB connection URL (e.g., "ws://localhost:8000", "http://localhost:8000")
    pub url: String,

    /// SurrealDB namespace
    pub namespace: String,

    /// SurrealDB database
    pub database: String,

    /// Optional username for authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,

    /// Optional password for authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    /// Connection timeout in seconds
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout_secs: u64,

    /// Request timeout in seconds
    #[serde(default = "default_request_timeout")]
    pub request_timeout_secs: u64,

    /// Topic mappings: Danube topics → SurrealDB tables
    #[serde(default)]
    pub topic_mappings: Vec<TopicMapping>,

    /// Global batch size (can be overridden per topic)
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Global flush interval in milliseconds
    #[serde(default = "default_flush_interval_ms")]
    pub flush_interval_ms: u64,
}

/// Mapping from a Danube topic to a SurrealDB table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicMapping {
    /// Danube topic to consume from
    pub topic: String,

    /// Danube subscription name
    pub subscription: String,

    /// Subscription type: Exclusive, Shared, FailOver
    #[serde(default = "default_subscription_type")]
    pub subscription_type: SubscriptionType,

    /// SurrealDB table name to insert into
    pub table_name: String,

    /// Include Danube metadata in records (topic, offset, timestamp)
    #[serde(default = "default_include_metadata")]
    pub include_danube_metadata: bool,

    /// Expected schema subject for validation (optional)
    /// If set, the runtime validates and deserializes messages automatically
    /// Schema must be registered in Danube Schema Registry
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_schema_subject: Option<String>,

    /// Custom batch size for this topic (overrides global)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_size: Option<usize>,

    /// Custom flush interval for this topic (overrides global)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flush_interval_ms: Option<u64>,

    /// Storage mode: Document or TimeSeries
    #[serde(default)]
    pub storage_mode: StorageMode,
}

// Default value functions
fn default_connection_timeout() -> u64 {
    30
}

fn default_request_timeout() -> u64 {
    30
}

fn default_batch_size() -> usize {
    100
}

fn default_flush_interval_ms() -> u64 {
    1000
}

fn default_include_metadata() -> bool {
    true
}

fn default_subscription_type() -> SubscriptionType {
    SubscriptionType::Shared
}

impl Default for StorageMode {
    fn default() -> Self {
        StorageMode::Document
    }
}

impl SurrealDBSinkConfig {
    /// Load configuration from TOML file
    ///
    /// The config file path must be specified via CONNECTOR_CONFIG_PATH environment variable.
    /// Environment variables can override secrets (username, password) and URLs.
    pub fn load() -> ConnectorResult<Self> {
        let config_path = env::var("CONNECTOR_CONFIG_PATH")
            .map_err(|_| ConnectorError::config(
                "CONNECTOR_CONFIG_PATH environment variable must be set to the path of the TOML configuration file"
            ))?;

        Self::from_file(&config_path)
    }

    /// Load configuration from a TOML file
    pub fn from_file(path: &str) -> ConnectorResult<Self> {
        let contents = fs::read_to_string(path).map_err(|e| {
            ConnectorError::config(format!("Failed to read config file '{}': {}", path, e))
        })?;

        let mut config: Self = toml::from_str(&contents)
            .map_err(|e| ConnectorError::config(format!("Failed to parse TOML config: {}", e)))?;

        // Apply environment variable overrides
        config.apply_env_overrides()?;

        Ok(config)
    }

    /// Apply environment variable overrides for secrets and connection details
    ///
    /// Only overrides sensitive data that shouldn't be in config files:
    /// - Credentials (username, password)
    /// - Connection URLs (for different environments)
    /// - Connector name (for different deployments)
    fn apply_env_overrides(&mut self) -> ConnectorResult<()> {
        // Override core Danube settings (mandatory fields from danube-connect-core)
        if let Ok(danube_url) = env::var("DANUBE_SERVICE_URL") {
            self.core.danube_service_url = danube_url;
        }

        if let Ok(connector_name) = env::var("CONNECTOR_NAME") {
            self.core.connector_name = connector_name;
        }

        // Override SurrealDB connection URL (for different environments: dev/staging/prod)
        if let Ok(url) = env::var("SURREALDB_URL") {
            self.surrealdb.url = url;
        }

        // Override credentials (secrets should not be in config files)
        if let Ok(username) = env::var("SURREALDB_USERNAME") {
            self.surrealdb.username = Some(username);
        }
        if let Ok(password) = env::var("SURREALDB_PASSWORD") {
            self.surrealdb.password = Some(password);
        }

        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> ConnectorResult<()> {
        // Validate SurrealDB URL
        if self.surrealdb.url.is_empty() {
            return Err(ConnectorError::config("SURREALDB_URL cannot be empty"));
        }

        // Validate namespace and database
        if self.surrealdb.namespace.is_empty() {
            return Err(ConnectorError::config(
                "SURREALDB_NAMESPACE cannot be empty",
            ));
        }
        if self.surrealdb.database.is_empty() {
            return Err(ConnectorError::config("SURREALDB_DATABASE cannot be empty"));
        }

        // Validate topic mappings
        if self.surrealdb.topic_mappings.is_empty() {
            return Err(ConnectorError::config(
                "At least one topic mapping is required",
            ));
        }

        for mapping in &self.surrealdb.topic_mappings {
            if mapping.topic.is_empty() {
                return Err(ConnectorError::config("Topic name cannot be empty"));
            }
            if mapping.subscription.is_empty() {
                return Err(ConnectorError::config("Subscription name cannot be empty"));
            }
            if mapping.table_name.is_empty() {
                return Err(ConnectorError::config("Table name cannot be empty"));
            }
            // storage_mode is an enum with default, so it's always valid
            // Just verify it's one of the expected values (Document or TimeSeries)
            match mapping.storage_mode {
                StorageMode::Document | StorageMode::TimeSeries => {}
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let mut config = SurrealDBSinkConfig {
            core: ConnectorConfig {
                connector_name: "test".to_string(),
                danube_service_url: "http://localhost:6650".to_string(),
                retry: Default::default(),
                processing: Default::default(),
                schemas: Vec::new(),
            },
            surrealdb: SurrealDBConfig {
                url: "ws://localhost:8000".to_string(),
                namespace: "test".to_string(),
                database: "test".to_string(),
                username: None,
                password: None,
                connection_timeout_secs: 30,
                request_timeout_secs: 30,
                topic_mappings: vec![TopicMapping {
                    topic: "/test/topic".to_string(),
                    subscription: "test-sub".to_string(),
                    subscription_type: SubscriptionType::Shared,
                    table_name: "events".to_string(),
                    include_danube_metadata: true,
                    expected_schema_subject: None,
                    batch_size: None,
                    flush_interval_ms: None,
                    storage_mode: StorageMode::Document,
                }],
                batch_size: 100,
                flush_interval_ms: 1000,
            },
        };

        assert!(config.validate().is_ok());

        // Test empty URL
        config.surrealdb.url = "".to_string();
        assert!(config.validate().is_err());
        config.surrealdb.url = "ws://localhost:8000".to_string();

        // Test empty topic mappings
        config.surrealdb.topic_mappings.clear();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_storage_mode_validation() {
        let config = SurrealDBSinkConfig {
            core: ConnectorConfig {
                connector_name: "test".to_string(),
                danube_service_url: "http://localhost:6650".to_string(),
                retry: Default::default(),
                processing: Default::default(),
                schemas: Vec::new(),
            },
            surrealdb: SurrealDBConfig {
                url: "ws://localhost:8000".to_string(),
                namespace: "test".to_string(),
                database: "test".to_string(),
                username: None,
                password: None,
                connection_timeout_secs: 30,
                request_timeout_secs: 30,
                topic_mappings: vec![
                    TopicMapping {
                        topic: "/test/document".to_string(),
                        subscription: "test-doc".to_string(),
                        subscription_type: SubscriptionType::Shared,
                        table_name: "documents".to_string(),
                        include_danube_metadata: true,
                        expected_schema_subject: None,
                        batch_size: None,
                        flush_interval_ms: None,
                        storage_mode: StorageMode::Document,
                    },
                    TopicMapping {
                        topic: "/test/timeseries".to_string(),
                        subscription: "test-ts".to_string(),
                        subscription_type: SubscriptionType::Shared,
                        table_name: "timeseries".to_string(),
                        include_danube_metadata: true,
                        expected_schema_subject: None,
                        batch_size: None,
                        flush_interval_ms: None,
                        storage_mode: StorageMode::TimeSeries,
                    },
                ],
                batch_size: 100,
                flush_interval_ms: 1000,
            },
        };

        // Both storage modes should validate successfully
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_default_values() {
        assert_eq!(default_batch_size(), 100);
        assert_eq!(default_flush_interval_ms(), 1000);
        assert_eq!(default_connection_timeout(), 30);
        assert_eq!(default_request_timeout(), 30);
        assert!(default_include_metadata());
        assert_eq!(StorageMode::default(), StorageMode::Document);
    }
}
