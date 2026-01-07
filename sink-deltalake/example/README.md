# Delta Lake Sink Connector Example

Complete working example demonstrating the Delta Lake Sink Connector for streaming real-time events into Delta Lake tables with ACID guarantees.

## Overview

This example shows how to:
1. Run Danube broker, MinIO (S3-compatible storage), and the connector with Docker Compose
2. Generate sample events and send them to Danube
3. Automatically stream events to Delta Lake tables
4. Query and analyze the data using Python or Spark

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
│ Delta Lake Sink │
│   Connector     │
└────────┬────────┘
         │ Write Parquet + Transaction Log
         ▼
┌─────────────────┐
│     MinIO       │
│ S3-Compatible   │
│  Object Storage │
└─────────────────┘
```

## Quick Start

### 1. Start the Stack

```bash
# Start all services (ETCD, Danube, Topic Init, MinIO, Connector)
docker-compose up -d

# Check logs
docker-compose logs -f deltalake-sink

# Verify all services are healthy
docker-compose ps
```

**Startup Sequence:**
1. **ETCD** starts and becomes healthy
2. **Danube Broker** starts (depends on ETCD)
3. **Topic Init** creates `/default/events` topic (depends on Danube)
4. **MinIO** starts and becomes healthy
5. **MinIO Init** creates `delta-tables` bucket (depends on MinIO)
6. **Delta Lake Sink** starts (depends on topic creation + MinIO bucket)

Services:
- **ETCD**: `http://localhost:2379` (Danube metadata storage)
- **Danube Broker**: `http://localhost:6650`
- **Danube Admin API**: `http://localhost:50051`
- **Danube Metrics**: `http://localhost:9040/metrics`
- **MinIO API**: `http://localhost:9000`
- **MinIO Console**: `http://localhost:9001` (minioadmin/minioadmin)
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

# Send more events with custom settings
COUNT=100 INTERVAL=100 ./test_producer.sh

# Or use danube-cli directly
./danube-cli-linux produce \
  --service-addr http://localhost:6650 \
  --topic /default/events \
  --message '{"event_type":"purchase","user_id":"user_001","product":"laptop","amount":999,"currency":"USD","timestamp":"2024-01-01T12:00:00Z"}' \
  --reliable
```

The test script generates various event types:
- **user_signup** - New user registrations
- **user_login** - User login events
- **purchase** - E-commerce transactions
- **page_view** - Website page views
- **api_call** - API endpoint calls

### 4. Verify Data in Delta Lake

#### Option 1: MinIO Console (Visual)

1. Open http://localhost:9001
2. Login with `minioadmin` / `minioadmin`
3. Navigate to `delta-tables` bucket
4. Browse `events/` folder to see:
   - Parquet data files
   - `_delta_log/` transaction log

#### Option 2: Python (Query Data)

```bash
# Install dependencies
pip install deltalake pandas pyarrow

# Query the Delta table
python3 << 'EOF'
import deltalake as dl
import pandas as pd

# Configure S3 connection for MinIO
storage_options = {
    "AWS_ACCESS_KEY_ID": "minioadmin",
    "AWS_SECRET_ACCESS_KEY": "minioadmin",
    "AWS_ENDPOINT_URL": "http://localhost:9000",
    "AWS_REGION": "us-east-1",
    "AWS_ALLOW_HTTP": "true",
}

# Load Delta table
dt = dl.DeltaTable("s3://delta-tables/events", storage_options=storage_options)

# Convert to pandas DataFrame
df = dt.to_pandas()

print(f"Total records: {len(df)}")
print("\nFirst 10 records:")
print(df.head(10))

print("\nEvent type distribution:")
print(df['event_type'].value_counts())

print("\nSchema:")
print(df.dtypes)
EOF
```

#### Option 3: Apache Spark (Advanced)

```python
from pyspark.sql import SparkSession

