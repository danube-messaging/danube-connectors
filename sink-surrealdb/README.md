# SurrealDB Sink Connector

Stream events from Danube into [SurrealDB](https://surrealdb.com/) - the ultimate multi-model database for modern applications. Built entirely in Rust for maximum performance and zero JVM overhead.

## ✨ Features

- 🔒 **Schema Validation** - Validate messages against registered JSON schemas
- 🚀 **Multi-Model Support** - Store events as documents or time-series data
- ⏱️ **Time-Series Optimization** - Automatic timestamp handling for temporal queries
- 🎯 **Multi-Topic Routing** - Route different topics to different tables with independent configurations
- 📦 **Runtime-Managed Batching** - Use shared core processing settings for throughput and latency tuning
- 🔄 **Subscription Types** - Shared, Exclusive, or FailOver subscription modes
- 🔑 **Custom Record IDs** - Use message attributes for idempotent inserts or auto-generate
- 📝 **Metadata Enrichment** - Optionally include Danube metadata (topic, offset, timestamp)
- ⚡ **Zero-Copy Performance** - Rust-to-Rust with WebSocket protocol
- 🛡️ **Production Ready** - Health checks, metrics, graceful shutdown

**Supported Storage Modes:**
- **Document** - Regular document storage (default)
- **TimeSeries** - Time-series data with automatic timestamp handling

**Use Cases:** Real-time analytics, event sourcing, time-series data, document storage, operational databases

## 🚀 Quick Start

### Running with Docker

```bash
docker run -d \
  --name surrealdb-sink \
  -v $(pwd)/connector.toml:/etc/connector.toml:ro \
  -e CONNECTOR_CONFIG_PATH=/etc/connector.toml \
  -e DANUBE_SERVICE_URL=http://danube-broker:6650 \
  -e CONNECTOR_NAME=surrealdb-sink \
  -e SURREALDB_URL=ws://surrealdb:8000 \
  -e SURREALDB_USERNAME=root \
  -e SURREALDB_PASSWORD=root \
  danube/sink-surrealdb:latest
```

**Note:** All structural configuration (topics, tables, schema validation, storage mode) must be in `connector.toml`. See [Configuration](#configuration) section below.

### Complete Example

For a complete working setup with Docker Compose, test data, and query examples:

👉 **See [sink-surrealdb example](example/README.md)**

The example includes:
- Docker Compose setup (Danube + ETCD + SurrealDB + Schema Registry)
- Schema registration and validation
- Pre-configured connector.toml with v0.2.0 features
- Test producers using danube-cli
- Query examples and data verification

## ⚙️ Configuration

### 📖 Complete Configuration Guide

See **[config/README.md](config/README.md)** for comprehensive configuration documentation including:
- Core and connector-specific configuration options
- Schema validation setup (v0.2.0)
- Subscription types and storage modes
- Environment variable reference
- Configuration examples

### 📄 Quick Reference

#### Environment Variables

Environment variables are used **only for secrets and connection URLs**:

| Variable | Description | Use Case |
|----------|-------------|----------|
| `CONNECTOR_CONFIG_PATH` | Path to TOML config file | **Required** |
| `DANUBE_SERVICE_URL` | Danube broker URL | Override for different environments |
| `CONNECTOR_NAME` | Unique connector name | Override for different deployments |
| `SURREALDB_URL` | SurrealDB connection URL | Override for different environments (dev/staging/prod) |
| `SURREALDB_USERNAME` | Database username | **Secrets** - should not be in config files |
| `SURREALDB_PASSWORD` | Database password | **Secrets** - should not be in config files |

**All other connector configuration (topics, tables, schema validation, storage mode) must be in the TOML file.**

#### TOML Configuration (Required)

All connector configuration must be defined in a TOML file:

```toml
connector_name = "surrealdb-sink"
danube_service_url = "http://localhost:6650"
metrics_port = 9090

[surrealdb]
url = "ws://localhost:8000"
namespace = "production"
database = "events"
username = "admin"
password = "password"

# Route user events to user_events table with schema validation
[[surrealdb.routes]]
from = "/events/user"
subscription = "surrealdb-user"
subscription_type = "Shared"
to = "user_events"
expected_schema_subject = "user-events-v1"  # Schema validation
storage_mode = "Document"
include_danube_metadata = true

# Route IoT data to sensor_readings table (time-series)
[[surrealdb.routes]]
from = "/iot/sensors"
subscription = "surrealdb-iot"
subscription_type = "Shared"
to = "sensor_readings"
expected_schema_subject = "sensor-v1"  # Schema validation
storage_mode = "TimeSeries"  # Adds _timestamp field
include_danube_metadata = true
```

See [config/README.md](config/README.md) for complete examples and detailed documentation.

## 🛠️ Development

### Building

```bash
# Build release binary
cargo build --release

# Run tests
cargo test

# Build Docker image
docker build -t danube/sink-surrealdb:latest .
```

### Schema Validation

Validate messages against registered JSON schemas:

```toml
[[surrealdb.routes]]
from = "/events/user"
subscription = "surrealdb-user"
subscription_type = "Shared"
to = "user_events"
expected_schema_subject = "user-events-v1"  # Enable validation
storage_mode = "Document"
```

**Benefits:**
- ✅ **Data Quality** - Invalid messages rejected at source
- ✅ **Type Safety** - Guaranteed message structure
- ✅ **Early Detection** - Validation errors caught at producer
- ✅ **Automatic Deserialization** - Runtime handles JSON parsing

**How it works:**
1. Register schema in Danube Schema Registry
2. Create topic with `--schema-subject user-events-v1`
3. Producer validates messages before sending
4. Connector receives pre-validated, deserialized JSON

**Example Schema (`user-events-v1`):**
```json
{
  "type": "object",
  "properties": {
    "user_id": {"type": "string"},
    "event_type": {"type": "string"},
    "timestamp": {"type": "string", "format": "date-time"}
  },
  "required": ["user_id", "event_type"]
}
```

**Stored in SurrealDB:**
```json
{
  "user_id": "user-123",
  "event_type": "login",
  "timestamp": "2024-01-01T12:00:00Z"
}
```

### Storage Modes

The connector supports two storage modes for different use cases:

### Document Mode (Default)

Regular document storage for general-purpose data:

```toml
storage_mode = "Document"
```

**Use for:**
- User events
- Application logs
- General JSON documents
- Unstructured data

### TimeSeries Mode

Optimized for time-series data with automatic timestamp handling:

```toml
storage_mode = "TimeSeries"
```

**Features:**
- Adds `_timestamp` field to every record
- Uses Danube `publish_time` (set when message was published)
- Optimized for temporal queries
- No payload parsing required

**Example Configuration:**

```toml
[[surrealdb.routes]]
from = "/iot/temperature"
subscription = "surrealdb-iot"
subscription_type = "Shared"
to = "temperature_readings"
expected_schema_subject = "sensor-v1"
storage_mode = "TimeSeries"  # Uses Danube publish_time
```

**Input Payload:**
```json
{
  "sensor_id": "sensor-001",
  "temperature": 23.5
}
```

**Stored in SurrealDB:**
```json
{
  "sensor_id": "sensor-001",
  "temperature": 23.5,
  "_timestamp": "2024-01-01T12:00:00.123Z"
}
```

**Timestamp Source:**
- Uses Danube `publish_time` (microseconds since epoch)
- Set automatically when message is published to Danube
- Converted to RFC3339 format for storage

**Use for:**
- IoT sensor data
- Metrics and monitoring
- System logs with timestamps
- Financial tick data

### Metadata Enrichment

Optionally include Danube metadata for audit trails:

```toml
include_danube_metadata = true
```

**Adds metadata to each record:**

```json
{
  "user_id": "user-123",
  "event_type": "login",
  "_danube_metadata": {
    "danube_topic": "/events/user",
    "danube_offset": 12345,
    "danube_timestamp": "2024-01-01T12:00:01Z",
    "danube_message_id": "topic:/events/user/producer:1/offset:12345"
  }
}
```

**Benefits:**
- Message traceability
- Event ordering
- Debugging
- Data lineage


### Record ID Management

### Auto-Generated IDs (Default)

SurrealDB generates unique IDs automatically:

```rust
// Producer doesn't set record_id attribute
producer.send(payload, None).await?;
```

**Result:** `events:⟨uuid⟩`

### Custom Record IDs (Idempotent)

Set `record_id` attribute in the producer:

```rust
let mut attributes = HashMap::new();
attributes.insert("record_id".to_string(), "order-12345".to_string());
producer.send(payload, Some(attributes)).await?;
```

**Result:** `orders:order-12345`

**Benefits:**
- Idempotent inserts (reprocessing updates existing record)
- Predictable record IDs
- Easy lookups
- Natural keys from source systems

### Performance Tuning

#### Connection Protocol

Use WebSocket for better performance:

```toml
url = "ws://surrealdb:8000"  # Recommended
# url = "http://surrealdb:8000"  # HTTP fallback
```

Runtime throughput and latency tuning is handled by the shared core processing settings rather than SurrealDB-specific batch fields.

### Monitoring

#### Prometheus Metrics

The connector exposes metrics on port `9090` (configurable):

```bash
curl http://localhost:9090/metrics
```

**Key Metrics:**
- `danube_connector_records_processed_total` - Total records processed
- `danube_connector_batches_flushed_total` - Total batches flushed
- `danube_connector_errors_total` - Total errors encountered
- `danube_connector_batch_size` - Current batch size histogram
- `danube_connector_flush_duration_seconds` - Flush duration histogram

#### Health Checks

Check connector health:

```bash
# Via metrics endpoint
curl http://localhost:9090/health

# Via logs
docker logs -f surrealdb-sink
```

## 📚 Documentation

### Complete Working Example

See **[example/README.md](example/README.md)** for a complete setup with:
- Docker Compose (Danube + ETCD + SurrealDB + Schema Registry)
- Schema registration and validation
- Test producers using danube-cli v0.6.1+
- Single and multi-topic configurations
- Query examples and monitoring

### Configuration Examples

- **[config/connector.toml](config/connector.toml)** - Fully documented reference configuration
- **[config/README.md](config/README.md)** - Complete configuration guide

### Architecture

```
┌─────────────────┐
│ Danube Broker   │
│  Topic: /events │
└────────┬────────┘
         │ Stream messages
         ▼
┌─────────────────┐
│ SurrealDB Sink  │
│   Connector     │
│  - Batch        │
│  - Transform    │
│  - Route        │
└────────┬────────┘
         │ Insert records
         ▼
┌─────────────────┐
│   SurrealDB     │
│  Multi-Model DB │
│  - Documents    │
│  - Graphs       │
│  - Time-Series  │
└─────────────────┘
```

## 🔧 Troubleshooting

### Schema Validation Errors

**Error:** `Schema validation failed`

**Solution:**
- Check schema is registered: `danube-admin-cli schemas list`
- Verify message matches schema requirements
- Ensure topic has correct `schema-subject` configured

### Connection Issues

**Error:** `Failed to connect to SurrealDB`

**Solution:**
- Verify SurrealDB is running: `curl http://localhost:8000/health`
- Check connection URL format: `ws://host:port` or `http://host:port`
- Verify credentials if authentication is enabled

### No Data in SurrealDB

**Solution:**
- Check connector logs: `docker logs surrealdb-sink`
- Verify messages are being sent to Danube
- Check batch settings - data may be buffered
- Confirm table name and namespace/database settings

## 📚 References

- [SurrealDB Documentation](https://surrealdb.com/docs)
- [Danube Messaging](https://github.com/danube-messaging/danube)
- [Configuration Guide](config/README.md)
- [Example Setup](example/README.md)
