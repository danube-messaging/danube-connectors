# Delta Lake Sink Connector Configuration Guide

Complete reference for configuring the Delta Lake Sink Connector.

## Table of Contents

- [Configuration Methods](#configuration-methods)
- [Core Settings](#core-settings)
- [Cloud Provider Setup](#cloud-provider-setup)
- [Topic Mappings](#topic-mappings)
- [Schema Validation](#schema-validation)
- [Field Mappings](#field-mappings)
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
to = "abfss://container@account.dfs.core.windows.net/path/to/table"
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
to = "gs://my-bucket/path/to/table"
```

## Routes

Map Danube topics to Delta Lake tables:

```toml
[[deltalake.routes]]
from = "/events/payments"
subscription = "deltalake-payments"
to = "s3://my-bucket/tables/payments"
write_mode = "append"  # or "overwrite"
include_danube_metadata = true

# Schema validation (schema already exists on topic via producer/admin)
expected_schema_subject = "payment-events-v1"

# Field mappings: JSON path → Delta Lake column
field_mappings = [
    { json_path = "payment_id", column = "payment_id", data_type = "Utf8", nullable = false },
    { json_path = "user_id", column = "user_id", data_type = "Utf8", nullable = false },
    { json_path = "amount", column = "amount", data_type = "Float64", nullable = false },
    { json_path = "currency", column = "currency", data_type = "Utf8", nullable = false },
    { json_path = "status", column = "status", data_type = "Utf8", nullable = false },
    { json_path = "created_at", column = "created_at", data_type = "Timestamp", nullable = false },
]
```

### Route Options

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `from` | String | Yes | Danube topic to consume from (format: `/namespace/topic`) |
| `subscription` | String | Yes | Subscription name for this consumer |
| `to` | String | Yes | Full path to Delta table (includes cloud prefix) |
| `expected_schema_subject` | String | Recommended | Schema subject for validation (created by producer/admin) |
| `field_mappings` | Array | Yes | Field mappings from JSON to Delta Lake columns (see below) |
| `write_mode` | String | No | `append` (default) or `overwrite` |
| `include_danube_metadata` | Boolean | No | Add `_danube_metadata` JSON column (default: false) |

## Schema Validation

**Important:** Schemas are created by **producers** or **danube-admin-cli** and attached to topics. The sink connector only validates that incoming messages match the expected schema.

### Schema Subject (Recommended)

Specify the schema subject to enable automatic validation:

```toml
[[deltalake.routes]]
from = "/events/payments"
expected_schema_subject = "payment-events-v1"  # Schema created by producer
field_mappings = [...]
```

**Benefits:**
- Runtime automatically validates messages
- Type-safe deserialization
- Schema evolution support
- No manual parsing needed

**Note:** If `expected_schema_subject` is not specified, messages are consumed without validation.

## Field Mappings

Define how JSON fields map to Delta Lake columns:

### Basic Field Mapping

```toml
field_mappings = [
    { json_path = "user_id", column = "user_id", data_type = "Utf8", nullable = false },
    { json_path = "amount", column = "amount", data_type = "Float64", nullable = false },
    { json_path = "created_at", column = "created_at", data_type = "Timestamp", nullable = false },
]
```

### Nested JSON Path Support

Extract values from nested JSON structures:

```toml
field_mappings = [
    # Simple path
    { json_path = "payment_id", column = "payment_id", data_type = "Utf8", nullable = false },
    
    # Nested path (extracts user.profile.email)
    { json_path = "user.profile.email", column = "user_email", data_type = "Utf8", nullable = false },
    
    # Deep nesting
    { json_path = "order.shipping.address.city", column = "city", data_type = "Utf8", nullable = true },
]
```

**Example JSON:**
```json
{
  "payment_id": "pay_123",
  "user": {
    "profile": {
      "email": "user@example.com"
    }
  },
  "order": {
    "shipping": {
      "address": {
        "city": "San Francisco"
      }
    }
  }
}
```

### Field Mapping Options

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `json_path` | String | Yes | JSON path to extract (supports nested: `"user.profile.name"`) |
| `column` | String | Yes | Delta Lake column name |
| `data_type` | String | Yes | Arrow data type (see below) |
| `nullable` | Boolean | No | Allow null values (default: true) |

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

### Field Mapping Examples

**E-commerce Orders:**
```toml
expected_schema_subject = "orders-v1"
field_mappings = [
    { json_path = "order_id", column = "order_id", data_type = "Utf8", nullable = false },
    { json_path = "customer_id", column = "customer_id", data_type = "Utf8", nullable = false },
    { json_path = "total_amount", column = "total_amount", data_type = "Float64", nullable = false },
    { json_path = "item_count", column = "item_count", data_type = "Int32", nullable = false },
    { json_path = "is_paid", column = "is_paid", data_type = "Boolean", nullable = false },
    { json_path = "order_date", column = "order_date", data_type = "Timestamp", nullable = false },
]
```

**IoT Sensor Data with Nested Fields:**
```toml
expected_schema_subject = "sensor-data-v1"
field_mappings = [
    { json_path = "device.sensor_id", column = "sensor_id", data_type = "Utf8", nullable = false },
    { json_path = "readings.temperature", column = "temperature", data_type = "Float32", nullable = false },
    { json_path = "readings.humidity", column = "humidity", data_type = "Float32", nullable = false },
    { json_path = "readings.pressure", column = "pressure", data_type = "Float32", nullable = true },
    { json_path = "device.battery_level", column = "battery_level", data_type = "UInt8", nullable = true },
    { json_path = "timestamp", column = "timestamp", data_type = "Timestamp", nullable = false },
]
```

## Runtime Processing

Batching and flush timing are managed by `danube-connect-core` runtime settings rather than the Delta Lake adapter.

```toml
[processing]
batch_size = 1000
batch_timeout_ms = 5000
```

Tune the shared processing section for throughput or latency without adding connector-specific batch settings.

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

[[deltalake.routes]]
from = "/events/payments"
subscription = "deltalake-payments"
to = "s3://my-bucket/tables/payments"
include_danube_metadata = true
expected_schema_subject = "payment-events-v1"
field_mappings = [
    { json_path = "payment_id", column = "payment_id", data_type = "Utf8", nullable = false },
    { json_path = "amount", column = "amount", data_type = "Float64", nullable = false },
    { json_path = "currency", column = "currency", data_type = "Utf8", nullable = false },
]

[[deltalake.routes]]
from = "/events/users"
subscription = "deltalake-users"
to = "s3://my-bucket/tables/users"
expected_schema_subject = "user-events-v1"
field_mappings = [
    { json_path = "user_id", column = "user_id", data_type = "Utf8", nullable = false },
    { json_path = "email", column = "email", data_type = "Utf8", nullable = false },
    { json_path = "signup_date", column = "signup_date", data_type = "Timestamp", nullable = false },
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

[[deltalake.routes]]
from = "/iot/sensors"
subscription = "deltalake-sensors"
to = "abfss://delta-tables@mystorageaccount.dfs.core.windows.net/sensors"
expected_schema_subject = "sensor-data-v1"
field_mappings = [
    { json_path = "sensor_id", column = "sensor_id", data_type = "Utf8", nullable = false },
    { json_path = "temperature", column = "temperature", data_type = "Float64", nullable = false },
    { json_path = "timestamp", column = "timestamp", data_type = "Timestamp", nullable = false },
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

[[deltalake.routes]]
from = "/analytics/events"
subscription = "deltalake-analytics"
to = "gs://my-bucket/tables/analytics"
include_danube_metadata = true
expected_schema_subject = "analytics-events-v1"
field_mappings = [
    { json_path = "event_id", column = "event_id", data_type = "Utf8", nullable = false },
    { json_path = "user_id", column = "user_id", data_type = "Utf8", nullable = false },
    { json_path = "event_type", column = "event_type", data_type = "Utf8", nullable = false },
    { json_path = "timestamp", column = "timestamp", data_type = "Timestamp", nullable = false },
]
```

## Best Practices

1. **Schema Management**
   - **Create schemas via producer or danube-admin-cli** - Sink connectors only validate
   - Use `expected_schema_subject` for automatic schema validation
   - Define explicit field mappings - avoid schema inference
   - Use appropriate data types for your data
   - Mark required fields as `nullable = false`
   - Include timestamp fields for time-series data
   - Use nested JSON paths for complex data structures

2. **Field Mappings**
   - Keep field mappings simple and flat when possible
   - Use descriptive column names in Delta Lake
   - Leverage nested JSON paths for extracting deeply nested data
   - Example: `"user.profile.email"` instead of flattening in producer
   - Pre-split paths are cached for performance (no repeated parsing)

3. **Batching**
   - Start with default batch sizes (1000 records, 5s flush)
   - Tune based on your throughput and latency requirements
   - Use per-topic overrides for different workloads
   - Higher batches = better throughput, higher latency
   - Lower batches = lower latency, more frequent writes

4. **Security**
   - Never put credentials in TOML files
   - Use environment variables for all secrets
   - Use IAM roles/managed identities when possible
   - Rotate credentials regularly
   - Limit access to Delta Lake storage locations

5. **Monitoring**
   - Monitor Prometheus metrics on `metrics_port`
   - Watch for batch flush times
   - Monitor Delta Lake transaction log size
   - Set up alerts for connection failures
   - Track schema validation errors

6. **Testing**
   - Test locally with MinIO before deploying to cloud
   - Create test schemas via danube-admin-cli
   - Verify field mappings extract correct values
   - Test with small batches first
   - Validate data in Delta Lake after writes
   - Test nested JSON path extraction
