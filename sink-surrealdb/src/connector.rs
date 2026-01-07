//! SurrealDB Sink Connector implementation
//!
//! This module implements the core connector logic for streaming messages
//! from Danube topics to SurrealDB tables with:
//! - Multi-topic support with per-table batching
//! - Configurable batch sizes and flush intervals
//! - Automatic retry and error handling
//! - Performance metrics and health checks

use crate::config::{SurrealDBSinkConfig, TopicMapping};
use crate::record::{to_surrealdb_record, SurrealDBRecord};
use async_trait::async_trait;
use danube_connect_core::{
    ConnectorConfig, ConnectorError, ConnectorResult, ConsumerConfig, SinkConnector, SinkRecord,
    SubscriptionType,
};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use surrealdb::engine::remote::ws::{Client, Ws};
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;
use tracing::{debug, error, info, warn};

/// Context for managing a single SurrealDB table (per topic mapping)
#[derive(Debug)]
struct TableContext {
    /// Topic mapping configuration
    mapping: TopicMapping,

    /// Batch buffer for this table
    batch_buffer: Vec<SurrealDBRecord>,

    /// Last flush timestamp
    last_flush: Instant,

    /// Effective batch size for this table
    batch_size: usize,

    /// Effective flush interval for this table
    flush_interval: Duration,

    /// Statistics
    records_inserted: u64,
    batches_flushed: u64,
    last_error: Option<String>,
}

impl TableContext {
    fn new(mapping: TopicMapping, global_batch_size: usize, global_flush_interval_ms: u64) -> Self {
        let batch_size = mapping.batch_size.unwrap_or(global_batch_size);
        let flush_interval = Duration::from_millis(
            mapping
                .flush_interval_ms
                .unwrap_or(global_flush_interval_ms),
        );

        Self {
            mapping,
            batch_buffer: Vec::with_capacity(batch_size),
            last_flush: Instant::now(),
            batch_size,
            flush_interval,
            records_inserted: 0,
            batches_flushed: 0,
            last_error: None,
        }
    }

    fn should_flush(&self) -> bool {
        self.batch_buffer.len() >= self.batch_size
            || (!self.batch_buffer.is_empty() && self.last_flush.elapsed() >= self.flush_interval)
    }
}

/// SurrealDB Sink Connector
pub struct SurrealDBSinkConnector {
    /// Configuration
    config: SurrealDBSinkConfig,

    /// SurrealDB client connection
    client: Option<Surreal<Client>>,

    /// Table contexts (one per topic mapping)
    tables: HashMap<String, TableContext>,
}

impl SurrealDBSinkConnector {
    /// Create a new connector with the given configuration
    pub fn with_config(config: SurrealDBSinkConfig) -> Self {
        let tables = config
            .surrealdb
            .topic_mappings
            .iter()
            .map(|mapping| {
                let context = TableContext::new(
                    mapping.clone(),
                    config.surrealdb.batch_size,
                    config.surrealdb.flush_interval_ms,
                );
                (mapping.topic.clone(), context)
            })
            .collect();

        Self {
            config,
            client: None,
            tables,
        }
    }

    /// Create a new connector (loads config automatically)
    pub fn new() -> ConnectorResult<Self> {
        let config = SurrealDBSinkConfig::load()?;
        Ok(Self::with_config(config))
    }