spark = SparkSession.builder \
    .appName("DeltaLakeExample") \
    .config("spark.jars.packages", "io.delta:delta-core_2.12:2.4.0") \
    .config("spark.sql.extensions", "io.delta.sql.DeltaSparkSessionExtension") \
    .config("spark.sql.catalog.spark_catalog", "org.apache.spark.sql.delta.catalog.DeltaCatalog") \
    .config("spark.hadoop.fs.s3a.endpoint", "http://localhost:9000") \
    .config("spark.hadoop.fs.s3a.access.key", "minioadmin") \
    .config("spark.hadoop.fs.s3a.secret.key", "minioadmin") \
    .config("spark.hadoop.fs.s3a.path.style.access", "true") \
    .config("spark.hadoop.fs.s3a.impl", "org.apache.hadoop.fs.s3a.S3AFileSystem") \
    .getOrCreate()

# Read Delta table
df = spark.read.format("delta").load("s3a://delta-tables/events")

# Show data
df.show(10)

# Run queries
df.createOrReplaceTempView("events")
spark.sql("SELECT event_type, COUNT(*) as count FROM events GROUP BY event_type").show()
```

## Configuration

### Connector Configuration (`connector.toml`)

The example includes a pre-configured `connector.toml` with:

- **Storage Backend**: S3 (MinIO)
- **Topic Mapping**: `/default/events` → `s3://delta-tables/events`
- **Schema**: Flexible schema supporting multiple event types
- **Batching**: 100 records or 1 second flush interval
- **Metadata**: Includes Danube metadata in `_danube_metadata` column

Key settings:
```toml
[deltalake]
storage_backend = "s3"
s3_region = "us-east-1"
s3_endpoint = "http://localhost:9000"  # MinIO
s3_allow_http = true

[[deltalake.topic_mappings]]
topic = "/default/events"
delta_table_path = "s3://delta-tables/events"
schema_type = "Json"
include_danube_metadata = true
```

### Environment Variables

Credentials and connection URLs are set via environment variables in `docker-compose.yml`:

```yaml
environment:
  - AWS_ACCESS_KEY_ID=minioadmin
  - AWS_SECRET_ACCESS_KEY=minioadmin
  - S3_ENDPOINT=http://minio:9000
  - DANUBE_SERVICE_URL=http://danube-broker:6650
```

## Monitoring

### Connector Metrics

```bash
# View Prometheus metrics
curl http://localhost:9090/metrics

# Key metrics:
# - connector_records_processed_total
# - connector_records_failed_total
# - connector_batch_size
# - connector_processing_duration_seconds
```

### Connector Logs

```bash
# Follow connector logs
docker-compose logs -f deltalake-sink

# Check for errors
docker-compose logs deltalake-sink | grep ERROR

# View last 100 lines
docker-compose logs --tail=100 deltalake-sink
```

### Danube Broker Metrics

```bash
# Danube broker metrics
curl http://localhost:9040/metrics
```

## Data Schema

The example uses a flexible schema that accommodates different event types:

```rust
schema = [
    { name = "event_type", data_type = "Utf8", nullable = false },
    { name = "user_id", data_type = "Utf8", nullable = false },
    { name = "timestamp", data_type = "Utf8", nullable = false },
    { name = "product", data_type = "Utf8", nullable = true },
    { name = "amount", data_type = "Int64", nullable = true },
    { name = "currency", data_type = "Utf8", nullable = true },
    // ... more fields
]
```

**Event Types:**

1. **Purchase Events**
   ```json
   {
     "event_type": "purchase",
     "user_id": "user_001",
     "product": "laptop",
     "amount": 999,
     "currency": "USD",
     "timestamp": "2024-01-01T12:00:00Z"
   }
   ```

2. **User Signup Events**
   ```json
   {
     "event_type": "user_signup",
     "user_id": "user_001",
     "email": "user_001@example.com",
     "source": "web",
     "timestamp": "2024-01-01T12:00:00Z"
   }
   ```

