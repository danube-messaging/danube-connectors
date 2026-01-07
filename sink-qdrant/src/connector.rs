//! Qdrant sink connector implementation

use crate::config::{QdrantConfig, TopicMapping};
use crate::transform::transform_to_point;
use async_trait::async_trait;
use danube_connect_core::{
    ConnectorConfig, ConnectorError, ConnectorResult, ConsumerConfig, SinkConnector, SinkRecord,
};
use qdrant_client::qdrant::PointStruct;
use qdrant_client::qdrant::{CreateCollectionBuilder, UpsertPointsBuilder};
use qdrant_client::Qdrant;
use std::collections::HashMap;
use std::time::Instant;
use tracing::{debug, info, warn};

/// Qdrant Sink Connector
///
/// Consumes messages from Danube topics and upserts vector embeddings to Qdrant.
/// Per-collection context for batching and tracking
struct CollectionContext {
    /// Topic mapping configuration for this collection
    mapping: TopicMapping,
    /// Batch buffer for this collection
    batch_buffer: Vec<PointStruct>,
    /// Last flush time for this collection
    last_flush: Instant,
    /// Effective batch size (topic-specific or global)
    effective_batch_size: usize,
    /// Effective batch timeout (topic-specific or global)
    effective_batch_timeout_ms: u64,
    /// Statistics
    points_inserted: u64,
    batches_flushed: u64,
}

impl CollectionContext {
    fn new(mapping: TopicMapping, global_batch_size: usize, global_batch_timeout: u64) -> Self {
        let effective_batch_size = mapping.effective_batch_size(global_batch_size);
        let effective_batch_timeout_ms = mapping.effective_batch_timeout(global_batch_timeout);

        Self {
            mapping,
            batch_buffer: Vec::with_capacity(effective_batch_size),
            last_flush: Instant::now(),
            effective_batch_size,
            effective_batch_timeout_ms,
            points_inserted: 0,
            batches_flushed: 0,
        }
    }

    fn should_flush(&self) -> bool {
        if self.batch_buffer.is_empty() {
            return false;
        }

        // Flush if batch is full
        if self.batch_buffer.len() >= self.effective_batch_size {
            return true;
        }

        // Flush if timeout exceeded
        let elapsed = self.last_flush.elapsed().as_millis() as u64;
        elapsed >= self.effective_batch_timeout_ms
    }
}

pub struct QdrantSinkConnector {
    config: QdrantConfig,
    client: Option<Qdrant>,
    /// Collection contexts keyed by Danube topic
    collections: HashMap<String, CollectionContext>,
}

impl QdrantSinkConnector {
    /// Create a new Qdrant sink connector with provided configuration
    pub fn with_config(config: QdrantConfig) -> Self {
        Self {
            config,
            client: None,
            collections: HashMap::new(),
        }
    }

    /// Create a new Qdrant sink connector with empty configuration
    pub fn new() -> Self {
        Self {
            config: QdrantConfig {
                url: String::new(),
                api_key: None,
                topic_mappings: vec![],
                batch_size: 100,
                batch_timeout_ms: 1000,
                timeout_secs: 30,
            },
            client: None,
            collections: HashMap::new(),
        }
    }

    /// Flush batch for a specific collection
    async fn flush_batch(&mut self, topic: &str) -> ConnectorResult<()> {
        let context = self.collections.get_mut(topic).ok_or_else(|| {
            ConnectorError::fatal(format!("No collection context found for topic: {}", topic))
        })?;

        if context.batch_buffer.is_empty() {
            return Ok(());
        }

        let client = self
            .client
            .as_ref()
            .ok_or_else(|| ConnectorError::fatal("Qdrant client not initialized"))?;

        let points_to_insert = std::mem::take(&mut context.batch_buffer);
        let count = points_to_insert.len();

        info!(
            "Flushing batch of {} points to Qdrant collection '{}' (topic: {})",
            count, context.mapping.collection_name, topic
        );

        // Upsert points to Qdrant
        client
            .upsert_points(UpsertPointsBuilder::new(
                &context.mapping.collection_name,
                points_to_insert,
            ))
            .await
            .map_err(|e| {
                ConnectorError::retryable(format!("Failed to upsert points to Qdrant: {}", e))
            })?;

        context.points_inserted += count as u64;
        context.batches_flushed += 1;
        context.last_flush = Instant::now();

        info!(
            "Successfully inserted {} points to '{}' (total: {}, batches: {})",
            count,
            context.mapping.collection_name,
            context.points_inserted,
            context.batches_flushed
        );

        Ok(())
    }

