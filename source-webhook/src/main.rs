//! HTTP/Webhook Source Connector for Danube
//!
//! A high-performance HTTP server that receives webhook events from external SaaS platforms
//! and publishes them to Danube topics.

mod auth;
mod config;
mod connector;
mod rate_limit;
mod server;

use anyhow::{Context, Result};
use danube_connect_core::{ConnectorConfig, SourceRuntime};
use std::env;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::WebhookSourceConfig;
use connector::WebhookConnector;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    init_tracing();

    tracing::info!("Starting Danube HTTP/Webhook Source Connector");

    // Load configuration
    let config_path = env::var("CONNECTOR_CONFIG_PATH")
        .unwrap_or_else(|_| "config/connector.toml".to_string());

    tracing::info!("Loading configuration from: {}", config_path);

    let webhook_config = WebhookSourceConfig::from_file(&config_path)
        .context("Failed to load configuration")?;

    tracing::info!(
        connector_name = %webhook_config.core.connector_name,
        danube_url = %webhook_config.core.danube_service_url,
        server_addr = %webhook_config.bind_address(),
        endpoints = webhook_config.endpoints.len(),
        "Configuration loaded successfully"
    );

    // Log endpoint configuration
    for endpoint in &webhook_config.endpoints {
        tracing::info!(
            path = %endpoint.path,
            topic = %endpoint.danube_topic,
            partitions = endpoint.partitions,
            reliable_dispatch = endpoint.reliable_dispatch,
            "Configured endpoint"
        );
    }

    // Log authentication configuration
    tracing::info!(
        auth_type = ?webhook_config.auth.auth_type,
        "Authentication configured"
    );

    // Create core connector config for runtime
    let core_config = ConnectorConfig {
        connector_name: webhook_config.core.connector_name.clone(),
        danube_service_url: webhook_config.core.danube_service_url.clone(),
        ..Default::default()
    };

    // Create webhook connector
    let connector = WebhookConnector::with_config(webhook_config);

    // Create and run the runtime (this will call initialize which starts the HTTP server)
    tracing::info!("Starting Danube runtime");
    let mut runtime = SourceRuntime::new(connector, core_config).await?;

    // Run runtime (this blocks until shutdown)
    runtime.run().await?;

    tracing::info!("Connector stopped");
    Ok(())
}

/// Initialize tracing/logging
fn init_tracing() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}
