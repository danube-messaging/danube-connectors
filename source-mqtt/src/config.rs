//! Configuration for the MQTT Source Connector

use danube_connect_core::{ConnectorConfig, ConnectorResult};
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;

/// Unified configuration for MQTT Source Connector
///
/// This struct combines core Danube configuration with MQTT-specific settings
/// in a single, easy-to-use configuration file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttSourceConfig {
    /// Core Danube Connect configuration (flattened at root level)
    #[serde(flatten)]
    pub core: ConnectorConfig,

    /// MQTT-specific configuration
    pub mqtt: MqttConfig,
}

impl MqttSourceConfig {
    /// Load configuration from TOML file
    ///
    /// The config file path must be specified via CONNECTOR_CONFIG_PATH environment variable.
    /// Environment variables can override secrets (username, password) and connection URLs.
    ///
    /// # Example
    ///
    /// ```toml
    /// # connector.toml - Single file for everything
    /// danube_service_url = "http://broker:6650"
    /// connector_name = "mqtt-source"
    ///
    /// [mqtt]
    /// broker_host = "mosquitto"
    /// # ... mqtt settings
    /// ```
    pub fn load() -> ConnectorResult<Self> {
        let config_path = env::var("CONNECTOR_CONFIG_PATH")
            .map_err(|_| danube_connect_core::ConnectorError::config(
                "CONNECTOR_CONFIG_PATH environment variable must be set to the path of the TOML configuration file"
            ))?;
        
        let mut config = Self::from_file(&config_path)?;

        // Apply environment variable overrides for secrets and connection details
        config.apply_env_overrides();

        Ok(config)
    }

    /// Load configuration from a TOML file
    pub fn from_file(path: &str) -> ConnectorResult<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            danube_connect_core::ConnectorError::config(format!(
                "Failed to read config file {}: {}",
                path, e
            ))
        })?;

        toml::from_str(&content).map_err(|e| {
            danube_connect_core::ConnectorError::config(format!(
                "Failed to parse config file {}: {}",
                path, e
            ))
        })
    }

    /// Apply environment variable overrides for secrets and connection details
    /// 
    /// Only overrides sensitive data that shouldn't be in config files:
    /// - Credentials (username, password)
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

        // Override MQTT connection settings
        if let Ok(host) = env::var("MQTT_BROKER_HOST") {
            self.mqtt.broker_host = host;
        }
        
        if let Ok(port) = env::var("MQTT_BROKER_PORT") {
            if let Ok(p) = port.parse() {
                self.mqtt.broker_port = p;
            }
        }
        
        if let Ok(client_id) = env::var("MQTT_CLIENT_ID") {
            self.mqtt.client_id = client_id;
        }

        // Override credentials (secrets should not be in config files)
        if let Ok(username) = env::var("MQTT_USERNAME") {
            self.mqtt.username = Some(username);
        }
        
        if let Ok(password) = env::var("MQTT_PASSWORD") {
            self.mqtt.password = Some(password);
        }
        
        if let Ok(use_tls) = env::var("MQTT_USE_TLS") {
            if let Ok(b) = use_tls.parse() {
                self.mqtt.use_tls = b;
            }
        }
    }

    /// Validate all configuration
    pub fn validate(&self) -> ConnectorResult<()> {
        self.core.validate()?;
        self.mqtt.validate()?;
        Ok(())
    }
}

/// MQTT connector configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttConfig {
    /// MQTT broker host
    pub broker_host: String,

    /// MQTT broker port
    #[serde(default = "default_port")]
    pub broker_port: u16,

    /// Client ID for MQTT connection
    pub client_id: String,

    /// Username for authentication (optional)
    pub username: Option<String>,

    /// Password for authentication (optional)
    pub password: Option<String>,

    /// Enable TLS/SSL
    #[serde(default)]
    pub use_tls: bool,

    /// Keep alive interval in seconds
    #[serde(default = "default_keep_alive")]
    pub keep_alive_secs: u64,

    /// Connection timeout in seconds
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout_secs: u64,

    /// Maximum message size in bytes
    #[serde(default = "default_max_packet_size")]
    pub max_packet_size: usize,

    /// Topic mappings (MQTT topic -> Danube topic)
    pub topic_mappings: Vec<TopicMapping>,

    /// Clean session on connect
    #[serde(default = "default_true")]
    pub clean_session: bool,

    /// Add MQTT metadata as message attributes
    #[serde(default = "default_true")]
    pub include_metadata: bool,

    /// Enable TCP_NODELAY for reduced latency (disables Nagle's algorithm)
    /// Beneficial for real-time messaging scenarios
    #[serde(default = "default_true")]
    pub tcp_nodelay: bool,
}

fn default_port() -> u16 {
    1883
}

fn default_keep_alive() -> u64 {
    60
}

fn default_connection_timeout() -> u64 {
    30
}

fn default_max_packet_size() -> usize {
    10 * 1024 * 1024 // 10MB
}

