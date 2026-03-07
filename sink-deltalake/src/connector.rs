//! Delta Lake Sink Connector implementation
//!
//! This connector streams events from Danube topics to Delta Lake tables,
//! supporting S3, Azure Blob Storage, and Google Cloud Storage.

use crate::config::{DeltaLakeSinkConfig, StorageBackend, TopicMapping};
use crate::record::to_record_batch;
use async_trait::async_trait;
use danube_connect_core::{
    ConnectorConfig, ConnectorError, ConnectorResult, ConsumerConfig, SinkConnector, SinkRecord,
    SubscriptionType,
};
use deltalake::operations::create::CreateBuilder;
use deltalake::writer::{DeltaWriter, RecordBatchWriter};
use deltalake::{DeltaTable, DeltaTableError};
use std::collections::HashMap;
use tracing::{debug, info};
use url::Url;

/// Delta Lake Sink Connector
///
/// Streams events from Danube topics to Delta Lake tables with ACID guarantees.
pub struct DeltaLakeSinkConnector {
    /// Connector configuration
    config: DeltaLakeSinkConfig,

    /// Delta tables cache (table_path -> DeltaTable)
    tables: HashMap<String, DeltaTable>,
}

impl DeltaLakeSinkConnector {
    /// Create a new Delta Lake Sink Connector with configuration
    pub fn with_config(config: DeltaLakeSinkConfig) -> Self {
        Self {
            config,
            tables: HashMap::new(),
        }
    }

    /// Get or create a Delta table
    async fn get_or_create_table(
        &mut self,
        mapping: &TopicMapping,
    ) -> ConnectorResult<&mut DeltaTable> {
        // Check if table already exists
        if !self.tables.contains_key(&mapping.delta_table_path) {
            info!("Opening Delta table at path: {}", mapping.delta_table_path);

            // Configure storage options based on backend
            let storage_options = self.build_storage_options()?;

            // Parse table path as URL
            let table_url = Url::parse(&mapping.delta_table_path).map_err(|e| {
                ConnectorError::fatal(format!("Invalid Delta table path URL: {}", e))
            })?;

            // Try to open existing table or create new one
            let table = match deltalake::open_table_with_storage_options(
                table_url,
                storage_options.clone(),
            )
            .await
            {
                Ok(table) => {
                    info!("Loaded existing Delta table: {}", mapping.delta_table_path);
                    table
                }
                Err(DeltaTableError::NotATable(_)) => {
                    info!(
                        "Table does not exist, creating new Delta table: {}",
                        mapping.delta_table_path
                    );
                    self.create_table(mapping, storage_options).await?
                }
                Err(e) => {
                    return Err(ConnectorError::fatal(format!(
                        "Failed to open Delta table: {}",
                        e
                    )))
                }
            };

            // Cache the table
            self.tables.insert(mapping.delta_table_path.clone(), table);
        }

        // Return mutable reference to the table
        Ok(self.tables.get_mut(&mapping.delta_table_path).unwrap())
    }

    /// Create a new Delta table with user-defined schema
    async fn create_table(
        &self,
        mapping: &TopicMapping,
        storage_options: HashMap<String, String>,
    ) -> ConnectorResult<DeltaTable> {
        // Build Arrow schema from config
        let schema = crate::record::build_arrow_schema(mapping)?;

        // Convert Arrow fields to Delta StructFields
        // Note: delta-rs 0.29 doesn't provide TryFrom traits for Arrow types
        // Manual conversion provides explicit control over type mapping
        let delta_fields: Vec<deltalake::kernel::StructField> = schema
            .fields()
            .iter()
            .map(|f| {
                let delta_type = arrow_to_delta_datatype(f.data_type());
                deltalake::kernel::StructField::new(f.name().clone(), delta_type, f.is_nullable())
            })
            .collect();

        // Create Delta table
        let table = CreateBuilder::new()
            .with_location(&mapping.delta_table_path)
            .with_storage_options(storage_options)
            .with_columns(delta_fields)
            .await
            .map_err(|e| ConnectorError::fatal(format!("Failed to create Delta table: {}", e)))?;

        info!("Created new Delta table: {}", mapping.delta_table_path);
        Ok(table)
    }

