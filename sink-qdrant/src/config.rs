//! Configuration for the Qdrant Sink Connector

use danube_connect_core::{
    ConfigEnvOverrides, ConfigValidate, ConnectorConfig, ConnectorConfigLoader, ConnectorResult,
    SubscriptionType,
};
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
        ConnectorConfigLoader::new().load()
    }

    /// Validate all configuration
    pub fn validate(&self) -> ConnectorResult<()> {
        self.validate_config()
    }
}

impl ConfigEnvOverrides for QdrantSinkConfig {
    fn apply_env_overrides(&mut self) -> ConnectorResult<()> {
        if let Ok(danube_url) = env::var("DANUBE_SERVICE_URL") {
            self.core.danube_service_url = danube_url;
        }

        if let Ok(connector_name) = env::var("CONNECTOR_NAME") {
            self.core.connector_name = connector_name;
        }

        if let Ok(url) = env::var("QDRANT_URL") {
            self.qdrant.url = url;
        }

        if let Ok(api_key) = env::var("QDRANT_API_KEY") {
            self.qdrant.api_key = Some(api_key);
        }

        Ok(())
    }
}

impl ConfigValidate for QdrantSinkConfig {
    fn validate_config(&self) -> ConnectorResult<()> {
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

    /// Routes: Danube topic → Qdrant collection configuration
    pub routes: Vec<TopicMapping>,

    /// Timeout for Qdrant operations in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

/// Topic mapping configuration: Danube topic → Qdrant collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicMapping {
    /// Danube topic to consume from (format: /{namespace}/{topic_name})
    pub from: String,

    /// Subscription name for this topic
    pub subscription: String,

    /// Subscription type (default: Exclusive)
    #[serde(default = "default_subscription_type")]
    pub subscription_type: SubscriptionType,

    /// Target Qdrant collection name
    pub to: String,

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
}

fn default_distance() -> Distance {
    Distance::Cosine
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

        if self.routes.is_empty() {
            return Err(danube_connect_core::ConnectorError::config(
                "At least one route is required",
            ));
        }

        // Validate each topic mapping
        for (idx, mapping) in self.routes.iter().enumerate() {
            if mapping.from.is_empty() {
                return Err(danube_connect_core::ConnectorError::config(format!(
                    "Route {} has empty 'from'",
                    idx
                )));
            }

            if mapping.to.is_empty() {
                return Err(danube_connect_core::ConnectorError::config(format!(
                    "Route {} has empty 'to'",
                    idx
                )));
            }

            if mapping.vector_dimension == 0 {
                return Err(danube_connect_core::ConnectorError::config(format!(
                    "Topic mapping {} has zero vector dimension",
                    idx
                )));
            }

            if mapping.subscription.is_empty() {
                return Err(danube_connect_core::ConnectorError::config(format!(
                    "Topic mapping {} has empty subscription",
                    idx
                )));
            }
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
            routes: vec![TopicMapping {
                from: "/default/vectors".to_string(),
                subscription: "qdrant-sink-sub".to_string(),
                subscription_type: SubscriptionType::Exclusive,
                to: "test_collection".to_string(),
                vector_dimension: 1536,
                distance: Distance::Cosine,
                auto_create_collection: true,
                include_danube_metadata: true,
                expected_schema_subject: None,
            }],
            timeout_secs: 30,
        };

        assert!(config.validate().is_ok());

        // Test empty URL
        config.url = "".to_string();
        assert!(config.validate().is_err());

        // Test empty topic mappings
        config.url = "http://localhost:6334".to_string();
        config.routes = vec![];
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
