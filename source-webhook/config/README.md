# HTTP/Webhook Source Connector - Configuration

This directory contains two example configurations for the webhook connector.

## Configuration Files

### 1. `connector.toml` - With Schema Validation

Full-featured configuration demonstrating schema registry integration:

```bash
# View the file
cat config/connector.toml
```

**What's included:**
- ✅ **2 Schemas configured**: Payment events (JSON Schema) + Generic webhooks (String schema)
- ✅ **4 Endpoints**: Stripe payments, GitHub push, generic webhooks, alerts
- ✅ **API Key authentication**
- ✅ **Rate limiting** (platform-wide + per-IP)
- ✅ **Mixed mode**: Some endpoints with schemas, others without

**Key sections:**
```toml
# Flattened core settings
danube_service_url = "http://localhost:6650"
connector_name = "webhook-stripe"

[processing]
batch_size = 100
poll_interval_ms = 100

[retry]
initial_interval_ms = 1000
max_retries = 5

# Schema validation
[[schemas]]
topic = "/stripe/payments"
subject = "stripe-payment-v1"
schema_type = "json_schema"
schema_file = "schemas/payment.json"
auto_register = true

# Endpoints
[[endpoints]]
path = "/webhooks/stripe/payments"
danube_topic = "/stripe/payments"  # ← Matches schema topic
partitions = 4
reliable_dispatch = true
```

**Use this when:**
- You want payload validation before publishing to Danube
- You need data quality guarantees
- You're integrating with SaaS webhooks (Stripe, GitHub, etc.)
- You want schema versioning and evolution

---

### 2. `connector-no-schemas.toml` - Without Schema Validation

Minimal configuration without schema validation (backward compatible):

```bash
# View the file
cat config/connector-no-schemas.toml
```

**What's included:**
- ❌ **No schemas** - All webhooks accepted without validation
- ✅ **2 Endpoints**: Generic and critical webhooks
- ✅ **API Key authentication**
- ✅ **Rate limiting**
- ✅ **Simpler setup**

**Key difference:**
```toml
# No [[schemas]] sections at all

[[endpoints]]
path = "/webhooks/generic"
danube_topic = "/webhooks/events"  # ← No schema validation
```

**Use this when:**
- You don't need payload validation
- You want maximum flexibility
- You're migrating from older connector versions
- You trust the webhook source

---

## Configuration Structure

### Flattened Core Settings (Required)

```toml
# At root level (no [core] section)
danube_service_url = "http://localhost:6650"
connector_name = "webhook-source"
```

### Processing Settings (Optional)

```toml
[processing]
batch_size = 100
poll_interval_ms = 100
metrics_port = 9090
```

### Retry Settings (Optional)

```toml
[retry]
initial_interval_ms = 1000
max_interval_ms = 60000
max_retries = 5
```

### Schema Configuration (Optional)

```toml
[[schemas]]
topic = "/stripe/payments"           # Must match endpoint's danube_topic
subject = "stripe-payment-v1"        # Schema registry subject name
schema_type = "json_schema"          # or "string", "bytes", "number"
schema_file = "schemas/payment.json" # Path to schema file (empty for primitives)
auto_register = true                 # Auto-register on startup
version_strategy = "latest"          # or "exact"
```

### Server Settings (Required)

```toml
[server]
host = "0.0.0.0"
port = 8080
timeout_seconds = 30
max_body_size = 1048576  # 1MB
```

### Authentication (Required)

```toml
[auth]
type = "apikey"              # Options: none, apikey, hmac, jwt
secret_env = "WEBHOOK_API_KEY"  # Environment variable name
header = "x-api-key"         # Header to check
```

### Rate Limiting (Optional)

```toml
[rate_limit]
requests_per_second = 100
burst_size = 200
per_ip_enabled = true
per_ip_requests_per_second = 10
```

### Endpoints (Required - at least one)

```toml
[[endpoints]]
path = "/webhooks/payments"          # HTTP path
danube_topic = "/stripe/payments"    # Target Danube topic
partitions = 4                       # Optional: 0 or omitted = non-partitioned
reliable_dispatch = true             # Optional: default false
```

---

## Environment Variables

Only secrets and connection URLs can be overridden:

