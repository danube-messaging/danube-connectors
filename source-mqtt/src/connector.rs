//! MQTT source connector implementation.

use crate::config::{MqttConfig, TopicMapping};
use async_trait::async_trait;
use danube_connect_core::{
    ConnectorConfig, ConnectorError, ConnectorResult, Offset, ProducerConfig, SchemaMapping,
    SourceConnector, SourceConnectorMode, SourceRecord, SourceSender,
};
use rumqttc::{AsyncClient, Event, Packet, Publish};
use tokio::task::AbortHandle;
use tracing::{debug, error, info, warn};

/// MQTT Source Connector
///
/// Subscribes to MQTT topics and publishes messages to Danube topics.
pub struct MqttSourceConnector {
    config: MqttConfig,
    schemas: Vec<SchemaMapping>,
    mqtt_client: Option<AsyncClient>,
    event_loop_abort: Option<AbortHandle>,
}

impl MqttSourceConnector {
    /// Create a new MQTT source connector with provided configuration
    pub fn with_config(config: MqttConfig, schemas: Vec<SchemaMapping>) -> Self {
        Self {
            config,
            schemas,
            mqtt_client: None,
            event_loop_abort: None,
        }
    }

    /// Create a new MQTT source connector with empty configuration
    /// This is used for testing purposes
    pub fn new() -> Self {
        Self {
            config: MqttConfig {
                broker_host: String::new(),
                broker_port: 1883,
                client_id: String::new(),
                username: None,
                password: None,
                use_tls: false,
                keep_alive_secs: 60,
                connection_timeout_secs: 30,
                max_packet_size: 10 * 1024 * 1024,
                topic_mappings: vec![],
                clean_session: true,
                include_metadata: true,
                tcp_nodelay: true,
            },
            schemas: vec![],
            mqtt_client: None,
            event_loop_abort: None,
        }
    }

    /// Check if MQTT topic matches pattern with wildcards
    fn topic_matches(pattern: &str, topic: &str) -> bool {
        let pattern_parts: Vec<&str> = pattern.split('/').collect();
        let topic_parts: Vec<&str> = topic.split('/').collect();

        Self::match_parts(&pattern_parts, &topic_parts)
    }

    fn match_parts(pattern_parts: &[&str], topic_parts: &[&str]) -> bool {
        if pattern_parts.is_empty() && topic_parts.is_empty() {
            return true;
        }

        if pattern_parts.is_empty() || topic_parts.is_empty() {
            return false;
        }

        let pattern_head = pattern_parts[0];
        let topic_head = topic_parts[0];

        match pattern_head {
            "#" => {
                // Multi-level wildcard - matches everything remaining
                true
            }
            "+" => {
                // Single-level wildcard - matches one level
                Self::match_parts(&pattern_parts[1..], &topic_parts[1..])
            }
            _ => {
                // Exact match required
                if pattern_head == topic_head {
                    Self::match_parts(&pattern_parts[1..], &topic_parts[1..])
                } else {
                    false
                }
            }
        }
    }

