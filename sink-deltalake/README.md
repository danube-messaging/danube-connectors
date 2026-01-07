# Delta Lake Sink Connector for Danube Connect

Stream events from Danube into [Delta Lake](https://delta.io/) - the open-source storage framework that brings ACID transactions to data lakes. Built entirely in Rust for maximum performance and zero JVM overhead.

**Note:** This connector is optimized for streaming ingestion with append/overwrite operations. It does **not** include the DataFusion query engine, keeping the binary size small and focused on high-performance data ingestion. For complex operations like merge/upsert, consider using Apache Spark or other Delta Lake tools.

## ✨ Features

- ☁️ **Multi-Cloud Support** - AWS S3, Azure Blob Storage, Google Cloud Storage
- 🔒 **ACID Transactions** - Delta Lake transaction log ensures data consistency
- 🛡️ **Schema Validation** - Automatic validation with schema registry integration
- 📊 **Type-Safe Processing** - Runtime deserializes and validates message payloads
- 🗺️ **Nested JSON Paths** - Extract values from deeply nested structures
- 🎯 **Multi-Topic Routing** - Route different topics to different Delta tables
- 📦 **Configurable Batching** - Optimize throughput with per-topic batch sizes
- 📝 **Metadata Enrichment** - Optional Danube metadata as JSON column
- ⚡ **Optimized Performance** - Pre-split JSON paths, arrow-json conversion
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

👉 **See [example/README.md](example/README.md)**

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
- Schema validation and field mappings
- Nested JSON path extraction
- Arrow data types reference
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
include_danube_metadata = true

# Schema validation (schema created by producer/admin)
expected_schema_subject = "payment-events-v1"

# Field mappings: JSON path → Delta Lake column
field_mappings = [
    { json_path = "payment_id", column = "payment_id", data_type = "Utf8", nullable = false },
    { json_path = "amount", column = "amount", data_type = "Float64", nullable = false },
    { json_path = "currency", column = "currency", data_type = "Utf8", nullable = false },
]

# Route IoT data with nested JSON to sensors table
[[deltalake.topic_mappings]]
topic = "/iot/sensors"
subscription = "deltalake-iot"
delta_table_path = "s3://my-bucket/tables/sensors"
batch_size = 500

# Schema validation
expected_schema_subject = "sensor-data-v1"

# Extract nested fields using JSON paths
field_mappings = [
    { json_path = "device.sensor_id", column = "sensor_id", data_type = "Utf8", nullable = false },
    { json_path = "readings.temperature", column = "temperature", data_type = "Float64", nullable = false },
    { json_path = "timestamp", column = "timestamp", data_type = "Timestamp", nullable = false },
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

### Schema Validation

**Important:** Schemas are created by **producers** or **danube-admin-cli** and attached to topics. The sink connector validates incoming messages against these schemas.

#### Automatic Validation

When you specify `expected_schema_subject`, the runtime automatically:
- Validates incoming messages match the schema
- Deserializes payloads into type-safe `serde_json::Value`
- Provides pre-parsed data to the connector

```toml
[[deltalake.topic_mappings]]
topic = "/events/payments"
expected_schema_subject = "payment-events-v1"  # Schema created by producer
field_mappings = [...]
```

**Benefits:**
- Type-safe processing
- Schema evolution support
- Automatic deserialization
- No manual parsing needed

#### Field Mappings with JSON Paths

Extract values from JSON using dot-notation paths:

```toml
field_mappings = [
    # Simple field
    { json_path = "payment_id", column = "payment_id", data_type = "Utf8", nullable = false },
    
    # Nested field (user.profile.email)
    { json_path = "user.profile.email", column = "user_email", data_type = "Utf8", nullable = false },
    
    # Deep nesting (device.sensors.temperature.reading)
    { json_path = "device.sensors.temperature.reading", column = "temp", data_type = "Float64", nullable = false },
]
```

**Example Input:**
```json
{
  "payment_id": "pay-123",
  "amount": 99.99,
  "user": {
    "profile": {
      "email": "user@example.com"
    }
  },
  "device": {
    "sensors": {
      "temperature": {
        "reading": 72.5
      }
    }
  }
}
```

**Stored in Delta Lake:**
| payment_id | amount | user_email | temp |
|------------|--------|------------|------|
| pay-123 | 99.99 | user@example.com | 72.5 |

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

**Optimizations:**
- **Pre-split JSON paths**: Paths cached on config load (99.99% reduction in allocations)
- **arrow-json integration**: Efficient type conversion with proper null handling
- **Delta-rs TryFrom**: Native Arrow → Delta type conversion
- **Batch processing**: Configurable batching reduces write overhead

**Note:** This connector uses basic append/overwrite operations without the DataFusion query engine, keeping the binary small and focused on streaming ingestion.

## 🔍 Troubleshooting

### Connection Issues

**Cannot connect to cloud storage:**
- Verify credentials are set in environment variables
- Check network connectivity and firewall rules
- Verify bucket/container exists
- Check IAM/RBAC permissions

### Schema Validation Errors

**Schema validation failures:**
- Verify `expected_schema_subject` matches the schema attached to the topic
- Ensure producer/admin created the schema before starting connector
- Check field mappings match the incoming JSON structure
- Verify JSON paths are correct (e.g., `"user.profile.email"` for nested fields)
- Check nullable settings for required fields
- Verify Arrow data types match your data
- Test with a small batch first
- Use danube-admin-cli to inspect topic schema

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