    /// Ensure collection exists for a specific mapping, create if needed
    async fn ensure_collection(&self, mapping: &TopicMapping) -> ConnectorResult<()> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| ConnectorError::fatal("Qdrant client not initialized"))?;

        // Check if collection exists
        let collections = client
            .list_collections()
            .await
            .map_err(|e| ConnectorError::fatal(format!("Failed to list collections: {}", e)))?;

        let collection_exists = collections
            .collections
            .iter()
            .any(|c| c.name == mapping.collection_name);

        if collection_exists {
            info!(
                "Collection '{}' already exists (topic: {})",
                mapping.collection_name, mapping.topic
            );
            return Ok(());
        }

        if !mapping.auto_create_collection {
            return Err(ConnectorError::fatal(format!(
                "Collection '{}' does not exist and auto_create_collection is disabled",
                mapping.collection_name
            )));
        }

        // Create collection
        info!(
            "Creating collection '{}' with dimension {} and distance metric {:?} (topic: {})",
            mapping.collection_name, mapping.vector_dimension, mapping.distance, mapping.topic
        );

        let vectors_config = qdrant_client::qdrant::VectorParamsBuilder::new(
            mapping.vector_dimension as u64,
            mapping.distance.to_qdrant(),
        )
        .build();

        client
            .create_collection(
                CreateCollectionBuilder::new(&mapping.collection_name)
                    .vectors_config(vectors_config),
            )
            .await
            .map_err(|e| {
                ConnectorError::fatal(format!(
                    "Failed to create collection '{}': {}",
                    mapping.collection_name, e
                ))
            })?;

        info!(
            "Collection '{}' created successfully",
            mapping.collection_name
        );

        Ok(())
    }
}

impl Default for QdrantSinkConnector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SinkConnector for QdrantSinkConnector {
    async fn initialize(&mut self, _config: ConnectorConfig) -> ConnectorResult<()> {
        info!("Initializing Qdrant Sink Connector");

        // Validate configuration (already loaded in main)
        self.config.validate()?;

        info!(
            "Qdrant Configuration: url={}, {} topic mapping(s)",
            self.config.url,
            self.config.topic_mappings.len()
        );

        // Create Qdrant client
        let client_config = self.config.qdrant_client_config();
        let client = Qdrant::new(client_config)
            .map_err(|e| ConnectorError::fatal(format!("Failed to create Qdrant client: {}", e)))?;

        // Test connection by listing collections
        client
            .list_collections()
            .await
            .map_err(|e| ConnectorError::fatal(format!("Failed to connect to Qdrant: {}", e)))?;

        info!("Successfully connected to Qdrant at {}", self.config.url);

        self.client = Some(client);

        // Initialize collection contexts for each topic mapping
        for mapping in &self.config.topic_mappings {
            info!(
                "Initializing collection '{}' for topic '{}' (dimension={}, distance={:?})",
                mapping.collection_name, mapping.topic, mapping.vector_dimension, mapping.distance
            );

            // Ensure collection exists
            self.ensure_collection(mapping).await?;

            // Create collection context
            let context = CollectionContext::new(
                mapping.clone(),
                self.config.batch_size,
                self.config.batch_timeout_ms,
            );

            self.collections.insert(mapping.topic.clone(), context);
        }

        info!(
            "Qdrant Sink Connector initialized successfully with {} collection(s)",
            self.collections.len()
        );
        Ok(())
    }

    async fn consumer_configs(&self) -> ConnectorResult<Vec<ConsumerConfig>> {
        // Return consumer config for each topic mapping
        let configs = self
            .config
            .topic_mappings
            .iter()
            .map(|mapping| ConsumerConfig {
                topic: mapping.topic.clone(),
                consumer_name: format!("qdrant-sink-{}", mapping.collection_name),
                subscription: mapping.subscription.clone(),
                subscription_type: mapping.subscription_type.clone(),
                expected_schema_subject: None, // No schema validation for Qdrant sink (accepts any valid JSON)
            })
            .collect();

        Ok(configs)
    }

    async fn process(&mut self, record: SinkRecord) -> ConnectorResult<()> {
        let topic = record.topic();

        // Get collection context for this topic
        let context = self.collections.get_mut(topic).ok_or_else(|| {
            ConnectorError::invalid_data(
                format!("No collection configured for topic: {}", topic),
                vec![],
            )
        })?;

        // Transform Danube message to Qdrant point
        let point = transform_to_point(
            &record,
            context.mapping.vector_dimension,
            context.mapping.include_danube_metadata,
        )?;

        debug!(
            "Transformed message from topic {} offset {} into Qdrant point for collection '{}'",
            record.topic(),
            record.offset(),
            context.mapping.collection_name
        );

        // Add to batch buffer
        context.batch_buffer.push(point);

        // Flush if batch is ready
        if context.should_flush() {
            self.flush_batch(topic).await?;
        }

        Ok(())
    }

