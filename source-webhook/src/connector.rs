//! Core connector logic for webhook processing and Danube publishing.

use async_trait::async_trait;
use chrono::Utc;
use danube_connect_core::{
    ConnectorConfig, ConnectorError, ConnectorResult, Offset, ProducerConfig, SourceConnector,
    SourceRecord,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::config::{EndpointConfig, WebhookSourceConfig};

/// Webhook connector state
pub struct WebhookConnector {
    /// Connector configuration
    config: WebhookSourceConfig,
    /// Channel receiver for incoming webhook records
    message_rx: Option<Receiver<SourceRecord>>,
    /// Channel sender for webhook handler (shared with HTTP server)
    message_tx: Option<Sender<SourceRecord>>,
    /// Endpoint configurations mapped by path
    endpoints: Arc<RwLock<HashMap<String, EndpointConfig>>>,
    /// HTTP server handle
    server_handle: Option<tokio::task::JoinHandle<()>>,
}

impl WebhookConnector {
    /// Create a new webhook connector with configuration
    pub fn with_config(config: WebhookSourceConfig) -> Self {
        // Build endpoint map
        let mut endpoints = HashMap::new();
        for endpoint in &config.endpoints {
            endpoints.insert(endpoint.path.clone(), endpoint.clone());
        }

        Self {
            config,
            message_rx: None,
            message_tx: None,
            endpoints: Arc::new(RwLock::new(endpoints)),
            server_handle: None,
        }
    }

    /// Get the message sender for the HTTP server
    #[allow(dead_code)]
    pub fn message_sender(&self) -> Option<Sender<SourceRecord>> {
        self.message_tx.clone()
    }

    /// Get endpoint configuration by path
    #[allow(dead_code)]
    pub async fn get_endpoint(&self, path: &str) -> Option<EndpointConfig> {
        let endpoints = self.endpoints.read().await;
        endpoints.get(path).cloned()
    }

    /// Get connector configuration
    #[allow(dead_code)]
    pub fn config(&self) -> &WebhookSourceConfig {
        &self.config
    }

    /// Create a SourceRecord from webhook data
    /// This is called by the HTTP server to convert webhook payloads to SourceRecords
    pub fn create_source_record(
        endpoint_config: &EndpointConfig,
        connector_name: &str,
        endpoint_path: &str,
        payload: Vec<u8>,
        headers: &HashMap<String, String>,
        client_ip: Option<&str>,
    ) -> SourceRecord {
        // Convert webhook payload to typed data
        // Try JSON first, fallback to base64-encoded bytes
        let payload_value = match serde_json::from_slice::<serde_json::Value>(&payload) {
            Ok(json_value) => json_value,
            Err(_) => {
                // Not JSON - encode as base64 bytes object
                use serde_json::json;
                json!({
                    "data": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &payload),
                    "size": payload.len(),
                    "encoding": "base64"
                })
            }
        };

        // Create source record with typed payload
        let mut record = SourceRecord::new(endpoint_config.danube_topic.clone(), payload_value)
            .with_attribute("webhook.source", connector_name)
            .with_attribute("webhook.endpoint", endpoint_path)
            .with_attribute("webhook.timestamp", Utc::now().to_rfc3339());

        // Add client IP if available
        if let Some(ip) = client_ip {
            record = record.with_attribute("webhook.ip", ip);
        }

        // Add user agent if available
        if let Some(user_agent) = headers.get("user-agent") {
            record = record.with_attribute("webhook.user_agent", user_agent);
        }

        // Add content type if available
        if let Some(content_type) = headers.get("content-type") {
            record = record.with_attribute("webhook.content_type", content_type);
        }

        record
    }
}