3. **User Login Events**
   ```json
   {
     "event_type": "user_login",
     "user_id": "user_001",
     "ip_address": "192.168.1.100",
     "device": "desktop",
     "timestamp": "2024-01-01T12:00:00Z"
   }
   ```

## Troubleshooting

### Services Won't Start

```bash
# Check service status
docker-compose ps

# Check logs for specific service
docker-compose logs etcd
docker-compose logs danube-broker
docker-compose logs minio
docker-compose logs deltalake-sink

# Restart services
docker-compose restart
```

### Connector Not Receiving Messages

```bash
# 1. Verify topic exists
docker exec -it danube-broker danube-admin-cli topics list

# 2. Check if topic has messages
docker-compose logs topic-init

# 3. Verify connector subscription
docker-compose logs deltalake-sink | grep subscription

# 4. Send test message
./test_producer.sh
```

### Cannot Access MinIO Console

```bash
# Check MinIO is running
docker-compose ps minio

# Check MinIO logs
docker-compose logs minio

# Verify port mapping
docker-compose port minio 9001
```

### Delta Table Not Created

```bash
# 1. Check connector logs for errors
docker-compose logs deltalake-sink | grep -i error

# 2. Verify MinIO bucket exists
docker exec -it minio mc ls myminio/

# 3. Check S3 credentials
docker-compose logs deltalake-sink | grep AWS

# 4. Verify connector can reach MinIO
docker exec -it deltalake-sink ping minio
```

### Python Query Fails

```bash
# Install correct versions
pip install deltalake==0.15.0 pandas pyarrow

# Verify MinIO is accessible
curl http://localhost:9000/minio/health/live

# Check bucket exists
curl -u minioadmin:minioadmin http://localhost:9000/delta-tables/
```

## Cleanup

```bash
# Stop all services
docker-compose down

# Remove volumes (deletes all data)
docker-compose down -v

# Remove images
docker-compose down --rmi all
```

## Advanced Usage

### Multiple Topics

Add more topic mappings in `connector.toml`:

```toml
[[deltalake.topic_mappings]]
topic = "/analytics/clicks"
delta_table_path = "s3://delta-tables/clicks"
schema_type = "Json"
schema = [
    { name = "user_id", data_type = "Utf8", nullable = false },
    { name = "page_url", data_type = "Utf8", nullable = false },
    { name = "timestamp", data_type = "Timestamp", nullable = false },
]

[[deltalake.topic_mappings]]
topic = "/logs/application"
delta_table_path = "s3://delta-tables/logs"
schema_type = "Json"
schema = [
    { name = "level", data_type = "Utf8", nullable = false },
    { name = "message", data_type = "Utf8", nullable = false },
    { name = "timestamp", data_type = "Timestamp", nullable = false },
]
```

### Performance Tuning

Adjust batch settings for your workload:

```toml
# High throughput
[deltalake]
batch_size = 5000
flush_interval_ms = 10000

# Low latency
[deltalake]
batch_size = 100
flush_interval_ms = 500
```

### Production Deployment

For production use:

1. **Use Real Cloud Storage**
   - Replace MinIO with AWS S3, Azure Blob Storage, or GCS
   - Update `storage_backend` and credentials

2. **Enable TLS**
   - Configure Danube broker with TLS
   - Use HTTPS endpoints

3. **Set Up Monitoring**
   - Export metrics to Prometheus
   - Set up Grafana dashboards
   - Configure alerts

4. **Scale Horizontally**
   - Run multiple connector instances
   - Use different subscription names for parallel processing

5. **Backup and Recovery**
   - Enable Delta Lake time travel
   - Set up regular backups
   - Test disaster recovery procedures

## Resources

- **[Delta Lake Sink Connector Documentation](../../connectors/sink-deltalake/README.md)**
- **[Configuration Guide](../../connectors/sink-deltalake/config/README.md)**
- [Delta Lake Documentation](https://docs.delta.io/)
- [Danube Documentation](https://github.com/danube-messaging/danube)
- [MinIO Documentation](https://min.io/docs/)

## License

MIT OR Apache-2.0