    /// Spawn MQTT event loop task
    fn spawn_event_loop(
        mut event_loop: rumqttc::EventLoop,
        sender: SourceSender,
        topic_mappings: Vec<TopicMapping>,
        include_metadata: bool,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            info!("MQTT event loop started");

            loop {
                match event_loop.poll().await {
                    Ok(event) => {
                        match event {
                            Event::Incoming(Packet::Publish(publish)) => {
                                debug!(
                                    "Received MQTT message: topic={}, qos={}, size={}",
                                    publish.topic,
                                    publish.qos as u8,
                                    publish.payload.len()
                                );

                                // Find matching Danube topic mapping
                                let mapping =
                                    Self::find_mapping_static(&publish.topic, &topic_mappings);

                                if let Some(mapping) = mapping {
                                    let record = Self::publish_to_record_static(
                                        &publish,
                                        mapping,
                                        include_metadata,
                                    );

                                    if let Err(e) = sender.send(record).await {
                                        error!("Failed to send message to source runtime: {}", e);
                                        break;
                                    }
                                } else {
                                    warn!(
                                        "No Danube topic mapping found for MQTT topic: {}",
                                        publish.topic
                                    );
                                }
                            }
                            Event::Incoming(Packet::ConnAck(connack)) => {
                                info!(
                                    "MQTT connected: session_present={}",
                                    connack.session_present
                                );
                            }
                            Event::Incoming(Packet::SubAck(suback)) => {
                                info!("MQTT subscription acknowledged: {:?}", suback.return_codes);
                            }
                            Event::Incoming(Packet::PingResp) => {
                                debug!("MQTT ping response received");
                            }
                            Event::Incoming(Packet::Disconnect) => {
                                warn!("MQTT disconnected");
                            }
                            Event::Outgoing(_) => {
                                // Outgoing packets, no action needed
                            }
                            _ => {
                                debug!("MQTT event: {:?}", event);
                            }
                        }
                    }
                    Err(e) => {
                        error!("MQTT event loop error: {}", e);
                        if sender.is_closed() {
                            break;
                        }
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    }
                }
            }

            info!("MQTT event loop stopped");
        })
    }

    /// Static version of publish_to_record for use in spawned task
    /// Creates a SourceRecord from MQTT message and topic mapping
    fn publish_to_record_static(
        publish: &Publish,
        mapping: &TopicMapping,
        include_metadata: bool,
    ) -> SourceRecord {
        // Convert MQTT payload to typed data
        // Try JSON first, fallback to base64-encoded bytes
        let payload_value = match serde_json::from_slice::<serde_json::Value>(&publish.payload) {
            Ok(json_value) => json_value,
            Err(_) => {
                // Not JSON - encode as base64 bytes object
                use serde_json::json;
                json!({
                    "data": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &publish.payload),
                    "size": publish.payload.len(),
                    "encoding": "base64"
                })
            }
        };

        let mut record = SourceRecord::new(&mapping.danube_topic, payload_value);

        // Add MQTT metadata as attributes
        if include_metadata {
            record = record
                .with_attribute("mqtt.topic", &publish.topic)
                .with_attribute("mqtt.qos", format!("{}", publish.qos as u8))
                .with_attribute("mqtt.retain", publish.retain.to_string())
                .with_attribute("mqtt.dup", publish.dup.to_string())
                .with_attribute("source", "mqtt");

            // Use MQTT topic as routing key for partitioned topics
            record = record.with_key(&publish.topic);
        }

        record
    }

    /// Find the matching topic mapping for an MQTT topic
    fn find_mapping_static<'a>(
        mqtt_topic: &str,
        topic_mappings: &'a [TopicMapping],
    ) -> Option<&'a TopicMapping> {
        // Find first matching mapping (exact or wildcard)
        topic_mappings.iter().find(|mapping| {
            // Exact match or wildcard match
            mapping.mqtt_topic == mqtt_topic || Self::topic_matches(&mapping.mqtt_topic, mqtt_topic)
        })
    }

    /// Find schema configuration for a Danube topic
    fn find_schema_config(&self, danube_topic: &str) -> Option<danube_connect_core::SchemaConfig> {
        self.schemas
            .iter()
            .find(|s| s.topic == danube_topic)
            .map(|schema| {
                // Convert SchemaMapping to SchemaConfig
                danube_connect_core::SchemaConfig {
                    subject: schema.subject.clone(),
                    schema_type: schema.schema_type.clone(),
                    schema_file: schema.schema_file.clone(),
                    auto_register: schema.auto_register,
                    version_strategy: schema.version_strategy.clone(),
                }
            })
    }
}

impl Default for MqttSourceConnector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SourceConnector for MqttSourceConnector {
    async fn initialize(&mut self, _config: ConnectorConfig) -> ConnectorResult<()> {
        info!("Initializing MQTT Source Connector");

        // Validate configuration (already loaded in main)
        self.config.validate()?;

        info!(
            "MQTT Configuration: broker={}:{}, client_id={}, topics={}",
            self.config.broker_host,
            self.config.broker_port,
            self.config.client_id,
            self.config.topic_mappings.len()
        );

        // Log topic mappings
        for mapping in &self.config.topic_mappings {
            info!(
                "Topic mapping: {} -> {} (QoS: {:?}, Partitions: {}, Reliable: {})",
                mapping.mqtt_topic,
                mapping.danube_topic,
                mapping.qos,
                mapping.partitions,
                mapping.effective_reliable_dispatch()
            );
        }

        info!("MQTT Source Connector initialized successfully");
        Ok(())
    }

    fn mode(&self) -> SourceConnectorMode {
        SourceConnectorMode::Streaming
    }

