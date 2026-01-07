//! Delta Lake Sink Connector - Main Entry Point
//!
//! Streams events from Danube topics to Delta Lake tables with ACID guarantees.

use danube_connect_core::SinkRuntime;
use danube_sink_deltalake::{DeltaLakeSinkConfig, DeltaLakeSinkConnector};
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Starting Danube Delta Lake Sink Connector");

    // Load configuration
    tracing::info!("Loading configuration from CONNECTOR_CONFIG_PATH");
    let config = DeltaLakeSinkConfig::load()?;

    tracing::info!(
        "Configuration loaded successfully: connector_name={}, storage_backend={:?}",
        config.core.connector_name,
        config.deltalake.storage_backend
    );

    // Create connector
    let connector = DeltaLakeSinkConnector::with_config(config.clone());

    // Create and run runtime
    tracing::info!("Starting Danube runtime");
    let mut runtime = SinkRuntime::new(connector, config.core).await?;

    // Run the connector
    runtime.run().await?;

    tracing::info!("Delta Lake Sink Connector stopped");
    Ok(())
}
