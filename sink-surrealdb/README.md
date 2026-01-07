# SurrealDB Sink Connector

Stream events from Danube into [SurrealDB](https://surrealdb.com/) - the ultimate multi-model database for modern applications. Built entirely in Rust for maximum performance and zero JVM overhead.

## ✨ Features

- 🚀 **Multi-Model Support** - Store events as documents or time-series data
- 📊 **Schema-Aware** - Supports JSON, String, Int64, and Bytes schema types
- ⏱️ **Time-Series Optimization** - Automatic timestamp handling for temporal queries
- 🎯 **Multi-Topic Routing** - Route different topics to different tables with independent configurations
- 📦 **Configurable Batching** - Optimize throughput with per-topic batch sizes and flush intervals
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

**Note:** All structural configuration (topics, tables, schema types, batching) must be in `connector.toml`. See [Configuration](#configuration) section below.

### Complete Example

For a complete working setup with Docker Compose, test data, and query examples:

👉 **See [examples/sink-surrealdb](../../examples/sink-surrealdb/README.md)**

The example includes:
- Docker Compose setup (Danube + ETCD + SurrealDB)
- Pre-configured connector.toml
- Test producers using danube-cli
- Query examples and data verification

## ⚙️ Configuration

### 📖 Complete Configuration Guide

See **[config/README.md](config/README.md)** for comprehensive configuration documentation including:
- Core and connector-specific configuration options
- Schema types and storage modes
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
| `SURREALDB_URL` | SurrealDB connection URL | Override for different environments (dev/staging/prod) |
| `SURREALDB_USERNAME` | Database username | **Secrets** - should not be in config files |
| `SURREALDB_PASSWORD` | Database password | **Secrets** - should not be in config files |

**All other configuration (topics, tables, schema types, batching) must be in the TOML file.**

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

batch_size = 100
flush_interval_ms = 1000

# Route user events to user_events table (JSON)
[[surrealdb.topic_mappings]]
topic = "/events/user"
subscription = "surrealdb-user"
table_name = "user_events"
schema_type = "Json"
include_danube_metadata = true
batch_size = 200

# Route IoT data to sensor_readings table (JSON)
[[surrealdb.topic_mappings]]
topic = "/iot/sensors"
subscription = "surrealdb-iot"
table_name = "sensor_readings"
schema_type = "Json"
include_danube_metadata = true
batch_size = 500
flush_interval_ms = 2000
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

### Schema Types

The connector supports Danube's schema system for type-safe data handling:

### JSON Schema (Default)

Most common for structured data:

```toml
schema_type = "Json"
```

**Input:**
```json
{
  "user_id": "user-123",
  "event_type": "login",
  "timestamp": "2024-01-01T12:00:00Z"
}
```

**Stored as-is in SurrealDB**

### String Schema

For plain text messages:

```toml
schema_type = "String"
```

**Input:** `"System started successfully"`

**Stored:** `{"data": "System started successfully"}`

### Int64 Schema

For numeric counters/metrics:

```toml
schema_type = "Int64"
```

**Input:** `42` (8 bytes, big-endian)

**Stored:** `{"value": 42}`

### Bytes Schema

For binary data:

```toml
schema_type = "Bytes"
```

**Input:** Raw bytes `[0x01, 0x02, 0x03]`

**Stored:** `{"data": "AQID", "size": 3}` (base64 encoded)

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
[[surrealdb.topic_mappings]]
topic = "/iot/temperature"
subscription = "surrealdb-iot"
table_name = "temperature_readings"
schema_type = "Json"
storage_mode = "TimeSeries"  # Uses Danube publish_time
batch_size = 500
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

#### Batch Size

Control throughput vs latency:

```toml
# High throughput (batch more records)
batch_size = 500
flush_interval_ms = 5000

# Low latency (flush frequently)
batch_size = 10
flush_interval_ms = 100
```

**Recommendations:**
- **High-volume streams**: `batch_size = 500-1000`
- **Real-time analytics**: `batch_size = 50-100`
- **Transactional data**: `batch_size = 10-20`

#### Connection Protocol

Use WebSocket for better performance:

```toml
url = "ws://surrealdb:8000"  # Recommended
# url = "http://surrealdb:8000"  # HTTP fallback
```

#### Per-Topic Optimization

Different topics can have different performance profiles:

```toml
# Logs: high volume, larger batches
[[surrealdb.topic_mappings]]
topic = "/logs"
batch_size = 1000
flush_interval_ms = 5000

# Orders: low volume, fast flush
[[surrealdb.topic_mappings]]
topic = "/orders"
batch_size = 10
flush_interval_ms = 100
```

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

See **[examples/sink-surrealdb](../../examples/sink-surrealdb)** for a complete setup with:
- Docker Compose (Danube + ETCD + SurrealDB)
- Test producers using danube-cli
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

### References

- [SurrealDB Documentation](https://surrealdb.com/docs)
- [Danube Broker](https://github.com/danrusei/danube)
- [Danube Connect Framework](https://github.com/danrusei/danube-connect)
- [SurrealDB Rust SDK](https://docs.rs/surrealdb/latest/surrealdb/)
