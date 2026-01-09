//! MQTT Source Connector for Danube Connect
//!
//! This connector subscribes to MQTT topics and publishes messages to Danube topics.
//! Perfect for IoT use cases where devices publish telemetry via MQTT.

mod config;
mod connector;

use config::MqttSourceConfig;
use connector::MqttSourceConnector;
use danube_connect_core::{ConnectorResult, SourceRuntime};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> ConnectorResult<()> {
    // Initialize logging first
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,danube_source_mqtt=debug"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .try_init()
        .ok(); // Ignore error if already initialized

    tracing::info!("Starting MQTT Source Connector");
    tracing::info!("Version: {}", env!("CARGO_PKG_VERSION"));

    // Load unified configuration from single file (TOML + ENV overrides)
    let config = MqttSourceConfig::load().map_err(|e| {
        tracing::error!("Failed to load configuration: {}", e);
        e
    })?;

    // Validate configuration
    config.validate()?;

    tracing::info!("Configuration loaded and validated successfully");
    tracing::info!("Connector: {}", config.core.connector_name);
    tracing::info!("Danube URL: {}", config.core.danube_service_url);
    tracing::info!("MQTT Broker: {}:{}", config.mqtt.broker_host, config.mqtt.broker_port);
    tracing::info!("MQTT Client ID: {}", config.mqtt.client_id);
    tracing::info!("Topic Mappings: {} configured", config.mqtt.topic_mappings.len());
    
    // Log each topic mapping with details
    for (idx, mapping) in config.mqtt.topic_mappings.iter().enumerate() {
        tracing::info!(
            "  [{}] {} → {} (QoS: {:?}, Partitions: {}, Reliable: {})",
            idx + 1,
            mapping.mqtt_topic,
            mapping.danube_topic,
            mapping.qos,
            mapping.partitions,
            mapping.effective_reliable_dispatch()
        );
    }

    // Create connector instance with MQTT configuration and schemas
    let connector = MqttSourceConnector::with_config(config.mqtt, config.core.schemas.clone());

    // Create and run the runtime
    let mut runtime = SourceRuntime::new(connector, config.core).await?;

    // Run until shutdown signal
    runtime.run().await?;

    tracing::info!("MQTT Source Connector stopped");
    Ok(())
}
