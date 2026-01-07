//! Delta Lake Sink Connector for Danube Connect
//!
//! This connector streams events from Danube topics to Delta Lake tables,
//! providing ACID transactions, schema evolution, and multi-cloud support.
//!
//! # Features
//!
//! - **Multi-Cloud Support**: S3, Azure Blob Storage, Google Cloud Storage
//! - **ACID Transactions**: Guaranteed consistency with Delta Lake transaction log
//! - **User-Defined Schemas**: Full control over table schemas
//! - **Batching**: Configurable batch sizes for optimal performance
//! - **Metadata**: Optional Danube metadata as JSON column
//! - **MinIO Compatible**: Test locally with MinIO S3-compatible storage
//!
//! # Example Configuration
//!
//! ```toml
//! [core]
//! danube_service_url = "http://localhost:6650"
//! connector_name = "deltalake-sink"
//!
//! [deltalake]
//! storage_backend = "s3"
//! s3_region = "us-east-1"
//! s3_endpoint = "http://localhost:9000"  # MinIO
//! s3_allow_http = true
//!
//! [[deltalake.topic_mappings]]
//! topic = "/events/payments"
//! subscription = "deltalake-payments"
//! delta_table_path = "s3://my-bucket/tables/payments"
//! schema_type = "Json"
//! include_danube_metadata = true
//!
//! # Compact inline schema format
//! schema = [
//!     { name = "payment_id", data_type = "Utf8", nullable = false },
//!     { name = "amount", data_type = "Float64", nullable = false },
//! ]
//! ```

pub mod config;
pub mod connector;
pub mod record;

pub use config::DeltaLakeSinkConfig;
pub use connector::DeltaLakeSinkConnector;
