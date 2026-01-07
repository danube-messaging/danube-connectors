//! SurrealDB Sink Connector for Danube Connect
//!
//! This connector consumes messages from Danube topics and inserts them into SurrealDB tables.
//! Perfect for building real-time applications with multi-model database capabilities.

mod config;
mod connector;
mod record;

use config::SurrealDBSinkConfig;
use connector::SurrealDBSinkConnector;
use danube_connect_core::{ConnectorResult, SinkRuntime};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> ConnectorResult<()> {
    // Initialize logging first
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,danube_sink_surrealdb=debug"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .try_init()
        .ok(); // Ignore error if already initialized

    tracing::info!("Starting SurrealDB Sink Connector");
    tracing::info!("Version: {}", env!("CARGO_PKG_VERSION"));

    // Load unified configuration from single file (TOML + ENV overrides)
    let config = SurrealDBSinkConfig::load().map_err(|e| {
        tracing::error!("Failed to load configuration: {}", e);
        e
    })?;

    // Validate configuration
    config.validate()?;

    tracing::info!("Configuration loaded and validated successfully");
    tracing::info!("Connector: {}", config.core.connector_name);
    tracing::info!("Danube URL: {}", config.core.danube_service_url);
    tracing::info!("SurrealDB URL: {}", config.surrealdb.url);
    tracing::info!(
        "SurrealDB Namespace: {}",
        config.surrealdb.namespace
    );
    tracing::info!("SurrealDB Database: {}", config.surrealdb.database);
    tracing::info!(
        "Topic Mappings: {} configured",
        config.surrealdb.topic_mappings.len()
    );

    for (idx, mapping) in config.surrealdb.topic_mappings.iter().enumerate() {
        tracing::info!(
            "  Mapping {}: Topic '{}' → Table '{}'",
            idx + 1,
            mapping.topic,
            mapping.table_name
        );
    }

    // Create connector instance with SurrealDB configuration
    let connector = SurrealDBSinkConnector::with_config(config.clone());

    // Create and run the sink runtime
    tracing::info!("Initializing connector runtime...");
    let mut runtime = SinkRuntime::new(connector, config.core).await?;
    
    // Run until shutdown signal
    runtime.run().await?;

    tracing::info!("SurrealDB Sink Connector terminated");
    Ok(())
}
