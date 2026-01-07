# HTTP/Webhook Source Connector Configuration Guide

Complete configuration reference for the HTTP/Webhook Source Connector.

## Table of Contents

- [Configuration File Structure](#configuration-file-structure)
- [Core Settings](#core-settings)
- [Server Settings](#server-settings)
- [Authentication Configuration](#authentication-configuration)
- [Rate Limiting Configuration](#rate-limiting-configuration)
- [Endpoint Configuration](#endpoint-configuration)
- [Environment Variable Overrides](#environment-variable-overrides)
- [Configuration Examples](#configuration-examples)
- [Best Practices](#best-practices)

## Configuration File Structure

The connector uses a TOML configuration file with five main sections:

```toml
# Core connector settings
[core]
connector_name = "webhook-source"
danube_service_url = "http://localhost:6650"

# HTTP server settings
[server]
host = "0.0.0.0"
port = 8080

# Authentication (platform-wide)
[auth]
type = "apikey"
secret_env = "WEBHOOK_API_KEY"

# Rate limiting (platform-wide)
[rate_limit]
requests_per_second = 100
burst_size = 200

# Webhook endpoints
[[endpoints]]
path = "/webhooks/payments"
danube_topic = "/stripe/payments"
partitions = 4
reliable_dispatch = true
```

## Core Settings

### Required Fields

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `core.connector_name` | string | Unique identifier for this connector instance | `"webhook-production-1"` |
| `core.danube_service_url` | string | Danube broker URL | `"http://danube-broker:6650"` |

### Optional Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `core.metrics_port` | integer | `9090` | Prometheus metrics port |

## Server Settings

### HTTP Server Configuration

```toml
[server]
host = "0.0.0.0"
port = 8080
timeout_seconds = 30
max_body_size = 1048576
```

### Server Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `server.host` | string | `"0.0.0.0"` | Host to bind to (0.0.0.0 = all interfaces) |
| `server.port` | integer | `8080` | HTTP port to listen on |
| `server.timeout_seconds` | integer | `30` | Request timeout in seconds |
| `server.max_body_size` | integer | `1048576` | Maximum request body size in bytes (default: 1MB) |

### TLS Configuration (Optional)

```toml
[server]
tls_cert_path = "/path/to/cert.pem"
tls_key_path = "/path/to/key.pem"
```

**Note:** For production, it's recommended to use a reverse proxy (nginx, Traefik) for TLS termination instead of built-in TLS.

## Authentication Configuration

Authentication is **platform-wide** and applies to all webhook endpoints. Health check endpoints (`/health`, `/ready`) are always accessible without authentication.

### Authentication Types

#### 1. None (No Authentication)

```toml
[auth]
type = "none"
```

**Use Case:** Internal webhooks behind firewall, development/testing

#### 2. API Key Authentication

```toml
[auth]
type = "apikey"
secret_env = "WEBHOOK_API_KEY"
header = "x-api-key"
```

**Fields:**
- `type` - Must be `"apikey"`
- `secret_env` - Environment variable containing the API key
- `header` - HTTP header name to check (default: `"x-api-key"`)

**Example Request:**
```bash
curl -X POST http://localhost:8080/webhooks/payments \
  -H "x-api-key: sk_live_abc123..." \
  -d '{"event": "payment"}'
```

**Use Case:** Simple authentication, internal services, custom webhooks

#### 3. HMAC Signature Authentication

```toml
[auth]
type = "hmac"
secret_env = "WEBHOOK_HMAC_SECRET"
header = "Stripe-Signature"
algorithm = "sha256"
```

**Fields:**
- `type` - Must be `"hmac"`
- `secret_env` - Environment variable containing the HMAC secret
- `header` - HTTP header containing the signature
- `algorithm` - Hash algorithm: `"sha256"` or `"sha512"`

**Supported Providers:**
- **Stripe**: `header = "Stripe-Signature"`, `algorithm = "sha256"`
- **GitHub**: `header = "X-Hub-Signature-256"`, `algorithm = "sha256"`
- **Shopify**: `header = "X-Shopify-Hmac-Sha256"`, `algorithm = "sha256"`

**Example Request (Stripe):**
```bash
curl -X POST http://localhost:8080/webhooks/stripe/payments \
  -H "Stripe-Signature: t=1234567890,v1=signature_hash" \
  -d '{"event": "payment.succeeded"}'
```

**Use Case:** SaaS webhooks (Stripe, GitHub, Shopify), signature verification

**Note:** HMAC implementation requires body buffering and is partially implemented. Use API Key or JWT for now.

#### 4. JWT Authentication

```toml
[auth]
type = "jwt"
secret_env = "WEBHOOK_JWT_SECRET"
```

**Fields:**
- `type` - Must be `"jwt"`
- `secret_env` - Environment variable containing JWT secret or public key
- `public_key_path` - (Optional) Path to RSA public key file for RS256

**Example Request:**
```bash
curl -X POST http://localhost:8080/webhooks/generic \
  -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..." \
  -d '{"event": "test"}'
```

**Use Case:** Service-to-service authentication, multi-tenant SaaS

### Authentication Fields Reference

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `auth.type` | string | ✅ | Authentication type: `"none"`, `"apikey"`, `"hmac"`, `"jwt"` |
| `auth.secret_env` | string | ✅ (except none) | Environment variable containing the secret |
| `auth.header` | string | optional | Header name (API Key/HMAC only) |
| `auth.algorithm` | string | optional | Hash algorithm for HMAC: `"sha256"` or `"sha512"` |
| `auth.public_key_path` | string | optional | Path to public key file (JWT RS256 only) |

## Rate Limiting Configuration

Rate limiting is **platform-wide** and applies to all webhook endpoints. Health check endpoints are not rate limited.

### Basic Rate Limiting

```toml
[rate_limit]
requests_per_second = 100
burst_size = 200
```

### Per-IP Rate Limiting

```toml
[rate_limit]
requests_per_second = 100
burst_size = 200
per_ip_enabled = true
per_ip_requests_per_second = 10
```

### Rate Limiting Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `rate_limit.requests_per_second` | integer | `100` | Maximum requests per second per endpoint |
| `rate_limit.burst_size` | integer | `200` | Maximum burst capacity (token bucket) |
| `rate_limit.per_ip_enabled` | boolean | `false` | Enable per-IP rate limiting |
| `rate_limit.per_ip_requests_per_second` | integer | `10` | Maximum requests per second per IP |

### How Rate Limiting Works

The connector uses a **token bucket algorithm**:

1. **Endpoint Rate Limit**: Applies to each endpoint path
   - Tokens refill at `requests_per_second` rate
   - Bucket capacity is `burst_size`
   - Allows traffic spikes up to burst size

2. **Per-IP Rate Limit** (optional): Applies per client IP
   - Independent limit per IP address
   - Protects against single-source abuse
   - Extracted from `X-Forwarded-For` or `X-Real-IP` headers

**Example:**
```toml
requests_per_second = 100  # Steady state: 100 req/s
burst_size = 200           # Can handle burst of 200 requests
```

This allows:
- Sustained rate: 100 requests/second
- Burst traffic: Up to 200 requests instantly
- Recovery: Bucket refills at 100 tokens/second

## Endpoint Configuration

Endpoints define webhook paths and their routing to Danube topics.

### Basic Endpoint

```toml
[[endpoints]]
path = "/webhooks/payments"
danube_topic = "/stripe/payments"
partitions = 4
reliable_dispatch = true
```

### Endpoint Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `path` | string | ✅ | HTTP path for this endpoint (must start with `/`) |
| `danube_topic` | string | ✅ | Target Danube topic (`/{namespace}/{topic}`) |
| `partitions` | integer | optional | Number of partitions (0 or omitted = non-partitioned) |
| `reliable_dispatch` | boolean | optional | Enable reliable delivery (default: `false`) |

### Partitions

- **`partitions = 0`** or **omitted**: Non-partitioned topic (single partition)
- **`partitions > 0`**: Partitioned topic with specified number of partitions

**Examples:**
```toml
# Non-partitioned (sequential processing)
[[endpoints]]
path = "/webhooks/orders"
danube_topic = "/orders/events"
# partitions = 0  # Omitted = non-partitioned

# Partitioned (parallel processing)
[[endpoints]]
path = "/webhooks/analytics"
danube_topic = "/analytics/events"
partitions = 8  # 8 partitions for high throughput
```

### Reliable Dispatch

- **`reliable_dispatch = true`**: Messages are guaranteed to be delivered (slower, more reliable)
- **`reliable_dispatch = false`**: Best-effort delivery (faster, may lose messages)

**Use Cases:**
- **Reliable**: Financial transactions, orders, critical events
- **Non-reliable**: Analytics, logs, metrics, non-critical notifications

### Danube Topic Format

Danube topics must follow the format: `/{namespace}/{topic_name}`

**Valid:**
- `/stripe/payments`
- `/github/push`
- `/webhooks/generic`

**Invalid:**
- `/stripe/payments/succeeded` (too many segments)
- `stripe/payments` (missing leading slash)
- `/stripe` (only one segment)

### Per-Endpoint Rate Limiting (Optional)

Override platform-wide rate limits for specific endpoints:

```toml
[[endpoints]]
path = "/webhooks/high-volume"
danube_topic = "/analytics/events"
partitions = 16

[endpoints.rate_limit]
requests_per_second = 1000
burst_size = 2000
```

## Environment Variable Overrides

Environment variables can override **only secrets and connection URLs**:

### Supported Environment Variables

| Variable | Overrides | Use Case |
|----------|-----------|----------|
| `CONNECTOR_CONFIG_PATH` | - | **Required**: Path to TOML config file |
| `DANUBE_SERVICE_URL` | `core.danube_service_url` | Different environments (dev/staging/prod) |
| `CONNECTOR_NAME` | `core.connector_name` | Multiple connector instances |
| `WEBHOOK_API_KEY` | `auth.secret_env` | **Secret** - API key authentication |
| `WEBHOOK_HMAC_SECRET` | `auth.secret_env` | **Secret** - HMAC signature verification |
| `WEBHOOK_JWT_SECRET` | `auth.secret_env` | **Secret** - JWT token verification |

### NOT Supported via Environment Variables

The following **must** be in the TOML file:
- Endpoint paths and routing (`[[endpoints]]`)
- Partitions and reliable dispatch
- Rate limiting configuration
- Server settings (host, port, timeouts)
- Authentication type and headers

### Example Usage

```bash
# Set config file path (required)
export CONNECTOR_CONFIG_PATH=/etc/connector.toml

# Override for production
export DANUBE_SERVICE_URL=http://prod-broker:6650
export CONNECTOR_NAME=webhook-prod-1
export WEBHOOK_API_KEY=${VAULT_API_KEY}

# Run connector
./danube-source-webhook
```

## Configuration Examples

### Example 1: Simple API Key Authentication

```toml
[core]
connector_name = "webhook-simple"
danube_service_url = "http://localhost:6650"

[server]
host = "0.0.0.0"
port = 8080

[auth]
type = "apikey"
secret_env = "WEBHOOK_API_KEY"
header = "x-api-key"

[rate_limit]
requests_per_second = 100
burst_size = 200

[[endpoints]]
path = "/webhooks/events"
danube_topic = "/webhooks/events"
partitions = 4
reliable_dispatch = true
```

### Example 2: Stripe Webhooks with HMAC

```toml
[core]
connector_name = "stripe-webhooks"
danube_service_url = "http://danube-broker:6650"

[server]
host = "0.0.0.0"
port = 8080
max_body_size = 2097152  # 2MB for large payloads

[auth]
type = "hmac"
secret_env = "STRIPE_WEBHOOK_SECRET"
header = "Stripe-Signature"
algorithm = "sha256"

[rate_limit]
requests_per_second = 500
burst_size = 1000
per_ip_enabled = true
per_ip_requests_per_second = 50

# Payment events (critical)
[[endpoints]]
path = "/webhooks/stripe/payments"
danube_topic = "/stripe/payments"
partitions = 8
reliable_dispatch = true

# Customer events (non-critical)
[[endpoints]]
path = "/webhooks/stripe/customers"
danube_topic = "/stripe/customers"
partitions = 4
reliable_dispatch = false
```

### Example 3: GitHub Webhooks

```toml
[core]
connector_name = "github-webhooks"
danube_service_url = "http://danube-broker:6650"

[server]
host = "0.0.0.0"
port = 8080

[auth]
type = "hmac"
secret_env = "GITHUB_WEBHOOK_SECRET"
header = "X-Hub-Signature-256"
algorithm = "sha256"

[rate_limit]
requests_per_second = 200
burst_size = 400

# Push events
[[endpoints]]
path = "/webhooks/github/push"
danube_topic = "/github/push"
partitions = 4
reliable_dispatch = false

# Pull request events
[[endpoints]]
path = "/webhooks/github/pull_request"
danube_topic = "/github/pull_requests"
partitions = 2
reliable_dispatch = false

# Release events (important)
[[endpoints]]
path = "/webhooks/github/release"
danube_topic = "/github/releases"
partitions = 1
reliable_dispatch = true
```

### Example 4: Multi-Tenant with JWT

```toml
[core]
connector_name = "multi-tenant-webhooks"
danube_service_url = "http://danube-broker:6650"

[server]
host = "0.0.0.0"
port = 8080

[auth]
type = "jwt"
secret_env = "WEBHOOK_JWT_SECRET"

[rate_limit]
requests_per_second = 1000
burst_size = 2000
per_ip_enabled = true
per_ip_requests_per_second = 100

# Tenant A events
[[endpoints]]
path = "/webhooks/tenant-a/events"
danube_topic = "/tenants/tenant_a"
partitions = 8
reliable_dispatch = true

# Tenant B events
[[endpoints]]
path = "/webhooks/tenant-b/events"
danube_topic = "/tenants/tenant_b"
partitions = 8
reliable_dispatch = true

# Shared analytics
[[endpoints]]
path = "/webhooks/analytics"
danube_topic = "/shared/analytics"
partitions = 16
reliable_dispatch = false
```

### Example 5: Internal Microservices (No Auth)

```toml
[core]
connector_name = "internal-webhooks"
danube_service_url = "http://danube-broker:6650"

[server]
host = "0.0.0.0"
port = 8080

[auth]
type = "none"  # No authentication (internal network)

[rate_limit]
requests_per_second = 5000
burst_size = 10000

# Order events
[[endpoints]]
path = "/webhooks/orders"
danube_topic = "/internal/orders"
partitions = 16
reliable_dispatch = true

# Inventory updates
[[endpoints]]
path = "/webhooks/inventory"
danube_topic = "/internal/inventory"
partitions = 8
reliable_dispatch = true

# Notifications (non-critical)
[[endpoints]]
path = "/webhooks/notifications"
danube_topic = "/internal/notifications"
partitions = 4
reliable_dispatch = false
```

## Best Practices

### Security

1. **Never put secrets in TOML files**
   - Use environment variables: `WEBHOOK_API_KEY`, `WEBHOOK_HMAC_SECRET`
   - Store secrets in vault or secret manager
   - Rotate secrets regularly

2. **Use HTTPS in production**
   - Deploy behind reverse proxy (nginx, Traefik, Cloudflare)
   - Let reverse proxy handle TLS termination
   - Use strong TLS configuration (TLS 1.2+)

3. **Choose appropriate authentication**
   - **None**: Only for internal/trusted networks
   - **API Key**: Simple, good for custom webhooks
   - **HMAC**: Best for SaaS webhooks (Stripe, GitHub)
   - **JWT**: Best for service-to-service, multi-tenant

4. **Enable rate limiting**
   - Protect against abuse and DDoS
   - Use per-IP limiting for public endpoints
   - Adjust limits based on expected traffic

### Performance

1. **Partition configuration**
   - **High-volume**: More partitions (8-32) for parallel processing
   - **Low-volume**: Fewer partitions (1-4) to reduce overhead
   - **Ordered data**: Use 1 partition or non-partitioned

2. **Reliable dispatch**
   - **Critical data**: Enable reliable dispatch (payments, orders)
   - **Non-critical data**: Disable for better performance (logs, metrics)
   - **Mixed workload**: Configure per endpoint

3. **Rate limiting**
   - Set `requests_per_second` based on expected load
   - Set `burst_size` to 2x steady state for traffic spikes
   - Monitor rate limit violations

4. **Body size limits**
   - Default: 1MB (good for most webhooks)
   - Increase for large payloads (file uploads, large events)
   - Decrease for memory-constrained environments

### Endpoint Design

1. **Path naming**
   - Use clear, hierarchical paths: `/webhooks/stripe/payments`
   - Include provider name: `/webhooks/github/push`
   - Be consistent across endpoints

2. **Topic routing**
   - One endpoint → One topic (simple, clear)
   - Group related events: `/stripe/payments`, `/stripe/customers`
   - Use namespaces: `/stripe/*`, `/github/*`, `/internal/*`

3. **Endpoint order**
   - Order doesn't matter (exact path matching)
   - But keep related endpoints together for readability

### Monitoring

1. **Health checks**
   - Monitor `/health` endpoint
   - Set up alerts for failures
   - Include in load balancer health checks

2. **Logging**
   - Enable debug logging for troubleshooting: `RUST_LOG=debug`
   - Monitor authentication failures
   - Track rate limit violations

3. **Metrics**
   - Monitor request rate per endpoint
   - Track authentication success/failure rates
   - Monitor message publish latency

## Running with Configuration

### With Cargo

```bash
export CONNECTOR_CONFIG_PATH=config/connector.toml
export WEBHOOK_API_KEY=test-api-key-12345
cargo run --release
```

### With Docker

```bash
docker run -d \
  --name webhook-source \
  -p 8080:8080 \
  -v $(pwd)/connector.toml:/etc/connector.toml:ro \
  -e CONNECTOR_CONFIG_PATH=/etc/connector.toml \
  -e DANUBE_SERVICE_URL=http://danube-broker:6650 \
  -e CONNECTOR_NAME=webhook-source \
  -e WEBHOOK_API_KEY=your-secret-key \
  danube/source-webhook:latest
```

## Troubleshooting

### Configuration Errors

**Problem:** Connector fails to start with config error

**Solutions:**
- Verify TOML syntax is valid
- Check all required fields are present
- Verify Danube topic format: `/{namespace}/{topic}`
- Ensure endpoint paths start with `/`

### Authentication Issues

**Problem:** All requests return 401 Unauthorized

**Solutions:**
- Verify `secret_env` environment variable is set
- Check header name matches config
- For HMAC, verify signature computation
- For JWT, check token hasn't expired

### Rate Limiting Issues

**Problem:** Legitimate traffic being rate limited

**Solutions:**
- Increase `requests_per_second` and `burst_size`
- Check if per-IP limiting is too restrictive
- Monitor actual traffic patterns
- Consider per-endpoint rate limit overrides

### Connection Issues

**Problem:** Can't connect to Danube broker

**Solutions:**
- Verify `danube_service_url` is correct
- Check network connectivity: `curl http://danube-broker:6650`
- Verify Danube broker is running
- Check firewall rules

## Additional Resources

- [Axum Web Framework](https://github.com/tokio-rs/axum)
- [Danube Documentation](https://github.com/danrusei/danube)
- [Complete Example](../../../examples/source-webhook/)
- [Stripe Webhook Documentation](https://stripe.com/docs/webhooks)
- [GitHub Webhook Documentation](https://docs.github.com/en/webhooks)
