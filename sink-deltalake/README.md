# Delta Lake Sink Connector for Danube Connect

Stream events from Danube into [Delta Lake](https://delta.io/) - the open-source storage framework that brings ACID transactions to data lakes. Built entirely in Rust for maximum performance and zero JVM overhead.

**Note:** This connector is optimized for streaming ingestion with append/overwrite operations. It does **not** include the DataFusion query engine, keeping the binary size small and focused on high-performance data ingestion. For complex operations like merge/upsert, consider using Apache Spark or other Delta Lake tools.

## ✨ Features

- ☁️ **Multi-Cloud Support** - AWS S3, Azure Blob Storage, Google Cloud Storage
- 🔒 **ACID Transactions** - Delta Lake transaction log ensures data consistency
- 📋 **User-Defined Schemas** - Full control over table schemas with Arrow data types
- 📊 **Schema-Aware** - Supports JSON, String, Int64, and Bytes schema types
- 🎯 **Multi-Topic Routing** - Route different topics to different Delta tables
- 📦 **Configurable Batching** - Optimize throughput with per-topic batch sizes
- 📝 **Metadata Enrichment** - Optional Danube metadata as JSON column
- ⚡ **Zero-Copy Performance** - Rust-to-Rust with Arrow in-memory format
- 🧪 **MinIO Compatible** - Test locally with S3-compatible storage
- 🛡️ **Production Ready** - Health checks, metrics, graceful shutdown

**Supported Cloud Providers:**
- **AWS S3** - Amazon Simple Storage Service
- **Azure Blob Storage** - Microsoft Azure cloud storage  
- **Google Cloud Storage** - Google Cloud Platform storage

**Use Cases:** Data lake ingestion, real-time analytics, event archival, data warehouse ETL, compliance and audit logs

## 🚀 Quick Start

### Running with Docker

```bash
docker run -d \
  --name deltalake-sink \
  -v $(pwd)/connector.toml:/etc/connector.toml:ro \
  -e CONNECTOR_CONFIG_PATH=/etc/connector.toml \
  -e DANUBE_SERVICE_URL=http://danube-broker:6650 \
  -e CONNECTOR_NAME=deltalake-sink \
  -e AWS_ACCESS_KEY_ID=minioadmin \
  -e AWS_SECRET_ACCESS_KEY=minioadmin \
  danube/sink-deltalake:latest
```

**Note:** All structural configuration (topics, tables, schemas, batching) must be in `connector.toml`. See [Configuration](#configuration) section below.

### Complete Example

For a complete working setup with Docker Compose, MinIO, and test data:

👉 **See [examples/sink-deltalake](../../examples/sink-deltalake/README.md)**

The example includes:
- Docker Compose setup (Danube + ETCD + MinIO)
- Pre-configured connector.toml
- Delta Lake table schema definitions
- Test producers using danube-cli
- MinIO console access for data verification
- Python query examples

## ⚙️ Configuration

### 📖 Complete Configuration Guide

See **[config/README.md](config/README.md)** for comprehensive configuration documentation including:
- Core and connector-specific configuration options
- Cloud provider setup (S3, Azure, GCS)
- Schema definition and Arrow data types
- Environment variable reference
- Configuration patterns and best practices
- Performance tuning guidelines

### 📄 Quick Reference

#### Environment Variables

Environment variables are used **only for secrets and connection URLs**:

| Variable | Description | Use Case |
|----------|-------------|----------|
| `CONNECTOR_CONFIG_PATH` | Path to TOML config file | **Required** |
| `DANUBE_SERVICE_URL` | Danube broker URL | Override for different environments |
| `CONNECTOR_NAME` | Unique connector name | Override for different deployments |
| `AWS_ACCESS_KEY_ID` | AWS/S3 access key | **Secrets** - should not be in config files |
| `AWS_SECRET_ACCESS_KEY` | AWS/S3 secret key | **Secrets** - should not be in config files |
| `AZURE_STORAGE_ACCOUNT_KEY` | Azure storage key | **Secrets** - should not be in config files |
| `GOOGLE_APPLICATION_CREDENTIALS` | GCS service account JSON path | **Secrets** - should not be in config files |

**All other configuration (topics, tables, schemas, batching) must be in the TOML file.**

#### TOML Configuration (Required)

All connector configuration must be defined in a TOML file:

```toml
connector_name = "deltalake-sink"
danube_service_url = "http://localhost:6650"
metrics_port = 9090

[deltalake]
storage_backend = "s3"
s3_region = "us-east-1"
s3_endpoint = "http://localhost:9000"  # MinIO
s3_allow_http = true

batch_size = 1000
flush_interval_ms = 5000

[[deltalake.topic_mappings]]
topic = "/events/payments"
subscription = "deltalake-payments"
delta_table_path = "s3://my-bucket/tables/payments"
schema_type = "Json"
include_danube_metadata = true
schema = [
    { name = "payment_id", data_type = "Utf8", nullable = false },
    { name = "amount", data_type = "Float64", nullable = false },
    { name = "currency", data_type = "Utf8", nullable = false },
]

# Route IoT data to sensors table
[[deltalake.topic_mappings]]
topic = "/iot/sensors"
subscription = "deltalake-iot"
delta_table_path = "s3://my-bucket/tables/sensors"
schema_type = "Json"
batch_size = 500
schema = [
    { name = "sensor_id", data_type = "Utf8", nullable = false },
    { name = "temperature", data_type = "Float64", nullable = false },
    { name = "timestamp", data_type = "Timestamp", nullable = false },
]
```

See [config/README.md](config/README.md) for complete examples and detailed documentation.

## 🛠️ Development

### Building

```bash
# Build release binary
cargo build --release --package danube-sink-deltalake

# Run tests
cargo test --package danube-sink-deltalake

# Build Docker image
docker build -t danube/sink-deltalake:latest -f connectors/sink-deltalake/Dockerfile .
```

### Schema Types

The connector supports Danube's schema system for type-safe data handling:

#### JSON Schema (Default)

Most common for structured data:

```toml
schema_type = "Json"
```

**Input:**
```json
{
  "payment_id": "pay-123",
  "amount": 99.99,
  "currency": "USD"
}
```

**Stored in Delta Lake** with user-defined Arrow schema

#### String Schema

For plain text messages:

```toml
schema_type = "String"
```

**Input:** `"System started successfully"`  
**Stored:** `{"data": "System started successfully"}`

#### Int64 Schema

For numeric counters/metrics:

```toml
schema_type = "Int64"
```

**Input:** `42` (8 bytes, big-endian)  
**Stored:** `{"value": 42}`

#### Bytes Schema

For binary data:

```toml
schema_type = "Bytes"
```

**Input:** Raw bytes `[0x01, 0x02, 0x03]`  
**Stored:** `{"data": "AQID", "size": 3}` (base64 encoded)

### Arrow Data Types

Supported data types for Delta Lake schemas:

- `Utf8` - String
- `Int8`, `Int16`, `Int32`, `Int64` - Signed integers
- `UInt8`, `UInt16`, `UInt32`, `UInt64` - Unsigned integers
- `Float32`, `Float64` - Floating point
- `Boolean` - True/False
- `Timestamp` - Timestamp with microsecond precision
- `Binary` - Binary data

## 📚 Use Cases

### Real-Time Analytics

Stream events into Delta Lake for real-time analytics with Databricks, Apache Spark, or Trino.

### Event Archival

Archive events for long-term storage and compliance with ACID guarantees.

### Data Lake Ingestion

Ingest streaming data into your data lake with automatic schema management.

### Data Warehouse ETL

Load data into Delta Lake as a staging layer for data warehouse pipelines.

## 📈 Performance

- **Throughput**: 10,000+ events/second per instance
- **Latency**: Sub-second write latency with batching
- **Memory**: ~50-100MB RAM (vs 2-4GB for JVM-based solutions)
- **Binary Size**: ~15MB (no DataFusion dependency - append-only operations)
- **Parquet**: Efficient columnar storage with compression

**Note:** This connector uses basic append/overwrite operations without the DataFusion query engine, keeping the binary small and focused on streaming ingestion.

## 🔍 Troubleshooting

### Connection Issues

**Cannot connect to cloud storage:**
- Verify credentials are set in environment variables
- Check network connectivity and firewall rules
- Verify bucket/container exists
- Check IAM/RBAC permissions

### Schema Errors

**Schema mismatch errors:**
- Ensure schema definition matches your data structure
- Check nullable settings for required fields
- Verify Arrow data types are correct
- Test with a small batch first

### Performance Issues

**Slow writes or high latency:**
- Increase `batch_size` for higher throughput
- Adjust `flush_interval_ms` based on latency requirements
- Check network bandwidth to cloud storage
- Monitor Delta Lake transaction log size

## 📖 Documentation

- **[Configuration Guide](config/README.md)** - Complete configuration reference
- [Delta Lake Documentation](https://docs.delta.io/) - Delta Lake official docs
- [Delta Lake Rust API](https://docs.rs/deltalake/) - Rust crate documentation
- [Apache Arrow](https://arrow.apache.org/) - Arrow data format
- [Danube Connect](https://github.com/danrusei/danube-connect) - Main project documentation

## 📄 License

MIT OR Apache-2.0
