# SurrealDB Sink Connector Example

Complete working example demonstrating the SurrealDB Sink Connector v0.2.0 with **schema validation** for streaming real-time events into SurrealDB.

## Overview

This example shows how to:
1. Register JSON schemas in Danube Schema Registry
2. Run Danube broker, SurrealDB, and the connector with Docker Compose
3. Generate sample events and send them to Danube with schema validation
4. Automatically stream validated events to SurrealDB tables
5. Query and analyze the data in SurrealDB

## Architecture

```
┌─────────────────┐
│  Test Producer  │
│ (Docker Tools)  │
└────────┬────────┘
         │ Events (JSON + Schema)
         ▼
┌─────────────────┐     ┌───────────────┐
│ Danube Broker   │────▶│ Schema        │
│  Topic: events  │     │ Registry      │
│  Embedded Raft  │     │ (events-v1)   │
└────────┬────────┘     └───────────────┘
         │ Validated Stream
         ▼
┌─────────────────┐
│ SurrealDB Sink  │
│   Connector     │
│   v0.2.0        │
└────────┬────────┘
         │ Batch Insert
         ▼
┌─────────────────┐
│   SurrealDB     │
│  Multi-Model DB │
└─────────────────┘
```

## Quick Start

### 1. Start the Stack

```bash
# Start all services (Danube, Topic Init, SurrealDB, Connector)
docker-compose up -d

# Check logs
docker-compose logs -f surrealdb-sink

# Check danube broker
docker-compose logs -f danube-broker

# Verify all services are healthy
docker-compose ps
```

**Startup Sequence:**
1. **Danube Broker** starts as a single-node broker using embedded Raft metadata
2. **Topic Init** (depends on Danube):
   - Registers schema `events-v1` in Schema Registry
   - Creates `/default/events` topic with schema validation
3. **SurrealDB** starts independently and becomes healthy
4. **SurrealDB Sink** starts (depends on topic creation + SurrealDB health)

**Shared Danube broker config:**
- The example mounts `../../example_shared/danube_broker_no_auth.yml`
- Update that single file when Danube broker config changes for all connector examples

Services:
- **Danube Broker**: `http://localhost:6650`
- **Danube Admin API**: `http://localhost:50051`
- **Danube Metrics**: `http://localhost:9040/metrics`
- **SurrealDB HTTP/WS**: `http://localhost:8000`
- **Connector Metrics**: `http://localhost:9090/metrics`
- **Docker Tools Profile**: `test-producer`

### 2. Use the Dockerized Helper Tools

The example no longer requires local Python or `danube-cli` for the main workflow.

On first use, Docker Compose builds a small Python tools image from `Dockerfile.tools` and reuses it for message production.

When overriding helper connection settings, use the Docker service names inside the Compose network:
- Danube: `http://danube-broker:6650`

### 3. Send Test Data

```bash
# Send 10 sample events
docker-compose --profile tools run --rm test-producer

# Send more events
COUNT=50 docker-compose --profile tools run --rm test-producer

# Custom configuration
DANUBE_URL=http://danube-broker:6650 \
TOPIC=/default/events \
COUNT=20 \
docker-compose --profile tools run --rm test-producer
```

The producer generates various event types validated against the `events-v1` schema:
- **user_signup**: New user registrations
- **user_login**: User login events
- **purchase**: E-commerce transactions
- **page_view**: Website page views
- **api_call**: API endpoint calls

**Example event** (matches `events-schema.json`):
```json
{
  "event_id": "evt_1_1704567890_12345",
  "event_type": "purchase",
  "timestamp": "2026-01-08T19:45:00Z",
  "user_id": "user_001",
  "data": {
    "product": "laptop",
    "amount": 850,
    "currency": "USD"
  }
}
```

### 4. Query SurrealDB

**Option 1: Using SurrealDB CLI (from container)**

```bash
# Enter SurrealDB container
docker exec -it surrealdb /surreal sql \
  --endpoint http://localhost:8000 \
  --username root \
  --password root \
  --namespace default \
  --database default

# Run queries
SELECT * FROM events LIMIT 10;
SELECT * FROM events WHERE event_type = 'purchase';
SELECT event_type, COUNT() AS count FROM events GROUP BY event_type;
```

**Option 2: Using HTTP API**