```bash
# Required
export CONNECTOR_CONFIG_PATH=/path/to/connector.toml

# Optional overrides
export DANUBE_SERVICE_URL=http://prod-broker:6650
export CONNECTOR_NAME=webhook-prod-1

# Secrets (do NOT put in TOML file)
export WEBHOOK_API_KEY=your-secret-key
export WEBHOOK_HMAC_SECRET=your-hmac-secret
export WEBHOOK_JWT_SECRET=your-jwt-secret
```

**Cannot be overridden** (must be in TOML):
- Endpoints (`[[endpoints]]`)
- Schemas (`[[schemas]]`)
- Server settings (`[server]`)
- Rate limiting (`[rate_limit]`)

---

## Schema Types

### JSON Schema (Recommended for structured data)

```toml
[[schemas]]
topic = "/stripe/payments"
subject = "stripe-payment-v1"
schema_type = "json_schema"
schema_file = "schemas/payment.json"  # Path to .json file
auto_register = true
version_strategy = "latest"
```

Validates:
- Required fields
- Data types (string, number, boolean, etc.)
- Patterns (regex)
- Enums (allowed values)
- Nested objects

### String Schema (Simple text validation)

```toml
[[schemas]]
topic = "/webhooks/generic"
subject = "generic-webhook-v1"
schema_type = "string"
schema_file = ""  # Empty for primitive types
auto_register = true
```

Validates:
- Any text/JSON is accepted
- Good for flexible payloads

### Other Primitive Types

```toml
# Number schema
schema_type = "number"
schema_file = ""

# Bytes schema
schema_type = "bytes"
schema_file = ""
```

---

## Quick Start

### 1. Choose Your Configuration

**With schema validation:**
```bash
cp config/connector.toml myconfig.toml
# Create schemas directory
mkdir -p schemas
# Add your schema files
```

**Without schema validation:**
```bash
cp config/connector-no-schemas.toml myconfig.toml
# No schemas needed
```

### 2. Set Environment Variables

```bash
export CONNECTOR_CONFIG_PATH=myconfig.toml
export WEBHOOK_API_KEY=your-secret-api-key
```

### 3. Run the Connector

```bash
cargo run --release
```

---

## Comparing the Two Configurations

| Feature | connector.toml | connector-no-schemas.toml |
|---------|---------------|--------------------------|
| **Schemas** | ✅ 2 schemas (JSON + String) | ❌ None |
| **Endpoints** | 4 endpoints | 2 endpoints |
| **Validation** | ✅ Validates payloads | ❌ Accepts all |
| **Use case** | Production with SaaS webhooks | Simple/flexible setups |
| **Data quality** | ✅ Guaranteed | ❌ No guarantees |
| **Setup complexity** | Medium (need schema files) | Low (just TOML) |

---

## Schema File Example

**Payment schema** (`schemas/payment.json`):

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["event", "amount", "currency", "timestamp"],
  "properties": {
    "event": {
      "type": "string",
      "enum": ["payment.succeeded", "payment.failed", "payment.canceled"]
    },
    "amount": {
      "type": "integer",
      "minimum": 0
    },
    "currency": {
      "type": "string",
      "pattern": "^[A-Z]{3}$"
    },
    "timestamp": {
      "type": "integer"
    }
  }
}
```

---

## Testing Your Configuration

```bash
# Test with valid payload
curl -X POST http://localhost:8080/webhooks/stripe/payments \
  -H "Content-Type: application/json" \
  -H "x-api-key: your-secret-api-key" \
  -d '{
    "event": "payment.succeeded",
    "amount": 5000,
    "currency": "USD",
    "timestamp": 1704931200
  }'

# Expected: 200 OK

# Test with invalid payload (if using connector.toml with schemas)
curl -X POST http://localhost:8080/webhooks/stripe/payments \
  -H "Content-Type: application/json" \
  -H "x-api-key: your-secret-api-key" \
  -d '{"event": "payment.succeeded"}'

# Expected: 400 Bad Request (missing required fields)
```

---

## Additional Resources

- **Full example**: See [../example/README.md](../example/README.md) for complete Docker Compose setup
- **Main README**: See [../README.md](../README.md) for features and use cases
- **Danube docs**: [https://github.com/danrusei/danube](https://github.com/danrusei/danube)