    /// Build storage options based on configured backend
    fn build_storage_options(&self) -> ConnectorResult<HashMap<String, String>> {
        let mut options = HashMap::new();

        match self.config.deltalake.storage_backend {
            StorageBackend::S3 => {
                // AWS credentials from environment (AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY)
                if let Some(region) = &self.config.deltalake.s3_region {
                    options.insert("region".to_string(), region.clone());
                }

                // Custom endpoint for MinIO or S3-compatible storage
                if let Some(endpoint) = &self.config.deltalake.s3_endpoint {
                    options.insert("endpoint".to_string(), endpoint.clone());
                }

                // Allow HTTP for local MinIO testing
                if self.config.deltalake.s3_allow_http {
                    options.insert("allow_http".to_string(), "true".to_string());
                }

                info!("Using S3 storage backend");
            }
            StorageBackend::Azure => {
                // Azure credentials from environment (AZURE_STORAGE_ACCOUNT_KEY or AZURE_STORAGE_SAS_TOKEN)
                if let Some(account) = &self.config.deltalake.azure_storage_account {
                    options.insert("account_name".to_string(), account.clone());
                }

                if let Some(container) = &self.config.deltalake.azure_container {
                    options.insert("container_name".to_string(), container.clone());
                }

                info!("Using Azure Blob Storage backend");
            }
            StorageBackend::GCS => {
                // GCP credentials from environment (GOOGLE_APPLICATION_CREDENTIALS)
                if let Some(project_id) = &self.config.deltalake.gcp_project_id {
                    options.insert("project_id".to_string(), project_id.clone());
                }

                info!("Using Google Cloud Storage backend");
            }
        }

        Ok(options)
    }

    /// Write a batch of records to Delta Lake
    async fn write_batch(
        &mut self,
        mapping: &TopicMapping,
        records: Vec<SinkRecord>,
    ) -> ConnectorResult<()> {
        if records.is_empty() {
            return Ok(());
        }

        debug!(
            "Writing batch of {} records to Delta table: {}",
            records.len(),
            mapping.delta_table_path
        );

        // Convert records to Arrow RecordBatch
        let record_batch = to_record_batch(&records, mapping)?;

        // Get or create the table
        let table = self.get_or_create_table(mapping).await?;

        // Create a fresh writer for this write operation
        // Note: RecordBatchWriter is not Sync, so we can't cache it
        let mut writer = RecordBatchWriter::for_table(table).map_err(|e| {
            ConnectorError::fatal_with_source(
                format!(
                    "Failed to create writer for Delta table: {}",
                    mapping.delta_table_path
                ),
                e,
            )
        })?;

        // Write the record batch
        writer.write(record_batch).await.map_err(|e| {
            ConnectorError::retryable_with_source(
                format!(
                    "Failed to write batch to Delta table: {}",
                    mapping.delta_table_path
                ),
                e,
            )
        })?;

        // Flush and commit the write
        let new_version = writer.flush_and_commit(table).await.map_err(|e| {
            ConnectorError::retryable_with_source(
                format!(
                    "Failed to commit to Delta table: {}",
                    mapping.delta_table_path
                ),
                e,
            )
        })?;

        // CRITICAL: Reload the table to get the latest version
        // The table reference is updated in place by flush_and_commit, but we should
        // reload to ensure we have the latest state for subsequent writes
        table.load().await.map_err(|e| {
            ConnectorError::retryable_with_source(
                format!(
                    "Failed to reload Delta table after commit: {}",
                    mapping.delta_table_path
                ),
                e,
            )
        })?;

        info!(
            "Successfully wrote {} records to Delta table: {} (version: {})",
            records.len(),
            mapping.delta_table_path,
            new_version
        );

        Ok(())
    }
}