fn default_true() -> bool {
    true
}

impl MqttConfig {

    /// Validate the configuration
    pub fn validate(&self) -> ConnectorResult<()> {
        if self.broker_host.is_empty() {
            return Err(danube_connect_core::ConnectorError::config(
                "broker_host cannot be empty",
            ));
        }

        if self.client_id.is_empty() {
            return Err(danube_connect_core::ConnectorError::config(
                "client_id cannot be empty",
            ));
        }

        if self.topic_mappings.is_empty() {
            return Err(danube_connect_core::ConnectorError::config(
                "At least one topic mapping is required",
            ));
        }

        for mapping in &self.topic_mappings {
            if mapping.mqtt_topic.is_empty() {
                return Err(danube_connect_core::ConnectorError::config(
                    "MQTT topic cannot be empty",
                ));
            }
            if mapping.danube_topic.is_empty() {
                return Err(danube_connect_core::ConnectorError::config(
                    "Danube topic cannot be empty",
                ));
            }
        }

        Ok(())
    }

    /// Get MQTT connection options
    pub fn mqtt_options(&self) -> rumqttc::MqttOptions {
        let mut options =
            rumqttc::MqttOptions::new(&self.client_id, &self.broker_host, self.broker_port);

        options.set_keep_alive(Duration::from_secs(self.keep_alive_secs));
        options.set_clean_session(self.clean_session);
        options.set_max_packet_size(self.max_packet_size, self.max_packet_size);

        if let (Some(username), Some(password)) = (&self.username, &self.password) {
            options.set_credentials(username, password);
        }

        options
    }

    /// Get network options for the MQTT connection
    /// Configures TCP-level settings like TCP_NODELAY
    pub fn network_options(&self) -> rumqttc::NetworkOptions {
        let mut options = rumqttc::NetworkOptions::new();

        // Enable TCP_NODELAY for reduced latency
        // Disables Nagle's algorithm, beneficial for real-time messaging
        options.set_tcp_nodelay(self.tcp_nodelay);

        options
    }
}

/// MQTT Quality of Service level
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)] // MQTT spec naming convention
pub enum QoS {
    /// At most once delivery
    AtMostOnce = 0,
    /// At least once delivery
    AtLeastOnce = 1,
    /// Exactly once delivery
    ExactlyOnce = 2,
}

impl From<QoS> for rumqttc::QoS {
    fn from(qos: QoS) -> Self {
        match qos {
            QoS::AtMostOnce => rumqttc::QoS::AtMostOnce,
            QoS::AtLeastOnce => rumqttc::QoS::AtLeastOnce,
            QoS::ExactlyOnce => rumqttc::QoS::ExactlyOnce,
        }
    }
}

/// Topic mapping configuration with Danube topic settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicMapping {
    /// MQTT topic pattern (supports wildcards: +, #)
    pub mqtt_topic: String,

    /// Target Danube topic
    pub danube_topic: String,

    /// QoS level for MQTT subscription
    #[serde(default = "default_qos")]
    pub qos: QoS,

    /// Number of partitions for the Danube topic (0 = non-partitioned)
    #[serde(default)]
    pub partitions: usize,

    /// Use reliable dispatch for this topic (WAL + Cloud persistence)
    /// If not specified, determined by QoS level:
    /// - QoS 0 (AtMostOnce) → non-reliable (default: false)
    /// - QoS 1/2 (AtLeastOnce/ExactlyOnce) → reliable (default: true)
    #[serde(default)]
    pub reliable_dispatch: Option<bool>,
}

impl TopicMapping {
    /// Get the effective reliable dispatch setting based on QoS if not explicitly set
    pub fn effective_reliable_dispatch(&self) -> bool {
        self.reliable_dispatch.unwrap_or_else(|| {
            // QoS 0 = non-reliable, QoS 1/2 = reliable
            self.qos != QoS::AtMostOnce
        })
    }
}

fn default_qos() -> QoS {
    QoS::AtLeastOnce
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let mut config = MqttConfig {
            broker_host: "localhost".to_string(),
            broker_port: 1883,
            client_id: "test-client".to_string(),
            username: None,
            password: None,
            use_tls: false,
            keep_alive_secs: 60,
            connection_timeout_secs: 30,
            max_packet_size: 1024 * 1024,
            topic_mappings: vec![TopicMapping {
                mqtt_topic: "sensors/#".to_string(),
                danube_topic: "/mqtt/sensors".to_string(),
                qos: QoS::AtLeastOnce,
                partitions: 0,
                reliable_dispatch: None,
            }],
            clean_session: true,
            include_metadata: true,
            tcp_nodelay: true,
        };

        assert!(config.validate().is_ok());

        // Test empty broker host
        config.broker_host = "".to_string();
        assert!(config.validate().is_err());

        // Test empty topic mappings
        config.broker_host = "localhost".to_string();
        config.topic_mappings = vec![];
        assert!(config.validate().is_err());
    }
}
