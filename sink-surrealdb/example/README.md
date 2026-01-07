# SurrealDB Sink Connector Example

Complete working example demonstrating the SurrealDB Sink Connector for streaming real-time events into SurrealDB.

## Overview

This example shows how to:
1. Run Danube broker, SurrealDB, and the connector with Docker Compose
2. Generate sample events and send them to Danube
3. Automatically stream events to SurrealDB tables
4. Query and analyze the data in SurrealDB

## Architecture

```
┌─────────────────┐
│  Test Producer  │
│  (danube-cli)   │
└────────┬────────┘
         │ Events (JSON)
         ▼
┌─────────────────┐     ┌──────────┐
│ Danube Broker   │────▶│   ETCD   │
│  Topic: events  │     │ Metadata │
└────────┬────────┘     └──────────┘
         │ Stream
         ▼
┌─────────────────┐
│ SurrealDB Sink  │
│   Connector     │
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
# Start all services (ETCD, Danube, Topic Init, SurrealDB, Connector)
docker-compose up -d

# Check logs
docker-compose logs -f surrealdb-sink

# Check danube broker
docker-compose logs -f danube-broker

# Verify all services are healthy
docker-compose ps
```

**Startup Sequence:**
1. **ETCD** starts and becomes healthy
2. **Danube Broker** starts (depends on ETCD)
3. **Topic Init** creates `/default/events` topic (depends on Danube)
4. **SurrealDB** starts independently and becomes healthy
5. **SurrealDB Sink** starts (depends on topic creation + SurrealDB health)

Services:
- **ETCD**: `http://localhost:2379` (Danube metadata storage)
- **Danube Broker**: `http://localhost:6650`
- **Danube Admin API**: `http://localhost:50051`
- **Danube Metrics**: `http://localhost:9040/metrics`
- **SurrealDB HTTP/WS**: `http://localhost:8000`
- **Connector Metrics**: `http://localhost:9090/metrics`

### 2. Install danube-cli

Download the latest release for your system from [Danube Releases](https://github.com/danube-messaging/danube/releases):

```bash
# Linux
wget https://github.com/danube-messaging/danube/releases/download/v0.5.2/danube-cli-linux
chmod +x danube-cli-linux

# macOS (Apple Silicon)
wget https://github.com/danube-messaging/danube/releases/download/v0.5.2/danube-cli-macos
chmod +x danube-cli-macos

# Windows
# Download danube-cli-windows.exe from the releases page
```

**Note:** The `test_producer.sh` script automatically detects `danube-cli-linux`, `danube-cli-macos`, or `danube-cli` in the current directory.

**Available platforms:**
- Linux: `danube-cli-linux`
- macOS (Apple Silicon): `danube-cli-macos`
- Windows: `danube-cli-windows.exe`

Or use the Docker image:
```bash
docker pull ghcr.io/danube-messaging/danube-cli:v0.5.2
```

### 3. Send Test Data

```bash
# Send 10 sample events
./test_producer.sh

# Send more events
COUNT=50 ./test_producer.sh

# Custom configuration
DANUBE_URL=http://localhost:6650 \
TOPIC=/default/events \
COUNT=20 \
./test_producer.sh
```

The script generates various event types:
- **user_signup**: New user registrations
- **user_login**: User login events
- **purchase**: E-commerce transactions
- **page_view**: Website page views
- **api_call**: API endpoint calls

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


## Configuration

### Single Topic Configuration

The example uses a single topic mapping in `connector.toml`:

```toml
[[surrealdb.topic_mappings]]
topic = "/default/events"
subscription = "surrealdb-sink-sub"
table_name = "events"
schema_type = "Json"
storage_mode = "Document"
include_danube_metadata = true
```

### Multi-Topic Configuration

To route multiple Danube topics to different SurrealDB tables, add more mappings:

```toml
# User events → users table
[[surrealdb.topic_mappings]]
topic = "/default/users"
subscription = "surrealdb-users"
table_name = "users"
schema_type = "Json"
storage_mode = "Document"

# IoT sensor data → temperature table (time-series)
[[surrealdb.topic_mappings]]
topic = "/iot/temperature"
subscription = "surrealdb-iot"
table_name = "temperature_readings"
schema_type = "Json"
storage_mode = "TimeSeries"  # Adds _timestamp field
batch_size = 500
flush_interval_ms = 2000

# Logs → logs table
[[surrealdb.topic_mappings]]
topic = "/logs/application"
subscription = "surrealdb-logs"
table_name = "app_logs"
schema_type = "String"
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

### Test Different Schema Types

**JSON Schema (Default):**
```bash
# Already configured in connector.toml
./test_producer.sh
```

**String Schema:**
```toml
schema_type = "String"
```
```bash
# Send plain text
echo "Log message" | danube-cli produce -s http://localhost:6650 -t /default/logs
```

**Int64 Schema:**
```toml
schema_type = "Int64"
```
```bash
# Send numbers
echo "42" | danube-cli produce -s http://localhost:6650 -t /default/counters
```

## Troubleshooting

### Connector Not Starting

```bash
# Check logs
docker-compose logs surrealdb-sink

# Common issues:
# 1. Danube not ready - wait for healthcheck
# 2. SurrealDB not ready - wait for healthcheck
# 3. Invalid credentials - check SURREALDB_USERNAME/PASSWORD
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
./test_producer.sh

# Wait for batch flush (default: 1 second)
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

### Optimize Batch Size

For high throughput:

```toml
batch_size = 500
flush_interval_ms = 5000
```

For low latency:

```toml
batch_size = 10
flush_interval_ms = 100
```

### Per-Topic Optimization

```toml
# High-volume topic
[[surrealdb.topic_mappings]]
topic = "/logs/application"
table_name = "logs"
batch_size = 1000
flush_interval_ms = 10000

# Real-time topic
[[surrealdb.topic_mappings]]
topic = "/alerts/critical"
table_name = "alerts"
batch_size = 1
flush_interval_ms = 0
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

## Resources

- [SurrealDB Documentation](https://surrealdb.com/docs)
- [SurrealDB Query Language](https://surrealdb.com/docs/surrealql)
- [Danube Messaging](https://github.com/danube-messaging/danube)
- [Connector Development Guide](../../info/connector-development-guide.md)

## Support

For issues or questions:
- Check [connector logs](../../connectors/sink-surrealdb/README.md#troubleshooting)
- Review [development guide](../../info/connector-development-guide.md)
- Open an issue on GitHub
