# SurrealDB Sink Connector Configuration

Configuration reference for v0.2.0 with schema validation support.

## Table of Contents

- [Quick Start](#quick-start)
- [Core Settings](#core-settings)
- [SurrealDB Connection](#surrealdb-connection)
- [Topic Mappings](#topic-mappings)
- [Storage Modes](#storage-modes)
- [Environment Variables](#environment-variables)
- [Examples](#examples)

## Quick Start

```toml
# Minimal configuration
connector_name = "surrealdb-sink"
danube_service_url = "http://localhost:6650"

[surrealdb]
url = "ws://localhost:8000"
namespace = "default"
database = "default"

[[surrealdb.topic_mappings]]
topic = "/default/events"
subscription = "surrealdb-sink"
subscription_type = "Shared"
table_name = "events"
expected_schema_subject = "events-v1"  # Schema validation
storage_mode = "Document"
```

**Run:**
```bash
CONNECTOR_CONFIG_PATH=/path/to/connector.toml danube-sink-surrealdb
```

## Core Settings

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `connector_name` | string | No | "surrealdb-sink" | Connector instance name |
| `danube_service_url` | string | Yes | - | Danube broker URL |
| `metrics_port` | integer | No | 9090 | Prometheus metrics port |

**Example:**
```toml
connector_name = "surrealdb-sink-production"
danube_service_url = "http://localhost:6650"
metrics_port = 9090
```

**Environment overrides:**

| Variable | TOML Key | Purpose |
|----------|----------|---------|
| `CONNECTOR_NAME` | `connector_name` | Override connector name |
| `DANUBE_SERVICE_URL` | `danube_service_url` | Override broker URL |

## SurrealDB Connection

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `url` | string | Yes | - | Connection URL (`ws://` or `http://`) |
| `namespace` | string | Yes | - | SurrealDB namespace |
| `database` | string | Yes | - | SurrealDB database |
| `username` | string | No | - | Authentication username |
| `password` | string | No | - | Authentication password |
| `connection_timeout_secs` | integer | No | 30 | Connection timeout |
| `request_timeout_secs` | integer | No | 30 | Request timeout |
| `batch_size` | integer | No | 100 | Global batch size |
| `flush_interval_ms` | integer | No | 1000 | Global flush interval |

**Example:**
```toml
[surrealdb]
url = "ws://localhost:8000"  # ws:// recommended for performance
namespace = "production"
database = "events"
username = "admin"  # Optional
password = "secret"  # Optional
connection_timeout_secs = 30
request_timeout_secs = 30
batch_size = 100
flush_interval_ms = 1000
```

**Environment overrides:**

| Variable | TOML Key | Purpose |
|----------|----------|----------|
| `SURREALDB_URL` | `surrealdb.url` | Override connection URL |
| `SURREALDB_USERNAME` | `surrealdb.username` | Set username (secret) |
| `SURREALDB_PASSWORD` | `surrealdb.password` | Set password (secret) |

## Topic Mappings

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `topic` | string | Yes | - | Danube topic to consume |
| `subscription` | string | Yes | - | Subscription name |
| `subscription_type` | string | No | "Shared" | Subscription type: `Shared`, `Exclusive`, `FailOver` |
| `table_name` | string | Yes | - | SurrealDB table name |
| `expected_schema_subject` | string | No | - | Schema validation (e.g., `events-v1`) |
| `storage_mode` | string | No | "Document" | Storage mode: `Document` or `TimeSeries` |
| `include_danube_metadata` | boolean | No | true | Add `_danube_metadata` field |
| `batch_size` | integer | No | global | Override batch size for this topic |
| `flush_interval_ms` | integer | No | global | Override flush interval for this topic |

**Basic mapping:**
```toml
[[surrealdb.topic_mappings]]
topic = "/events/user"
subscription = "surrealdb-user-events"
subscription_type = "Shared"
table_name = "user_events"
```

**With schema validation (v0.2.0):**
```toml
[[surrealdb.topic_mappings]]
topic = "/events/user"
subscription = "surrealdb-user"
subscription_type = "Shared"
table_name = "user_events"
expected_schema_subject = "user-events-v1"  # Enables validation
storage_mode = "Document"
include_danube_metadata = true
```

**Record IDs:**
- Producer sets `record_id` attribute → `table_name:record_id`
- No attribute set → Auto-generated UUID

## Storage Modes

### Document Mode (Default)

Stores data as-is:

```toml
storage_mode = "Document"
```

**Result:**
```json
{"user_id": "123", "action": "login"}
```

### TimeSeries Mode

Adds automatic `_timestamp` field:

```toml
storage_mode = "TimeSeries"
```

**Result:**
```json
{
  "_timestamp": "2026-01-08T19:45:23.456789Z",
  "user_id": "123",
  "action": "login"
}
```

**Use for:** IoT data, logs, metrics, events


## Environment Variables

**Required:**

| Variable | Purpose |
|----------|----------|
| `CONNECTOR_CONFIG_PATH` | Path to TOML config |

**Optional (secrets & URLs):**

| Variable | Purpose |
|----------|----------|
| `DANUBE_SERVICE_URL` | Override Danube broker URL |
| `CONNECTOR_NAME` | Override connector name |
| `SURREALDB_URL` | Override SurrealDB URL |
| `SURREALDB_USERNAME` | Database username |
| `SURREALDB_PASSWORD` | Database password |

**Docker example:**

```yaml
surrealdb-sink:
  volumes:
    - ./connector.toml:/etc/connector.toml:ro
  environment:
    - CONNECTOR_CONFIG_PATH=/etc/connector.toml
    - DANUBE_SERVICE_URL=http://danube-broker:6650
    - SURREALDB_URL=ws://surrealdb:8000
    - SURREALDB_USERNAME=${DB_USER}
    - SURREALDB_PASSWORD=${DB_PASS}
```

## Examples

### Example 1: Schema Validation

```toml
connector_name = "events-sink"
danube_service_url = "http://localhost:6650"

[surrealdb]
url = "ws://localhost:8000"
namespace = "app"
database = "events"

[[surrealdb.topic_mappings]]
topic = "/events/user"
subscription = "surrealdb-events"
subscription_type = "Shared"
table_name = "user_events"
expected_schema_subject = "user-events-v1"  # Schema validation
storage_mode = "Document"
```

### Example 2: IoT Time-Series

```toml
connector_name = "iot-sink"
danube_service_url = "http://localhost:6650"

[surrealdb]
url = "ws://localhost:8000"
namespace = "iot"
database = "sensors"
batch_size = 500
flush_interval_ms = 2000

[[surrealdb.topic_mappings]]
topic = "/iot/temperature"
subscription = "surrealdb-iot"
subscription_type = "Shared"
table_name = "temperature_readings"
expected_schema_subject = "sensor-v1"
storage_mode = "TimeSeries"  # Adds _timestamp field
batch_size = 1000
```

### Example 3: Multi-Topic

```toml
connector_name = "multi-sink"
danube_service_url = "http://localhost:6650"

[surrealdb]
url = "ws://localhost:8000"
namespace = "app"
database = "main"

# User events with schema
[[surrealdb.topic_mappings]]
topic = "/events/user"
subscription = "surrealdb-user"
subscription_type = "Shared"
table_name = "user_events"
expected_schema_subject = "user-v1"
storage_mode = "Document"

# IoT data as time-series
[[surrealdb.topic_mappings]]
topic = "/iot/sensors"
subscription = "surrealdb-iot"
subscription_type = "Shared"
table_name = "sensor_readings"
expected_schema_subject = "sensor-v1"
storage_mode = "TimeSeries"
batch_size = 500

# Logs without schema validation
[[surrealdb.topic_mappings]]
topic = "/logs/app"
subscription = "surrealdb-logs"
subscription_type = "Exclusive"
table_name = "app_logs"
storage_mode = "TimeSeries"
include_danube_metadata = false
```

## More Information

- [Example Setup](../example/README.md) - Complete Docker Compose example
- [Connector README](../README.md) - Architecture and troubleshooting