    async fn process_batch(&mut self, records: Vec<SinkRecord>) -> ConnectorResult<()> {
        // Process each record using process() method
        // This automatically routes to correct collection and handles batching
        for record in records {
            self.process(record).await?;
        }

        Ok(())
    }

    async fn shutdown(&mut self) -> ConnectorResult<()> {
        info!("Shutting down Qdrant Sink Connector");

        // Flush any remaining points in all collections
        let topics: Vec<String> = self.collections.keys().cloned().collect();

        for topic in topics {
            if let Some(context) = self.collections.get(&topic) {
                if !context.batch_buffer.is_empty() {
                    warn!(
                        "Flushing {} remaining points from collection '{}' (topic: {}) before shutdown",
                        context.batch_buffer.len(),
                        context.mapping.collection_name,
                        topic
                    );
                    self.flush_batch(&topic).await?;
                }
            }
        }

        // Print statistics for all collections
        let mut total_points = 0u64;
        let mut total_batches = 0u64;

        for (topic, context) in &self.collections {
            info!(
                "Collection '{}' (topic: {}): {} points inserted, {} batches flushed",
                context.mapping.collection_name,
                topic,
                context.points_inserted,
                context.batches_flushed
            );
            total_points += context.points_inserted;
            total_batches += context.batches_flushed;
        }

        info!(
            "Qdrant Sink Connector stopped. Total: {} points inserted, {} batches across {} collection(s)",
            total_points, total_batches, self.collections.len()
        );
        Ok(())
    }

    async fn health_check(&self) -> ConnectorResult<()> {
        // Check if client is initialized
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| ConnectorError::fatal("Qdrant client not initialized"))?;

        // Verify connection by listing collections
        client
            .list_collections()
            .await
            .map_err(|e| ConnectorError::retryable(format!("Health check failed: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Distance;
    use danube_connect_core::SubscriptionType;
    use std::collections::HashMap;

    #[test]
    fn test_connector_creation() {
        let connector = QdrantSinkConnector::new();
        assert!(connector.client.is_none());
        assert_eq!(connector.collections.len(), 0);
    }

    #[test]
    fn test_collection_context_flush_logic() {
        let mapping = TopicMapping {
            topic: "/default/test".to_string(),
            subscription: "test-sub".to_string(),
            subscription_type: SubscriptionType::Exclusive,
            collection_name: "test_collection".to_string(),
            vector_dimension: 384,
            distance: Distance::Cosine,
            auto_create_collection: true,
            include_danube_metadata: true,
            batch_size: Some(3),
            batch_timeout_ms: None,
        };

        let mut context = CollectionContext::new(mapping, 100, 1000);

        assert!(!context.should_flush()); // Empty buffer

        // Add points up to batch size
        let empty_payload: HashMap<String, qdrant_client::qdrant::Value> = HashMap::new();

        context
            .batch_buffer
            .push(PointStruct::new(1, vec![0.1, 0.2], empty_payload.clone()));
        context
            .batch_buffer
            .push(PointStruct::new(2, vec![0.3, 0.4], empty_payload.clone()));
        assert!(!context.should_flush()); // Not full yet

        context
            .batch_buffer
            .push(PointStruct::new(3, vec![0.5, 0.6], empty_payload));
        assert!(context.should_flush()); // Now should flush
    }

    #[test]
    fn test_topic_mapping_effective_values() {
        let mapping = TopicMapping {
            topic: "/default/test".to_string(),
            subscription: "test-sub".to_string(),
            subscription_type: SubscriptionType::Exclusive,
            collection_name: "test_collection".to_string(),
            vector_dimension: 384,
            distance: Distance::Cosine,
            auto_create_collection: true,
            include_danube_metadata: true,
            batch_size: Some(50),
            batch_timeout_ms: Some(500),
        };

        // Uses topic-specific values
        assert_eq!(mapping.effective_batch_size(100), 50);
        assert_eq!(mapping.effective_batch_timeout(1000), 500);

        let mapping_defaults = TopicMapping {
            topic: "/default/test2".to_string(),
            subscription: "test-sub2".to_string(),
            subscription_type: SubscriptionType::Exclusive,
            collection_name: "test_collection2".to_string(),
            vector_dimension: 768,
            distance: Distance::Euclid,
            auto_create_collection: true,
            include_danube_metadata: false,
            batch_size: None,
            batch_timeout_ms: None,
        };

        // Uses global values
        assert_eq!(mapping_defaults.effective_batch_size(100), 100);
        assert_eq!(mapping_defaults.effective_batch_timeout(1000), 1000);
    }
}