    /// Flush a specific table's batch to SurrealDB
    async fn flush_table(&mut self, topic: &str) -> ConnectorResult<()> {
        let context = self
            .tables
            .get_mut(topic)
            .ok_or_else(|| ConnectorError::fatal(format!("Unknown topic: {}", topic)))?;

        if context.batch_buffer.is_empty() {
            return Ok(());
        }

        let table_name = &context.mapping.table_name;
        let batch_size = context.batch_buffer.len();

        debug!(
            "Flushing {} records to SurrealDB table '{}'",
            batch_size, table_name
        );

        let client = self
            .client
            .as_ref()
            .ok_or_else(|| ConnectorError::fatal("SurrealDB client not initialized"))?;

        // Insert records in batch
        let records: Vec<_> = context.batch_buffer.drain(..).collect();

        for record in records {
            // SurrealDB 2.x has serialization issues with serde_json::Value enums
            // Workaround: Use query parameters with cloned data
            // Clone is necessary because .bind() requires 'static lifetime
            let data = record.data.clone();
            
            let result = match &record.id {
                Some(id) => {
                    // Insert with specific record ID using query parameters
                    let thing = format!("{}:{}", table_name, id);
                    let query = format!("CREATE {} CONTENT $data", thing);
                    // Bind the data as a parameter - SurrealDB handles the serialization
                    client.query(query)
                        .bind(("data", data))
                        .await
                        .map(|_| ())
                        .map_err(|e| (id.clone(), e))
                }
                None => {
                    // Auto-generate ID using query parameters
                    let query = format!("CREATE {} CONTENT $data", table_name);
                    // Bind the data as a parameter - SurrealDB handles the serialization
                    client.query(query)
                        .bind(("data", data))
                        .await
                        .map(|_| ())
                        .map_err(|e| (String::from("auto"), e))
                }
            };

            if let Err((id, e)) = result {
                error!("Failed to insert record with ID '{}': {}", id, e);
                context.last_error = Some(format!("Insert error: {}", e));
                return Err(ConnectorError::retryable(format!(
                    "Failed to insert record: {}",
                    e
                )));
            }
        }

        // Update statistics
        context.records_inserted += batch_size as u64;
        context.batches_flushed += 1;
        context.last_flush = Instant::now();
        context.last_error = None;

        info!(
            "Successfully flushed {} records to table '{}' (total: {}, batches: {})",
            batch_size, table_name, context.records_inserted, context.batches_flushed
        );

        Ok(())
    }

    /// Flush all tables that need flushing
    async fn flush_all_pending(&mut self) -> ConnectorResult<()> {
        let topics_to_flush: Vec<String> = self
            .tables
            .iter()
            .filter(|(_, ctx)| ctx.should_flush())
            .map(|(topic, _)| topic.clone())
            .collect();

        for topic in topics_to_flush {
            self.flush_table(&topic).await?;
        }

        Ok(())
    }
}

#[async_trait]
impl SinkConnector for SurrealDBSinkConnector {
    async fn initialize(&mut self, _config: ConnectorConfig) -> ConnectorResult<()> {
        info!("Initializing SurrealDB Sink Connector");
        info!("Connecting to SurrealDB at: {}", self.config.surrealdb.url);

        // Connect to SurrealDB
        let client = Surreal::new::<Ws>(&self.config.surrealdb.url)
            .await
            .map_err(|e| {
                ConnectorError::retryable(format!("Failed to connect to SurrealDB: {}", e))
            })?;

        // Authenticate if credentials provided
        if let (Some(username), Some(password)) = (
            &self.config.surrealdb.username,
            &self.config.surrealdb.password,
        ) {
            client
                .signin(Root { username, password })
                .await
                .map_err(|e| {
                    ConnectorError::fatal(format!("SurrealDB authentication failed: {}", e))
                })?;
            info!("Authenticated with SurrealDB as user '{}'", username);
        }

        // Use namespace and database
        client
            .use_ns(&self.config.surrealdb.namespace)
            .use_db(&self.config.surrealdb.database)
            .await
            .map_err(|e| {
                ConnectorError::retryable(format!(
                    "Failed to use namespace '{}' and database '{}': {}",
                    self.config.surrealdb.namespace, self.config.surrealdb.database, e
                ))
            })?;

        info!(
            "Using namespace '{}' and database '{}'",
            self.config.surrealdb.namespace, self.config.surrealdb.database
        );

        self.client = Some(client);

        info!("SurrealDB connection initialized successfully");
        info!(
            "Configured {} table mappings",
            self.config.surrealdb.topic_mappings.len()
        );

        Ok(())
    }

    async fn consumer_configs(&self) -> ConnectorResult<Vec<ConsumerConfig>> {
        let configs = self
            .config
            .surrealdb
            .topic_mappings
            .iter()
            .map(|mapping| ConsumerConfig {
                topic: mapping.topic.clone(),
                consumer_name: format!(
                    "{}-{}",
                    self.config.core.connector_name, mapping.table_name
                ),
                subscription: mapping.subscription.clone(),
                subscription_type: SubscriptionType::Shared,
                expected_schema_subject: None, // No schema validation for SurrealDB sink (accepts any valid JSON)
            })
            .collect();

        Ok(configs)
    }