    async fn start_streaming(&mut self, sender: SourceSender) -> ConnectorResult<()> {
        if self.mqtt_client.is_some() {
            return Err(ConnectorError::config(
                "MQTT source streaming has already been started",
            ));
        }

        // Create MQTT client
        let mqtt_options = self.config.mqtt_options();
        let (client, mut event_loop) = AsyncClient::new(mqtt_options, 100);

        event_loop.network_options = self.config.network_options();

        // Subscribe to MQTT topics
        for mapping in &self.config.topic_mappings {
            info!(
                "Subscribing to MQTT topic: {} (QoS: {:?})",
                mapping.mqtt_topic, mapping.qos
            );

            client
                .subscribe(&mapping.mqtt_topic, mapping.qos.into())
                .await
                .map_err(|e| {
                    ConnectorError::fatal_with_source(
                        format!("Failed to subscribe to topic: {}", mapping.mqtt_topic),
                        e,
                    )
                })?;
        }

        // Spawn event loop in background task
        let event_loop_handle = Self::spawn_event_loop(
            event_loop,
            sender,
            self.config.topic_mappings.clone(),
            self.config.include_metadata,
        );

        self.mqtt_client = Some(client);
        self.event_loop_abort = Some(event_loop_handle.abort_handle());

        info!("MQTT Source Connector streaming started successfully");
        Ok(())
    }

    async fn producer_configs(&self) -> ConnectorResult<Vec<ProducerConfig>> {
        // Extract all unique Danube topics from the topic mappings
        // and create producer configurations for each
        let producer_configs: Vec<_> = self
            .config
            .topic_mappings
            .iter()
            .map(|mapping| {
                // Find matching schema configuration for this Danube topic
                let schema_config = self.find_schema_config(&mapping.danube_topic);

                ProducerConfig {
                    topic: mapping.danube_topic.clone(),
                    partitions: mapping.partitions,
                    reliable_dispatch: mapping.effective_reliable_dispatch(),
                    schema_config,
                }
            })
            .collect();

        if producer_configs.is_empty() {
            return Err(ConnectorError::config(
                "No topic mappings configured. Please add topic mappings in the configuration.",
            ));
        }

        Ok(producer_configs)
    }

    async fn commit(&mut self, offsets: Vec<Offset>) -> ConnectorResult<()> {
        // MQTT doesn't require explicit offset commits
        // Messages are acknowledged automatically by rumqttc
        debug!("Committed {} offsets", offsets.len());
        Ok(())
    }

    async fn shutdown(&mut self) -> ConnectorResult<()> {
        info!("Shutting down MQTT Source Connector");

        if let Some(abort_handle) = self.event_loop_abort.take() {
            abort_handle.abort();
        }

        // Disconnect MQTT client
        if let Some(client) = self.mqtt_client.take() {
            if let Err(e) = client.disconnect().await {
                warn!("Error disconnecting MQTT client: {}", e);
            }
        }

        info!("MQTT Source Connector stopped");
        Ok(())
    }

    async fn health_check(&self) -> ConnectorResult<()> {
        // Check if MQTT client is connected
        if self.mqtt_client.is_none() {
            return Err(ConnectorError::fatal("MQTT client not initialized"));
        }

        // Could add more sophisticated health checks here
        // (e.g., last message received time, connection state)

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topic_matching() {
        // Exact match
        assert!(MqttSourceConnector::topic_matches(
            "sensors/temp",
            "sensors/temp"
        ));
        assert!(!MqttSourceConnector::topic_matches(
            "sensors/temp",
            "sensors/humidity"
        ));

        // Single-level wildcard (+)
        assert!(MqttSourceConnector::topic_matches(
            "sensors/+/data",
            "sensors/temp/data"
        ));
        assert!(MqttSourceConnector::topic_matches(
            "sensors/+/data",
            "sensors/humidity/data"
        ));
        assert!(!MqttSourceConnector::topic_matches(
            "sensors/+/data",
            "sensors/temp/config"
        ));

        // Multi-level wildcard (#)
        assert!(MqttSourceConnector::topic_matches(
            "sensors/#",
            "sensors/temp"
        ));
        assert!(MqttSourceConnector::topic_matches(
            "sensors/#",
            "sensors/temp/data"
        ));
        assert!(MqttSourceConnector::topic_matches(
            "sensors/#",
            "sensors/humidity/zone1/data"
        ));
        assert!(!MqttSourceConnector::topic_matches(
            "sensors/#",
            "devices/temp"
        ));

        // Mixed wildcards
        assert!(MqttSourceConnector::topic_matches(
            "factory/+/sensors/#",
            "factory/line1/sensors/temp"
        ));
        assert!(MqttSourceConnector::topic_matches(
            "factory/+/sensors/#",
            "factory/line2/sensors/pressure/zone1"
        ));
    }

    #[test]
    fn test_connector_creation() {
        let connector = MqttSourceConnector::new();
        assert!(connector.mqtt_client.is_none());
        assert!(connector.event_loop_abort.is_none());
    }
}