#[async_trait]
impl SourceConnector for WebhookConnector {
    async fn initialize(&mut self, _config: ConnectorConfig) -> ConnectorResult<()> {
        info!("Initializing Webhook Source Connector");

        // Validate configuration (already loaded in main)
        self.config.validate().map_err(|e| {
            ConnectorError::config(format!("Configuration validation failed: {}", e))
        })?;

        info!(
            "Webhook Configuration: connector={}, endpoints={}",
            self.config.core.connector_name,
            self.config.endpoints.len()
        );

        // Log endpoint configurations
        for endpoint in &self.config.endpoints {
            info!(
                "Endpoint: {} -> {} (Partitions: {})",
                endpoint.path, endpoint.danube_topic, endpoint.partitions
            );
        }

        // Create channel for message passing from HTTP server to runtime
        let (message_tx, message_rx) = mpsc::channel(1000);

        self.message_tx = Some(message_tx);
        self.message_rx = Some(message_rx);

        // Start HTTP server in background task
        // We need to create a shared state for the server
        let server_config = self.config.clone();
        let server_endpoints = Arc::clone(&self.endpoints);
        let server_tx = self.message_tx.clone().unwrap();

        let server_handle = tokio::spawn(async move {
            if let Err(e) =
                crate::server::start_server_with_state(server_config, server_endpoints, server_tx)
                    .await
            {
                error!("HTTP server error: {}", e);
            }
        });

        self.server_handle = Some(server_handle);

        info!("Webhook Source Connector initialized successfully");
        info!("HTTP server started on {}", self.config.bind_address());
        Ok(())
    }

    async fn producer_configs(&self) -> ConnectorResult<Vec<ProducerConfig>> {
        // Extract all unique Danube topics from endpoints
        // Use HashMap to deduplicate topics (multiple endpoints can use same topic)
        let mut topics: HashMap<String, (usize, bool)> = HashMap::new();

        for endpoint in &self.config.endpoints {
            // Use partitions from config directly
            // 0 = non-partitioned (runtime won't call with_partitions)
            // >0 = partitioned with n partitions
            let partitions = endpoint.partitions as usize;

            // Use reliable_dispatch from config
            let reliable_dispatch = endpoint.reliable_dispatch;

            topics.insert(
                endpoint.danube_topic.clone(),
                (partitions, reliable_dispatch),
            );
        }

        let producer_configs: Vec<_> = topics
            .into_iter()
            .map(|(topic, (partitions, reliable_dispatch))| ProducerConfig {
                topic,
                partitions,
                reliable_dispatch,
                schema_config: None, // No schema registry for webhook source (uses auto-detection)
            })
            .collect();

        if producer_configs.is_empty() {
            return Err(ConnectorError::config(
                "No endpoints configured. Please add endpoint mappings in the configuration.",
            ));
        }

        Ok(producer_configs)
    }

    async fn poll(&mut self) -> ConnectorResult<Vec<SourceRecord>> {
        let mut records = Vec::new();

        // Receive messages from channel with timeout
        if let Some(ref mut rx) = self.message_rx {
            match tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await {
                Ok(Some(record)) => {
                    records.push(record);

                    // Try to receive more messages without blocking
                    while let Ok(record) = rx.try_recv() {
                        records.push(record);
                        // Limit batch size
                        if records.len() >= 100 {
                            break;
                        }
                    }
                }
                Ok(None) => {
                    // Channel closed
                    return Err(ConnectorError::fatal("Webhook channel closed"));
                }
                Err(_) => {
                    // Timeout - no messages available, this is normal
                }
            }
        }

        Ok(records)
    }

    async fn commit(&mut self, _offsets: Vec<Offset>) -> ConnectorResult<()> {
        // Webhooks don't require offset commits
        // Messages are acknowledged via HTTP response
        Ok(())
    }

    async fn shutdown(&mut self) -> ConnectorResult<()> {
        info!("Shutting down Webhook Source Connector");

        // Close the message channel
        self.message_tx = None;

        // Stop HTTP server
        if let Some(handle) = self.server_handle.take() {
            handle.abort();
            info!("HTTP server stopped");
        }

        Ok(())
    }
}
