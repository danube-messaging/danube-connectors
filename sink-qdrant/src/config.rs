//! Configuration for the Qdrant Sink Connector

use danube_connect_core::{ConnectorConfig, ConnectorResult, SubscriptionType};
use serde::{Deserialize, Serialize};
use std::env;

/// Unified configuration for Qdrant Sink Connector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QdrantSinkConfig {
    /// Core Danube Connect configuration (flattened at root level)
    #[serde(flatten)]
    pub core: ConnectorConfig,

    /// Qdrant-specific configuration
    pub qdrant: QdrantConfig,
}

impl QdrantSinkConfig {
    /// Load configuration from TOML file
    /// 
    /// The config file path must be specified via CONNECTOR_CONFIG_PATH environment variable.
    /// Environment variables can override secrets (API key) and URLs.
    pub fn load() -> ConnectorResult<Self> {
        let config_path = env::var("CONNECTOR_CONFIG_PATH")
            .map_err(|_| danube_connect_core::ConnectorError::config(
                "CONNECTOR_CONFIG_PATH environment variable must be set to the path of the TOML configuration file"
            ))?;
        
        Self::from_file(&config_path)
    }

    /// Load configuration from a TOML file
    pub fn from_file(path: &str) -> ConnectorResult<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            danube_connect_core::ConnectorError::config(format!(
                "Failed to read config file {}: {}",
                path, e
            ))
        })?;

        let mut config: Self = toml::from_str(&content).map_err(|e| {
            danube_connect_core::ConnectorError::config(format!(
                "Failed to parse config file {}: {}",
                path, e
            ))
        })?;

        // Apply environment variable overrides for secrets and URLs
        config.apply_env_overrides();

        Ok(config)
    }

    /// Apply environment variable overrides for secrets and connection details
    /// 
    /// Only overrides sensitive data that shouldn't be in config files:
    /// - API key (secret)
    /// - Connection URLs (for different environments)
    /// - Connector name (for different deployments)
    fn apply_env_overrides(&mut self) {
        // Override core Danube settings (mandatory fields from danube-connect-core)
        if let Ok(danube_url) = env::var("DANUBE_SERVICE_URL") {
            self.core.danube_service_url = danube_url;
        }
        
        if let Ok(connector_name) = env::var("CONNECTOR_NAME") {
            self.core.connector_name = connector_name;
        }

        // Override Qdrant URL (for different environments: dev/staging/prod)
        if let Ok(url) = env::var("QDRANT_URL") {
            self.qdrant.url = url;
        }

        // Override API key (secret should not be in config files)
        if let Ok(api_key) = env::var("QDRANT_API_KEY") {
            self.qdrant.api_key = Some(api_key);
        }
    }

    /// Validate all configuration
    pub fn validate(&self) -> ConnectorResult<()> {
        // Core config validation is handled by danube-connect-core runtime
        self.qdrant.validate()?;
        Ok(())
    }
}

/// Qdrant connector configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QdrantConfig {
    /// Qdrant server URL (gRPC endpoint)
    pub url: String,

    /// Optional API key for Qdrant Cloud
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// Topic mappings: Danube topic → Qdrant collection configuration
    pub topic_mappings: Vec<TopicMapping>,

    /// Global batch size for bulk upserts (10-500)
    /// Can be overridden per topic
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Global batch timeout in milliseconds
    /// Can be overridden per topic
    #[serde(default = "default_batch_timeout")]
    pub batch_timeout_ms: u64,

    /// Timeout for Qdrant operations in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

/// Topic mapping configuration: Danube topic → Qdrant collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicMapping {
    /// Danube topic to consume from (format: /{namespace}/{topic_name})
    pub topic: String,

    /// Subscription name for this topic
    pub subscription: String,

    /// Subscription type (default: Exclusive)
    #[serde(default = "default_subscription_type")]
    pub subscription_type: SubscriptionType,

    /// Target Qdrant collection name
    pub collection_name: String,

    /// Vector dimension (must match embedding model for this topic)
    pub vector_dimension: usize,

    /// Distance metric for this collection
    #[serde(default = "default_distance")]
    pub distance: Distance,

    /// Automatically create collection if it doesn't exist
    #[serde(default = "default_auto_create")]
    pub auto_create_collection: bool,

    /// Include Danube metadata in payload for this topic
    #[serde(default = "default_include_metadata")]
    pub include_danube_metadata: bool,

    /// Expected schema subject for validation (optional)
    /// If set, the runtime validates and deserializes messages automatically
    /// Schema must be registered in Danube Schema Registry
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_schema_subject: Option<String>,

    /// Topic-specific batch size (overrides global)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_size: Option<usize>,

    /// Topic-specific batch timeout (overrides global)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_timeout_ms: Option<u64>,
}

