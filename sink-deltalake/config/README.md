# Delta Lake Sink Connector Configuration Guide

Complete reference for configuring the Delta Lake Sink Connector.

## Table of Contents

- [Configuration Methods](#configuration-methods)
- [Core Settings](#core-settings)
- [Cloud Provider Setup](#cloud-provider-setup)
- [Topic Mappings](#topic-mappings)
- [Schema Definition](#schema-definition)
- [Batch Processing](#batch-processing)
- [Environment Variables](#environment-variables)
- [Examples](#examples)

## Configuration Methods

### TOML Configuration File (Required)

All connector configuration must be defined in a TOML file:

```bash
CONNECTOR_CONFIG_PATH=/path/to/connector.toml danube-sink-deltalake
```

### Environment Variable Overrides

Environment variables can override **only secrets and connection URLs**:

```bash
# Base configuration from TOML file
CONNECTOR_CONFIG_PATH=config.toml \
# Override secrets (don't put these in config files!)
AWS_ACCESS_KEY_ID=your-access-key \
AWS_SECRET_ACCESS_KEY=your-secret-key \
# Override URLs for different environments
DANUBE_SERVICE_URL=http://prod-broker:6650 \
danube-sink-deltalake
```

**Note:** Topic mappings, schemas, and other settings must be in the TOML file.

## Core Settings

These settings configure the connector instance and Danube connection:

```toml
# Connector instance name (appears in logs and metrics)
connector_name = "deltalake-sink-production"

# Danube broker service URL
danube_service_url = "http://localhost:6650"

# Prometheus metrics port
metrics_port = 9090
```

### Environment Variable Overrides

| Variable | TOML Key | Purpose |
|----------|----------|---------|
| `CONNECTOR_NAME` | `connector_name` | Override for different deployments |
| `DANUBE_SERVICE_URL` | `danube_service_url` | Override for different environments |

**Note:** `metrics_port` must be configured in the TOML file.

## Cloud Provider Setup

### Storage Backend Selection

Choose one cloud provider per connector instance:

```toml
[deltalake]
storage_backend = "s3"  # or "azure" or "gcs"
```

### AWS S3 Configuration

```toml
[deltalake]
storage_backend = "s3"
s3_region = "us-east-1"
s3_endpoint = "http://localhost:9000"  # Optional: for MinIO
s3_allow_http = true  # Optional: for MinIO/local testing

# Credentials from environment variables:
# AWS_ACCESS_KEY_ID
# AWS_SECRET_ACCESS_KEY
```

**MinIO Example:**
```toml
[deltalake]
storage_backend = "s3"
s3_region = "us-east-1"
s3_endpoint = "http://localhost:9000"
s3_allow_http = true
```

### Azure Blob Storage Configuration

```toml
[deltalake]
storage_backend = "azure"
azure_storage_account = "mystorageaccount"

# Credentials from environment variables:
# AZURE_STORAGE_ACCOUNT_KEY (or AZURE_STORAGE_SAS_TOKEN)
```

**Delta Table Path Format:**
```toml
delta_table_path = "abfss://container@account.dfs.core.windows.net/path/to/table"
```

### Google Cloud Storage Configuration

```toml
[deltalake]
storage_backend = "gcs"
gcp_project_id = "my-gcp-project"

# Credentials from environment variables:
# GOOGLE_APPLICATION_CREDENTIALS (path to service account JSON)
```

**Delta Table Path Format:**
```toml
delta_table_path = "gs://my-bucket/path/to/table"
```

## Topic Mappings

Map Danube topics to Delta Lake tables:

```toml
[[deltalake.topic_mappings]]
topic = "/events/payments"
subscription = "deltalake-payments"
delta_table_path = "s3://my-bucket/tables/payments"
schema_type = "Json"
write_mode = "append"  # or "overwrite"
include_danube_metadata = true

# Define table schema (compact inline format)
schema = [
    { name = "payment_id", data_type = "Utf8", nullable = false },
    { name = "user_id", data_type = "Utf8", nullable = false },
    { name = "amount", data_type = "Float64", nullable = false },
    { name = "currency", data_type = "Utf8", nullable = false },
    { name = "status", data_type = "Utf8", nullable = false },
    { name = "created_at", data_type = "Timestamp", nullable = false },
]
```

### Topic Mapping Options

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `topic` | String | Yes | Danube topic to consume from (format: `/namespace/topic`) |
| `subscription` | String | Yes | Subscription name for this consumer |
| `delta_table_path` | String | Yes | Full path to Delta table (includes cloud prefix) |
| `schema_type` | String | Yes | Danube schema type: `Json`, `String`, `Int64`, `Bytes` |
| `schema` | Array | Yes | Arrow schema definition (see below) |
| `write_mode` | String | No | `append` (default) or `overwrite` |
| `include_danube_metadata` | Boolean | No | Add `_danube_metadata` JSON column (default: false) |
| `batch_size` | Integer | No | Override global batch size for this topic |
| `flush_interval_ms` | Integer | No | Override global flush interval for this topic |

## Schema Definition

Define your Delta Lake table schema using Arrow data types:

### Compact Inline Format (Recommended)

```toml
schema = [
    { name = "field_name", data_type = "Utf8", nullable = true },
    { name = "amount", data_type = "Float64", nullable = false },
    { name = "created_at", data_type = "Timestamp", nullable = false },
]
```

### Supported Arrow Data Types

| Data Type | Description | Example Values |
|-----------|-------------|----------------|
| `Utf8` | String | `"hello"`, `"user@example.com"` |
| `Int8` | 8-bit signed integer | `-128` to `127` |
| `Int16` | 16-bit signed integer | `-32768` to `32767` |
| `Int32` | 32-bit signed integer | `-2147483648` to `2147483647` |
| `Int64` | 64-bit signed integer | Large integers |
| `UInt8` | 8-bit unsigned integer | `0` to `255` |
| `UInt16` | 16-bit unsigned integer | `0` to `65535` |
| `UInt32` | 32-bit unsigned integer | `0` to `4294967295` |
| `UInt64` | 64-bit unsigned integer | Large positive integers |
| `Float32` | 32-bit floating point | `3.14`, `-0.5` |
| `Float64` | 64-bit floating point | High precision decimals |
| `Boolean` | True/False | `true`, `false` |
| `Timestamp` | Timestamp (microsecond precision) | `2024-01-01T12:00:00Z` |
| `Binary` | Binary data | Raw bytes |

### Schema Examples

**E-commerce Orders:**
```toml
schema = [
    { name = "order_id", data_type = "Utf8", nullable = false },
    { name = "customer_id", data_type = "Utf8", nullable = false },
    { name = "total_amount", data_type = "Float64", nullable = false },
    { name = "item_count", data_type = "Int32", nullable = false },
    { name = "is_paid", data_type = "Boolean", nullable = false },
    { name = "order_date", data_type = "Timestamp", nullable = false },
]
```

**IoT Sensor Data:**
```toml
schema = [
    { name = "sensor_id", data_type = "Utf8", nullable = false },
    { name = "temperature", data_type = "Float32", nullable = false },
    { name = "humidity", data_type = "Float32", nullable = false },
    { name = "pressure", data_type = "Float32", nullable = true },
    { name = "battery_level", data_type = "UInt8", nullable = true },
    { name = "timestamp", data_type = "Timestamp", nullable = false },
]
```

## Batch Processing

Configure batching for optimal throughput:

```toml
[deltalake]
# Global defaults
batch_size = 1000
flush_interval_ms = 5000
```

### Batch Settings

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `batch_size` | Integer | 1000 | Number of records to batch before writing |
| `flush_interval_ms` | Integer | 5000 | Maximum time (ms) to wait before flushing |

### Per-Topic Overrides

```toml
# High-volume topic with larger batches
[[deltalake.topic_mappings]]
topic = "/events/high-volume"
batch_size = 5000
flush_interval_ms = 10000
schema = [...]

# Low-latency topic with smaller batches
[[deltalake.topic_mappings]]
topic = "/alerts/critical"
batch_size = 100
flush_interval_ms = 1000
schema = [...]
```

### Performance Tuning

**High Throughput:**
- Increase `batch_size` (e.g., 5000-10000)
- Increase `flush_interval_ms` (e.g., 10000-30000)
- Trade-off: Higher latency

**Low Latency:**
- Decrease `batch_size` (e.g., 100-500)
- Decrease `flush_interval_ms` (e.g., 1000-2000)
- Trade-off: Lower throughput

## Environment Variables

### Required

| Variable | Description |
|----------|-------------|
| `CONNECTOR_CONFIG_PATH` | Path to TOML configuration file |

### Core Overrides

| Variable | TOML Key | Description |
|----------|----------|-------------|
| `DANUBE_SERVICE_URL` | `danube_service_url` | Override Danube broker URL |
| `CONNECTOR_NAME` | `connector_name` | Override connector name |

### Cloud Provider Credentials

**AWS S3:**
| Variable | Description |
|----------|-------------|
| `AWS_ACCESS_KEY_ID` | AWS access key ID |
| `AWS_SECRET_ACCESS_KEY` | AWS secret access key |
| `AWS_REGION` | Override S3 region |
| `S3_ENDPOINT` | Override S3 endpoint (for MinIO) |

**Azure Blob Storage:**
| Variable | Description |
|----------|-------------|
| `AZURE_STORAGE_ACCOUNT_KEY` | Azure storage account key |
| `AZURE_STORAGE_SAS_TOKEN` | Alternative: SAS token |
| `AZURE_STORAGE_ACCOUNT` | Override storage account name |

**Google Cloud Storage:**
| Variable | Description |
|----------|-------------|
| `GOOGLE_APPLICATION_CREDENTIALS` | Path to service account JSON file |
| `GCP_PROJECT_ID` | Override GCP project ID |

## Examples

### Complete S3/MinIO Example

```toml
connector_name = "deltalake-sink-dev"
danube_service_url = "http://localhost:6650"
metrics_port = 9090

[deltalake]
storage_backend = "s3"
s3_region = "us-east-1"
s3_endpoint = "http://localhost:9000"
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

[[deltalake.topic_mappings]]
topic = "/events/users"
subscription = "deltalake-users"
delta_table_path = "s3://my-bucket/tables/users"
schema_type = "Json"
batch_size = 2000
schema = [
    { name = "user_id", data_type = "Utf8", nullable = false },
    { name = "email", data_type = "Utf8", nullable = false },
    { name = "signup_date", data_type = "Timestamp", nullable = false },
]
```

### Azure Example

```toml
connector_name = "deltalake-sink-prod"
danube_service_url = "http://danube-broker:6650"
metrics_port = 9090

[deltalake]
storage_backend = "azure"
azure_storage_account = "mystorageaccount"

batch_size = 1000
flush_interval_ms = 5000

[[deltalake.topic_mappings]]
topic = "/iot/sensors"
subscription = "deltalake-sensors"
delta_table_path = "abfss://delta-tables@mystorageaccount.dfs.core.windows.net/sensors"
schema_type = "Json"
schema = [
    { name = "sensor_id", data_type = "Utf8", nullable = false },
    { name = "temperature", data_type = "Float64", nullable = false },
    { name = "timestamp", data_type = "Timestamp", nullable = false },
]
```

### GCS Example

```toml
connector_name = "deltalake-sink-analytics"
danube_service_url = "http://danube-broker:6650"
metrics_port = 9090

[deltalake]
storage_backend = "gcs"
gcp_project_id = "my-gcp-project"

batch_size = 5000
flush_interval_ms = 10000

[[deltalake.topic_mappings]]
topic = "/analytics/events"
subscription = "deltalake-analytics"
delta_table_path = "gs://my-bucket/tables/analytics"
schema_type = "Json"
include_danube_metadata = true
schema = [
    { name = "event_id", data_type = "Utf8", nullable = false },
    { name = "user_id", data_type = "Utf8", nullable = false },
    { name = "event_type", data_type = "Utf8", nullable = false },
    { name = "timestamp", data_type = "Timestamp", nullable = false },
]
```

## Best Practices

1. **Schema Design**
   - Define schemas explicitly - don't rely on schema inference
   - Use appropriate data types for your data
   - Mark required fields as `nullable = false`
   - Include timestamp fields for time-series data

2. **Batching**
   - Start with default batch sizes (1000 records, 5s flush)
   - Tune based on your throughput and latency requirements
   - Use per-topic overrides for different workloads

3. **Security**
   - Never put credentials in TOML files
   - Use environment variables for all secrets
   - Use IAM roles/managed identities when possible
   - Rotate credentials regularly

4. **Monitoring**
   - Monitor Prometheus metrics on `metrics_port`
   - Watch for batch flush times
   - Monitor Delta Lake transaction log size
   - Set up alerts for connection failures

5. **Testing**
   - Test locally with MinIO before deploying to cloud
   - Verify schema matches your data structure
   - Test with small batches first
   - Validate data in Delta Lake after writes