```bash
# Get all events
curl -X POST http://localhost:8000/sql \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json' \
  -u root:root \
  -d '{"query": "SELECT * FROM events LIMIT 10;"}'

# Filter by event type
curl -X POST http://localhost:8000/sql \
  -H 'Content-Type: application/json' \
  -u root:root \
  -d '{"query": "SELECT * FROM events WHERE event_type = '\''purchase'\'';"}'

# Aggregate by event type
curl -X POST http://localhost:8000/sql \
  -H 'Content-Type: application/json' \
  -u root:root \
  -d '{"query": "SELECT event_type, COUNT() AS count FROM events GROUP BY event_type;"}'
```


## Schema Validation

### Event Schema

Messages are validated against `events-schema.json`:

```json
{
  "type": "object",
  "properties": {
    "event_id": {"type": "string"},
    "event_type": {"type": "string"},
    "timestamp": {"type": "string", "format": "date-time"},
    "user_id": {"type": "string"},
    "data": {"type": "object"}
  },
  "required": ["event_type", "timestamp"]
}
```

### How It Works

1. **Schema Registration**: `topic-init` service registers `events-v1` schema
2. **Topic Creation**: Topic created with `--schema-subject events-v1`
3. **Producer Validation**: The Dockerized Python producer references `events-v1` before sending
4. **Runtime Validation**: Broker validates all messages
5. **Automatic Deserialization**: Connector receives pre-validated, deserialized JSON

### Benefits

- ✅ **Data Quality**: Invalid messages rejected at source
- ✅ **Type Safety**: Guaranteed message structure
- ✅ **Early Detection**: Validation errors caught at producer
- ✅ **Simplified Code**: No manual deserialization in connector

## Configuration

### Single Topic Configuration

The example uses a single route in `connector.toml`:

```toml
[[surrealdb.routes]]
from = "/default/events"
subscription = "surrealdb-sink-sub"
subscription_type = "Shared"            # Load balancing
to = "events"
expected_schema_subject = "events-v1"  # Schema validation
storage_mode = "Document"
include_danube_metadata = true
```

### Multi-Topic Configuration

To route multiple Danube topics to different SurrealDB tables, add more routes:

```toml
# User events → users table (with schema validation)
[[surrealdb.routes]]
from = "/default/users"
subscription = "surrealdb-users"
subscription_type = "Shared"
to = "users"
expected_schema_subject = "users-v1"  # Validate against users schema
storage_mode = "Document"

# IoT sensor data → temperature table (time-series with schema)
[[surrealdb.routes]]
from = "/iot/temperature"
subscription = "surrealdb-iot"
subscription_type = "Shared"
to = "temperature_readings"
expected_schema_subject = "sensor-v1"  # Validate sensor data
storage_mode = "TimeSeries"  # Adds _timestamp field

# Logs → logs table (no schema validation for flexibility)
[[surrealdb.routes]]
from = "/logs/application"
subscription = "surrealdb-logs"
subscription_type = "Exclusive"
to = "app_logs"
# expected_schema_subject not set - accepts any data
storage_mode = "Document"
```

### Storage Modes

**Document Mode (Default):**
```toml
storage_mode = "Document"
```
- Regular document storage
- Best for: User events, transactions, general data

**TimeSeries Mode:**
```toml
storage_mode = "TimeSeries"
```
- Automatically adds `_timestamp` field using Danube publish time
- Best for: IoT data, metrics, logs, sensor readings
- Optimized for temporal queries

### Environment Variable Overrides

Edit `docker-compose.yml` to override settings:

```yaml
environment:
  # Core settings
  - DANUBE_SERVICE_URL=http://danube-broker:6650
  - CONNECTOR_NAME=surrealdb-sink-prod
  
  # SurrealDB connection
  - SURREALDB_URL=ws://surrealdb:8000
  
  # Secrets (don't put in TOML)
  - SURREALDB_USERNAME=root
  - SURREALDB_PASSWORD=${DB_PASSWORD}
```

## Monitoring

### Connector Metrics

```bash
# View Prometheus metrics
curl http://localhost:9090/metrics

# Key metrics
curl http://localhost:9090/metrics | grep danube_connector
```

### Logs

```bash
# All services
docker-compose logs -f

# Just connector
docker-compose logs -f surrealdb-sink

# SurrealDB logs
docker-compose logs -f surrealdb
```


## Verifying the Connector

### Check Data Flow

```bash
# 1. Verify messages are in Danube
docker-compose logs danube-broker | grep produce

# 2. Verify connector is processing
docker-compose logs surrealdb-sink | grep "Flushing batch"

# 3. Check SurrealDB has received records
curl -X POST http://localhost:8000/sql \
  -u root:root \
  -d '{"query": "SELECT count() FROM events;"}' | jq
```

### Verify Schema Validation