/// Convert Arrow DataType to Delta DataType
/// Simplified mapping for commonly used types
fn arrow_to_delta_datatype(arrow_type: &arrow::datatypes::DataType) -> deltalake::kernel::DataType {
    use arrow::datatypes::DataType as ArrowType;
    use arrow::datatypes::TimeUnit;
    use deltalake::kernel::DataType as DeltaType;
    use deltalake::kernel::PrimitiveType;

    match arrow_type {
        ArrowType::Utf8 | ArrowType::LargeUtf8 => DeltaType::Primitive(PrimitiveType::String),
        ArrowType::Int8 => DeltaType::Primitive(PrimitiveType::Byte),
        ArrowType::Int16 => DeltaType::Primitive(PrimitiveType::Short),
        ArrowType::Int32 => DeltaType::Primitive(PrimitiveType::Integer),
        ArrowType::Int64 => DeltaType::Primitive(PrimitiveType::Long),
        ArrowType::UInt8 => DeltaType::Primitive(PrimitiveType::Short), // Promote to signed
        ArrowType::UInt16 => DeltaType::Primitive(PrimitiveType::Integer),
        ArrowType::UInt32 => DeltaType::Primitive(PrimitiveType::Long),
        ArrowType::UInt64 => DeltaType::Primitive(PrimitiveType::Long),
        ArrowType::Float32 => DeltaType::Primitive(PrimitiveType::Float),
        ArrowType::Float64 => DeltaType::Primitive(PrimitiveType::Double),
        ArrowType::Boolean => DeltaType::Primitive(PrimitiveType::Boolean),
        ArrowType::Binary | ArrowType::LargeBinary => DeltaType::Primitive(PrimitiveType::Binary),
        ArrowType::Timestamp(TimeUnit::Microsecond, _) => {
            DeltaType::Primitive(PrimitiveType::Timestamp)
        }
        ArrowType::Timestamp(TimeUnit::Millisecond, _) => {
            DeltaType::Primitive(PrimitiveType::Timestamp)
        }
        ArrowType::Date32 | ArrowType::Date64 => DeltaType::Primitive(PrimitiveType::Date),
        _ => panic!("Unsupported Arrow type for Delta Lake: {:?}", arrow_type),
    }
}

#[async_trait]
impl SinkConnector for DeltaLakeSinkConnector {
    async fn initialize(&mut self, _config: ConnectorConfig) -> ConnectorResult<()> {
        info!("Initializing Delta Lake Sink Connector");
        info!(
            "Connector: {}, Storage Backend: {:?}",
            self.config.core.connector_name, self.config.deltalake.storage_backend
        );

        // Log topic mappings
        for mapping in &self.config.deltalake.topic_mappings {
            let schema_info = mapping
                .expected_schema_subject
                .as_ref()
                .map(|s| format!(", schema: {}", s))
                .unwrap_or_default();
            info!(
                "Topic Mapping: {} -> {} (fields: {}{})",
                mapping.topic,
                mapping.delta_table_path,
                mapping.field_mappings.len(),
                schema_info
            );
        }

        info!("Delta Lake Sink Connector initialized successfully");
        Ok(())
    }

    async fn consumer_configs(&self) -> ConnectorResult<Vec<ConsumerConfig>> {
        let configs = self
            .config
            .deltalake
            .topic_mappings
            .iter()
            .map(|mapping| ConsumerConfig {
                topic: mapping.topic.clone(),
                subscription: mapping.subscription.clone(),
                consumer_name: format!(
                    "{}-{}",
                    self.config.core.connector_name, mapping.subscription
                ),
                subscription_type: SubscriptionType::Shared,
                // Runtime validates schema and provides pre-deserialized data
                expected_schema_subject: mapping.expected_schema_subject.clone(),
            })
            .collect();

        Ok(configs)
    }

    async fn process_batch(&mut self, records: Vec<SinkRecord>) -> ConnectorResult<()> {
        if records.is_empty() {
            return Ok(());
        }

        debug!("process_batch() called with {} records", records.len());

        let mut by_topic: HashMap<String, Vec<SinkRecord>> = HashMap::new();
        for record in records {
            let topic = record.topic().to_string();
            by_topic.entry(topic).or_insert_with(Vec::new).push(record);
        }

        for (topic, topic_records) in by_topic {
            let mapping = self
                .config
                .deltalake
                .topic_mappings
                .iter()
                .find(|m| m.topic == topic)
                .cloned()
                .ok_or_else(|| {
                    ConnectorError::fatal(format!("No mapping found for topic: {}", topic))
                })?;

            self.write_batch(&mapping, topic_records).await?;
        }

        Ok(())
    }

    async fn shutdown(&mut self) -> ConnectorResult<()> {
        info!("Shutting down Delta Lake Sink Connector");

        info!("Delta Lake Sink Connector shutdown complete");
        Ok(())
    }
}
