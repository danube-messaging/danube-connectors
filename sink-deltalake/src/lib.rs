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
//! [[deltalake.routes]]
//! from = "/events/payments"
//! subscription = "deltalake-payments"
//! to = "s3://my-bucket/tables/payments"
//! include_danube_metadata = true
//!
//! # Field mappings from JSON payload to Delta Lake columns
//! field_mappings = [
//!     { json_path = "payment_id", column = "payment_id", data_type = "Utf8", nullable = false },
//!     { json_path = "amount", column = "amount", data_type = "Float64", nullable = false },
//! ]
//! ```

pub mod config;
pub mod connector;
pub mod record;

pub use config::DeltaLakeSinkConfig;
pub use connector::DeltaLakeSinkConnector;
