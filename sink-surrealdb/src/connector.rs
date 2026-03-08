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
};
use std::collections::HashMap;
use surrealdb::engine::remote::ws::{Client, Ws};
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;
use tracing::{debug, error, info, warn};

/// Context for managing a single SurrealDB table (per topic mapping)
#[derive(Debug)]
struct TableContext {
    /// Topic mapping configuration
    mapping: TopicMapping,

    /// Statistics
    records_inserted: u64,
    batches_flushed: u64,
    last_error: Option<String>,
}

impl TableContext {
    fn new(mapping: TopicMapping) -> Self {
        Self {
            mapping,
            records_inserted: 0,
            batches_flushed: 0,
            last_error: None,
        }
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
            .routes
            .iter()
            .map(|mapping| {
                let context = TableContext::new(mapping.clone());
                (mapping.from.clone(), context)
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
    async fn flush_table(
        &mut self,
        topic: &str,
        records: Vec<SurrealDBRecord>,
    ) -> ConnectorResult<()> {
        let context = self
            .tables
            .get_mut(topic)
            .ok_or_else(|| ConnectorError::fatal(format!("Unknown topic: {}", topic)))?;

        if records.is_empty() {
            return Ok(());
        }

        let table_name = &context.mapping.to;
        let batch_size = records.len();

        debug!(
            "Flushing {} records to SurrealDB table '{}'",
            batch_size, table_name
        );

        let client = self
            .client
            .as_ref()
            .ok_or_else(|| ConnectorError::fatal("SurrealDB client not initialized"))?;

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
                    client
                        .query(query)
                        .bind(("data", data))
                        .await
                        .map(|_| ())
                        .map_err(|e| (id.clone(), e))
                }
                None => {
                    // Auto-generate ID using query parameters
                    let query = format!("CREATE {} CONTENT $data", table_name);
                    // Bind the data as a parameter - SurrealDB handles the serialization
                    client
                        .query(query)
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
        context.last_error = None;

        info!(
            "Successfully flushed {} records to table '{}' (total: {}, batches: {})",
            batch_size, table_name, context.records_inserted, context.batches_flushed
        );

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
            self.config.surrealdb.routes.len()
        );

        Ok(())
    }

    async fn consumer_configs(&self) -> ConnectorResult<Vec<ConsumerConfig>> {
        let configs = self
            .config
            .surrealdb
            .routes
            .iter()
            .map(|mapping| ConsumerConfig {
                topic: mapping.from.clone(),
                consumer_name: format!("{}-{}", self.config.core.connector_name, mapping.to),
                subscription: mapping.subscription.clone(),
                subscription_type: mapping.subscription_type.clone(),
                expected_schema_subject: mapping.expected_schema_subject.clone(),
            })
            .collect();

        Ok(configs)
    }

    async fn process_batch(&mut self, records: Vec<SinkRecord>) -> ConnectorResult<()> {
        let mut batches: HashMap<String, Vec<SurrealDBRecord>> = HashMap::new();

        for record in records {
            let topic = record.topic().to_string();

            let context = self.tables.get(&topic).ok_or_else(|| {
                ConnectorError::fatal(format!("No mapping configured for topic: {}", topic))
            })?;

            let surrealdb_record = to_surrealdb_record(&record, &context.mapping)?;
            batches.entry(topic).or_default().push(surrealdb_record);
        }

        for (topic, batch) in batches {
            self.flush_table(&topic, batch).await?;
        }

        Ok(())
    }

    async fn shutdown(&mut self) -> ConnectorResult<()> {
        info!("Shutting down SurrealDB Sink Connector");

        // Print final statistics
        info!("Final statistics:");
        for (topic, context) in &self.tables {
            info!(
                "  Topic '{}' → Table '{}': {} records ({} batches)",
                topic, context.mapping.to, context.records_inserted, context.batches_flushed
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
    use danube_connect_core::SubscriptionType;

    #[test]
    fn test_table_context_creation() {
        let mapping = TopicMapping {
            from: "/test/topic".to_string(),
            subscription: "test-sub".to_string(),
            subscription_type: SubscriptionType::Shared,
            to: "events".to_string(),
            include_danube_metadata: false,
            expected_schema_subject: None,
            storage_mode: StorageMode::Document,
        };

        let context = TableContext::new(mapping.clone());

        assert_eq!(context.mapping.from, mapping.from);
        assert_eq!(context.mapping.to, mapping.to);
        assert_eq!(context.records_inserted, 0);
        assert_eq!(context.batches_flushed, 0);
        assert!(context.last_error.is_none());
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
                routes: vec![TopicMapping {
                    from: "/test/topic".to_string(),
                    subscription: "test-sub".to_string(),
                    subscription_type: SubscriptionType::Shared,
                    to: "events".to_string(),
                    include_danube_metadata: true,
                    expected_schema_subject: None,
                    storage_mode: StorageMode::Document,
                }],
            },
        };

        let connector = SurrealDBSinkConnector::with_config(config);
        assert_eq!(connector.tables.len(), 1);
        assert!(connector.client.is_none());
    }
}
