//! Qdrant Sink Connector for Danube Connect
//!
//! This connector consumes messages from Danube topics and upserts vector embeddings to Qdrant.
//! Perfect for building RAG (Retrieval Augmented Generation) pipelines and AI applications.

mod config;
mod connector;
mod transform;

use config::QdrantSinkConfig;
use connector::QdrantSinkConnector;
use danube_connect_core::{ConnectorResult, SinkRuntime};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> ConnectorResult<()> {
    // Initialize logging first
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,danube_sink_qdrant=debug"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .try_init()
        .ok(); // Ignore error if already initialized

    tracing::info!("Starting Qdrant Sink Connector");
    tracing::info!("Version: {}", env!("CARGO_PKG_VERSION"));

    // Load unified configuration from single file (TOML + ENV overrides)
    let config = QdrantSinkConfig::load().map_err(|e| {
        tracing::error!("Failed to load configuration: {}", e);
        e
    })?;

    // Validate configuration
    config.validate()?;

    tracing::info!("Configuration loaded and validated successfully");
    tracing::info!("Connector: {}", config.core.connector_name);
    tracing::info!("Danube URL: {}", config.core.danube_service_url);
    tracing::info!("Qdrant URL: {}", config.qdrant.url);
    tracing::info!("Topic Mappings: {} configured", config.qdrant.topic_mappings.len());
    
    for (idx, mapping) in config.qdrant.topic_mappings.iter().enumerate() {
        tracing::info!(
            "  Mapping {}: Topic '{}' → Collection '{}' (dim={}, distance={:?})",
            idx + 1,
            mapping.topic,
            mapping.collection_name,
            mapping.vector_dimension,
            mapping.distance
        );
    }
    
    tracing::info!("Batch Size: {}", config.qdrant.batch_size);
    tracing::info!(
        "Batch Timeout: {}ms",
        config.qdrant.batch_timeout_ms
    );

    // Create connector instance with Qdrant configuration
    let connector = QdrantSinkConnector::with_config(config.qdrant);

    // Create and run the runtime
    let mut runtime = SinkRuntime::new(connector, config.core).await?;

    // Run until shutdown signal
    runtime.run().await?;

    tracing::info!("Qdrant Sink Connector stopped");
    Ok(())
}