**Check registered schemas:**
```bash
# List all schemas
docker exec surrealdb-topic-init danube-admin-cli schemas list

# Get schema details
docker exec surrealdb-topic-init danube-admin-cli schemas get events-v1
```

**Check topic schema:**
```bash
# Describe topic to see schema-subject
docker exec surrealdb-topic-init danube-admin-cli topics describe /default/events
```

**Test schema validation:**
```bash
# Valid message (passes validation)
COUNT=1 RAW_MESSAGE='{"event_type":"test","timestamp":"2026-01-08T19:45:00Z"}' \
docker-compose --profile tools run --rm test-producer

# Invalid message (fails validation - missing required field)
COUNT=1 RAW_MESSAGE='{"event_type":"test"}' \
docker-compose --profile tools run --rm test-producer
```

## Troubleshooting

### Schema Validation Errors

**Error:** `Schema validation failed`

**Solution:**
1. Check schema is registered:
```bash
docker exec surrealdb-topic-init danube-admin-cli schemas list
```

2. Verify message matches schema:
```bash
# Required fields for events-v1:
# - event_type (string)
# - timestamp (date-time string)
```

3. Test with valid message:
```bash
COUNT=1 RAW_MESSAGE='{"event_type":"test","timestamp":"2026-01-08T19:45:00Z"}' \
docker-compose --profile tools run --rm test-producer
```

### Connector Not Starting

```bash
# Check logs
docker-compose logs surrealdb-sink

# Common issues:
# 1. Topic not created - check topic-init logs
# 2. Schema not registered - check topic-init logs
# 3. Danube not ready - wait for healthcheck
# 4. SurrealDB not ready - wait for healthcheck
# 5. Invalid credentials - check SURREALDB_USERNAME/PASSWORD
```

### No Data in SurrealDB

```bash
# Verify data was sent
docker-compose logs surrealdb-sink | grep "Successfully flushed"

# Check record count
curl -X POST http://localhost:8000/sql \
  -u root:root \
  -d '{"query": "SELECT count() FROM events;"}'

# If zero, resend data:
docker-compose --profile tools run --rm test-producer

# Wait for runtime processing (default batch timeout from [processing])
sleep 2

# Try query again
```

### Authentication Errors

**Error:** `Authentication failed`

**Solution:**
1. Verify credentials in docker-compose.yml match SurrealDB
2. Default credentials: `root` / `root`
3. Update environment variables:
```yaml
- SURREALDB_USERNAME=root
- SURREALDB_PASSWORD=root
```

### Table Not Found

```bash
# List all tables
curl -X POST http://localhost:8000/sql \
  -u root:root \
  -d '{"query": "INFO FOR DB;"}'

# If table doesn't exist, check:
# 1. Connector initialized successfully
docker-compose logs surrealdb-sink | grep "initialized"

# 2. Messages were sent
docker-compose logs surrealdb-sink | grep "Flushing"
```

## Performance Tips

### Tune Runtime Processing

For high throughput:

```toml
[processing]
batch_size = 500
batch_timeout_ms = 5000
```

For low latency:

```toml
[processing]
batch_size = 10
batch_timeout_ms = 100
```

## Cleanup

```bash
# Stop all services
docker-compose down

# Remove volumes (deletes all data)
docker-compose down -v

# Remove everything including images
docker-compose down -v --rmi all
```

## What Gets Stored in SurrealDB

With `include_danube_metadata = true`, records include Danube metadata:

```json
{
  "event_id": "evt_1_1704567890_12345",
  "event_type": "purchase",
  "timestamp": "2026-01-08T19:45:00Z",
  "user_id": "user_001",
  "data": {
    "product": "laptop",
    "amount": 850,
    "currency": "USD"
  },
  "_danube_metadata": {
    "danube_topic": "/default/events",
    "danube_timestamp": "2026-01-08T19:45:23.456789Z",
    "danube_producer": "example-producer"
  }
}
```

**With TimeSeries mode**, an additional `_timestamp` field is added:
```json
{
  "_timestamp": "2026-01-08T19:45:23.456789Z",
  ...
}
```

## Resources

- [SurrealDB Documentation](https://surrealdb.com/docs)
- [SurrealDB Query Language](https://surrealdb.com/docs/surrealql)
- [Danube Messaging](https://github.com/danube-messaging/danube)
- [Connector Configuration Guide](../config/README.md)

## Support

For issues or questions:
- Check [connector documentation](../README.md)
- Review [configuration guide](../config/README.md)
- Open an issue on GitHub