    async fn process(&mut self, record: SinkRecord) -> ConnectorResult<()> {
        let topic = record.topic();

        // Get the table context for this topic
        let context = self.tables.get_mut(topic).ok_or_else(|| {
            ConnectorError::fatal(format!("No mapping configured for topic: {}", topic))
        })?;

        // Convert message to SurrealDB record based on schema type
        let surrealdb_record = to_surrealdb_record(&record, &context.mapping)?;

        // Add to batch buffer
        context.batch_buffer.push(surrealdb_record);

        // Flush if necessary
        if context.should_flush() {
            self.flush_table(topic).await?;
        }

        Ok(())
    }

    async fn process_batch(&mut self, records: Vec<SinkRecord>) -> ConnectorResult<()> {
        for record in records {
            self.process(record).await?;
        }

        // Flush any pending batches
        self.flush_all_pending().await?;

        Ok(())
    }

    async fn shutdown(&mut self) -> ConnectorResult<()> {
        info!("Shutting down SurrealDB Sink Connector");

        // Flush all remaining batches
        for topic in self.tables.keys().cloned().collect::<Vec<_>>() {
            if let Err(e) = self.flush_table(&topic).await {
                warn!("Error flushing table during shutdown: {}", e);
            }
        }

        // Print final statistics
        info!("Final statistics:");
        for (topic, context) in &self.tables {
            info!(
                "  Topic '{}' → Table '{}': {} records ({} batches)",
                topic,
                context.mapping.table_name,
                context.records_inserted,
                context.batches_flushed
            );
        }

        info!("SurrealDB Sink Connector shutdown complete");
        Ok(())
    }

    async fn health_check(&self) -> ConnectorResult<()> {
        if self.client.is_none() {
            return Err(ConnectorError::fatal(
                "SurrealDB client not initialized. Call initialize() first.",
            ));
        }

        // Check for recent errors
        for (topic, context) in &self.tables {
            if let Some(error) = &context.last_error {
                warn!("Topic '{}' has recent error: {}", topic, error);
            }
        }

        Ok(())
    }
}

impl Default for SurrealDBSinkConnector {
    fn default() -> Self {
        Self::new().expect("Failed to create default connector")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StorageMode;
    use danube_connect_core::SchemaType;
    use serde_json::Value;

    #[test]
    fn test_table_context_flush_logic() {
        let mapping = TopicMapping {
            topic: "/test/topic".to_string(),
            subscription: "test-sub".to_string(),
            table_name: "events".to_string(),
            include_danube_metadata: false,
            batch_size: Some(10),
            flush_interval_ms: Some(5000),
            schema_type: SchemaType::Json,
            storage_mode: StorageMode::Document,
        };

        let mut context = TableContext::new(mapping, 100, 1000);

        // Should not flush when empty
        assert!(!context.should_flush());

        // Add records up to batch size
        for _ in 0..9 {
            context.batch_buffer.push(SurrealDBRecord {
                id: None,
                data: Value::Null,
            });
        }
        assert!(!context.should_flush());

        // Should flush when batch size reached
        context.batch_buffer.push(SurrealDBRecord {
            id: None,
            data: Value::Null,
        });
        assert!(context.should_flush());
    }

    #[test]
    fn test_connector_creation() {
        let config = SurrealDBSinkConfig {
            core: ConnectorConfig {
                connector_name: "test".to_string(),
                danube_service_url: "http://localhost:6650".to_string(),
                retry: Default::default(),
                processing: Default::default(),
                schemas: Vec::new(), // No schemas for sink connector test
            },
            surrealdb: crate::config::SurrealDBConfig {
                url: "ws://localhost:8000".to_string(),
                namespace: "test".to_string(),
                database: "test".to_string(),
                username: None,
                password: None,
                connection_timeout_secs: 30,
                request_timeout_secs: 30,
                topic_mappings: vec![TopicMapping {
                    topic: "/test/topic".to_string(),
                    subscription: "test-sub".to_string(),
                    table_name: "events".to_string(),
                    include_danube_metadata: true,
                    batch_size: None,
                    flush_interval_ms: None,
                    schema_type: SchemaType::Json,
                    storage_mode: StorageMode::Document,
                }],
                batch_size: 100,
                flush_interval_ms: 1000,
            },
        };

        let connector = SurrealDBSinkConnector::with_config(config);
        assert_eq!(connector.tables.len(), 1);
        assert!(connector.client.is_none());
    }
}