impl TopicMapping {
    /// Get effective batch size (topic-specific or global)
    pub fn effective_batch_size(&self, global: usize) -> usize {
        self.batch_size.unwrap_or(global)
    }

    /// Get effective batch timeout (topic-specific or global)
    pub fn effective_batch_timeout(&self, global: u64) -> u64 {
        self.batch_timeout_ms.unwrap_or(global)
    }
}

fn default_distance() -> Distance {
    Distance::Cosine
}

fn default_batch_size() -> usize {
    100
}

fn default_batch_timeout() -> u64 {
    1000
}

fn default_auto_create() -> bool {
    true
}

fn default_include_metadata() -> bool {
    true
}

fn default_timeout() -> u64 {
    30
}

fn default_subscription_type() -> SubscriptionType {
    SubscriptionType::Exclusive
}

/// Distance metric for vector similarity
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Distance {
    /// Cosine similarity (most common for embeddings)
    Cosine,
    /// Euclidean distance
    Euclid,
    /// Dot product
    Dot,
    /// Manhattan distance
    Manhattan,
}

impl Distance {
    pub fn to_qdrant(&self) -> qdrant_client::qdrant::Distance {
        match self {
            Distance::Cosine => qdrant_client::qdrant::Distance::Cosine,
            Distance::Euclid => qdrant_client::qdrant::Distance::Euclid,
            Distance::Dot => qdrant_client::qdrant::Distance::Dot,
            Distance::Manhattan => qdrant_client::qdrant::Distance::Manhattan,
        }
    }
}

impl QdrantConfig {
    /// Validate the configuration
    pub fn validate(&self) -> ConnectorResult<()> {
        if self.url.is_empty() {
            return Err(danube_connect_core::ConnectorError::config(
                "Qdrant URL cannot be empty",
            ));
        }

        if self.topic_mappings.is_empty() {
            return Err(danube_connect_core::ConnectorError::config(
                "At least one topic mapping is required",
            ));
        }

        // Validate each topic mapping
        for (idx, mapping) in self.topic_mappings.iter().enumerate() {
            if mapping.topic.is_empty() {
                return Err(danube_connect_core::ConnectorError::config(
                    format!("Topic mapping {} has empty topic", idx),
                ));
            }

            if mapping.collection_name.is_empty() {
                return Err(danube_connect_core::ConnectorError::config(
                    format!("Topic mapping {} has empty collection name", idx),
                ));
            }

            if mapping.vector_dimension == 0 {
                return Err(danube_connect_core::ConnectorError::config(
                    format!("Topic mapping {} has zero vector dimension", idx),
                ));
            }

            if mapping.subscription.is_empty() {
                return Err(danube_connect_core::ConnectorError::config(
                    format!("Topic mapping {} has empty subscription", idx),
                ));
            }
        }

        if self.batch_size == 0 {
            return Err(danube_connect_core::ConnectorError::config(
                "Batch size must be greater than 0",
            ));
        }

        Ok(())
    }

    /// Create Qdrant client configuration
    pub fn qdrant_client_config(&self) -> qdrant_client::config::QdrantConfig {
        let mut builder = qdrant_client::config::QdrantConfig::from_url(&self.url);

        if let Some(ref api_key) = self.api_key {
            builder.set_api_key(api_key);
        }

        builder.set_timeout(std::time::Duration::from_secs(self.timeout_secs));

        builder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let mut config = QdrantConfig {
            url: "http://localhost:6334".to_string(),
            api_key: None,
            topic_mappings: vec![TopicMapping {
                topic: "/default/vectors".to_string(),
                subscription: "qdrant-sink-sub".to_string(),
                subscription_type: SubscriptionType::Exclusive,
                collection_name: "test_collection".to_string(),
                vector_dimension: 1536,
                distance: Distance::Cosine,
                auto_create_collection: true,
                include_danube_metadata: true,
                expected_schema_subject: None,
                batch_size: None,
                batch_timeout_ms: None,
            }],
            batch_size: 100,
            batch_timeout_ms: 1000,
            timeout_secs: 30,
        };

        assert!(config.validate().is_ok());

        // Test empty URL
        config.url = "".to_string();
        assert!(config.validate().is_err());

        // Test empty topic mappings
        config.url = "http://localhost:6334".to_string();
        config.topic_mappings = vec![];
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_distance_conversion() {
        assert_eq!(
            Distance::Cosine.to_qdrant(),
            qdrant_client::qdrant::Distance::Cosine
        );
        assert_eq!(
            Distance::Euclid.to_qdrant(),
            qdrant_client::qdrant::Distance::Euclid
        );
    }
}
