# HTTP/Webhook Source Connector

A high-performance HTTP/Webhook source connector that receives webhooks from external services and publishes them to Danube topics. Built for reliability, security, and scalability.

## ✨ Features

- 🌐 **HTTP Server** - Production-ready HTTP server built with Axum
- 🔐 **Multiple Auth Methods** - API Key, HMAC signatures, JWT, or None
- 🚦 **Rate Limiting** - Per-endpoint and per-IP rate limiting with token bucket algorithm
- 🎯 **Multi-Endpoint Routing** - Route different webhook paths to different Danube topics
- 📦 **Partitioned Topics** - Per-endpoint partition configuration for parallel processing
- 🛡️ **Reliable Dispatch** - Configurable reliable/non-reliable delivery per endpoint
- 🎨 **Schema Validation** - Optional JSON Schema validation with auto-registration
- 📝 **Metadata Enrichment** - Automatic enrichment with timestamp, IP, headers, user-agent
- ⚡ **High Performance** - Async I/O with middleware-based architecture
- 🏥 **Health Checks** - Built-in health and readiness endpoints
- 📊 **Observability** - Structured logging with tracing

**Supported Authentication:**
- **None** - No authentication (for internal/trusted webhooks)
- **API Key** - Header-based API key authentication
- **HMAC** - Signature-based authentication (Stripe, GitHub, Shopify style)
- **JWT** - JSON Web Token authentication

**Use Cases:** SaaS webhooks (Stripe, GitHub, Shopify), event ingestion, API gateways, webhook proxies, real-time notifications

## 🚀 Quick Start

### Running with Docker

```bash
docker run -d \
  --name webhook-source \
  -p 8080:8080 \
  -v $(pwd)/connector.toml:/etc/connector.toml:ro \
  -e CONNECTOR_CONFIG_PATH=/etc/connector.toml \
  -e DANUBE_SERVICE_URL=http://danube-broker:6650 \
  -e CONNECTOR_NAME=webhook-source \
  -e WEBHOOK_API_KEY=your-secret-api-key \
  danube/source-webhook:latest
```

**Note:** All structural configuration (endpoints, topics, partitions, rate limits) must be in `connector.toml`. See [Configuration](#configuration) section below.

### Complete Example

For a complete working setup with Docker Compose, test webhooks, and step-by-step guide:

👉 **See [example/README.md](example/README.md)**

The example includes:
- Docker Compose setup (Danube + ETCD + Webhook Connector)
- Pre-configured connector.toml with 4 endpoints and 2 schemas
- Schema validation testing (JSON Schema + String schema)
- Test webhook publisher script
- Authentication and rate limiting examples

## ⚙️ Configuration

### 📖 Complete Configuration Guide

See **[config/README.md](config/README.md)** for comprehensive configuration documentation including:
- Core and HTTP server settings
- Endpoint configuration and routing
- Authentication methods (API Key, HMAC, JWT)
- Rate limiting configuration
- Environment variable reference
- Configuration examples and best practices

### 📄 Quick Reference

#### Minimal Configuration Example

```toml
# connector.toml

# Core Danube settings (flattened)
danube_service_url = "http://danube-broker:6650"
connector_name = "webhook-source"

# Processing and retry settings
[processing]
batch_size = 100
poll_interval_ms = 100
metrics_port = 9090

[retry]
initial_interval_ms = 1000
max_interval_ms = 60000
max_retries = 5

# Optional: Schema validation
[[schemas]]
topic = "/stripe/payments"
subject = "stripe-payment-v1"
schema_type = "json_schema"
schema_file = "schemas/payment.json"
auto_register = true
version_strategy = "latest"

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

# Stripe payment webhooks
[[endpoints]]
path = "/webhooks/stripe/payments"
danube_topic = "/stripe/payments"
partitions = 4
reliable_dispatch = true

# GitHub push events
[[endpoints]]
path = "/webhooks/github/push"
danube_topic = "/github/push"
partitions = 2
reliable_dispatch = false
```

#### Environment Variable Overrides

Environment variables can override **only secrets and connection URLs**:

| Variable | Description | Example |
|----------|-------------|---------|
| `CONNECTOR_CONFIG_PATH` | Path to TOML config (required) | `/etc/connector.toml` |
| `DANUBE_SERVICE_URL` | Override Danube broker URL | `http://prod-broker:6650` |
| `CONNECTOR_NAME` | Override connector name | `webhook-production` |
| `WEBHOOK_API_KEY` | API key for authentication (secret) | `sk_live_abc123...` |
| `WEBHOOK_HMAC_SECRET` | HMAC signing secret (secret) | `whsec_xyz789...` |
| `WEBHOOK_JWT_SECRET` | JWT verification secret (secret) | `jwt_secret_key` |

See [config/README.md](config/README.md) for complete configuration documentation.

## 🛠️ Development

### Building

```bash
# Build release binary
cargo build --release --package danube-source-webhook

# Run tests
cargo test --package danube-source-webhook

# Build Docker image
docker build -f source-webhook/Dockerfile -t danube/source-webhook:latest ../..
```

### Running from Source

```bash
# Run with configuration file
export CONNECTOR_CONFIG_PATH=config/connector.toml
export WEBHOOK_API_KEY=test-api-key-12345
cargo run --release

# With environment overrides
export CONNECTOR_CONFIG_PATH=config/connector.toml
export DANUBE_SERVICE_URL=http://localhost:6650
export WEBHOOK_API_KEY=test-api-key-12345
cargo run --release --package danube-source-webhook
```

### Testing Webhooks

Send a test webhook:

```bash
curl -X POST http://localhost:8080/webhooks/stripe/payments \
  -H "Content-Type: application/json" \
  -H "x-api-key: test-api-key-12345" \
  -d '{
    "event": "payment.succeeded",
    "amount": 5000,
    "currency": "usd",
    "customer_id": "cus_test123"
  }'
```

### Message Attributes

Each message published to Danube includes webhook metadata as attributes:

```json
{
  "webhook.source": "webhook-source",
  "webhook.endpoint": "/webhooks/stripe/payments",
  "webhook.timestamp": "2024-12-23T10:00:00Z",
  "webhook.ip": "192.168.1.100",
  "webhook.user_agent": "Stripe/1.0",
  "webhook.content_type": "application/json"
}
```

These attributes are queryable in Danube consumers and useful for filtering, routing, and debugging.

## 📚 Documentation

### Complete Working Example

See **[example/](example/)** for a complete setup with:
- Docker Compose (Danube + ETCD + Webhook Connector)
- Schema validation testing (JSON Schema + String schema)
- Test webhook publishers
- Step-by-step testing guide
- Authentication and rate limiting examples

### Configuration Examples

- **[config/connector.toml](config/connector.toml)** - Fully documented reference configuration
- **[config/README.md](config/README.md)** - Complete configuration guide

### How It Works

```
External Services → HTTP Server → Auth Middleware → Rate Limit → Route to Danube
(Stripe, GitHub)      (Port 8080)    (API Key/HMAC/JWT)   (Token Bucket)   (Topics)
```

**Request Flow:**
1. External service sends HTTP POST to configured endpoint (e.g., `/webhooks/stripe/payments`)
2. Auth middleware validates credentials (API Key, HMAC signature, or JWT)
3. Rate limit middleware checks per-endpoint and per-IP limits
4. Handler validates endpoint exists in configuration
5. Payload is enriched with metadata (timestamp, IP, headers)
6. Message is published to corresponding Danube topic
7. Returns 200 OK or appropriate error code

### Endpoints

The connector exposes several endpoints:

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/webhooks/*` | POST | Configured webhook endpoints |
| `/health` | GET | Health check (no auth) |
| `/ready` | GET | Readiness check (no auth) |

### Authentication Methods

#### API Key
```bash
curl -X POST http://localhost:8080/webhooks/generic \
  -H "x-api-key: your-api-key" \
  -d '{"event": "test"}'
```

#### HMAC (Stripe-style)
```bash
# Signature computed from body + secret
curl -X POST http://localhost:8080/webhooks/stripe/payments \
  -H "Stripe-Signature: t=1234567890,v1=signature_hash" \
  -d '{"event": "payment.succeeded"}'
```

#### JWT
```bash
curl -X POST http://localhost:8080/webhooks/generic \
  -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..." \
  -d '{"event": "test"}'
```

### References

- **[Configuration Guide](config/README.md)** - Complete configuration reference
- **[Working Example](../../examples/source-webhook)** - Docker Compose setup
- [Axum Web Framework](https://github.com/tokio-rs/axum)
- [Danube Messaging](https://github.com/danrusei/danube)

## 🔒 Security Considerations

### Production Deployment

1. **Use HTTPS** - Deploy behind a reverse proxy with TLS (nginx, Traefik, Cloudflare)
2. **Strong Secrets** - Use cryptographically secure random keys (32+ characters)
3. **HMAC for SaaS** - Use HMAC authentication for Stripe, GitHub, Shopify webhooks
4. **Rate Limiting** - Enable per-IP rate limiting to prevent abuse
5. **IP Whitelisting** - Restrict webhook sources by IP at firewall level
6. **Monitor Logs** - Watch for authentication failures and rate limit violations

### Authentication Best Practices

- **API Key**: Rotate keys regularly, use different keys per environment
- **HMAC**: Verify signatures match webhook provider's documentation
- **JWT**: Use RS256 (public key) instead of HS256 (shared secret) when possible
- **None**: Only use for internal/trusted networks behind firewall

## 📊 Monitoring

### Health Checks

```bash
# Health check
curl http://localhost:8080/health

# Readiness check
curl http://localhost:8080/ready
```

### Logging

The connector uses structured logging with tracing:

```bash
# Set log level
export RUST_LOG=info,danube_source_webhook=debug

# View logs
docker logs -f webhook-source
```

### Metrics

Monitor these key metrics:
- Request rate per endpoint
- Authentication failures
- Rate limit violations
- Message publish latency
- Error rates

## 🚨 Troubleshooting

### Webhooks Returning 401 Unauthorized

**Problem:** Authentication failures

**Solutions:**
- Verify API key matches `WEBHOOK_API_KEY` environment variable
- Check header name matches config (`x-api-key` by default)
- For HMAC, verify signature computation matches provider's spec
- For JWT, check token hasn't expired

### Webhooks Returning 404 Not Found

**Problem:** Endpoint not configured

**Solutions:**
- Verify endpoint path matches configuration exactly (case-sensitive)
- Check `[[endpoints]]` section in `connector.toml`
- Ensure path starts with `/` in both config and request

### Webhooks Returning 429 Too Many Requests

**Problem:** Rate limit exceeded

**Solutions:**
- Increase `requests_per_second` in config
- Increase `burst_size` for traffic spikes
- Check if per-IP rate limiting is too restrictive
- Verify legitimate traffic isn't being blocked

### Messages Not Appearing in Danube

**Problem:** Publish failures

**Solutions:**
- Check connector logs for publish errors
- Verify Danube broker is running and accessible
- Check topic name format: `/{namespace}/{topic}` (exactly 2 segments)
- Verify namespace exists in Danube broker
- Check network connectivity to Danube broker
- If using schemas: verify schema files exist and are valid JSON Schema
- Check schema validation errors in logs

### High Latency

**Problem:** Slow webhook processing

**Solutions:**
- Increase Danube topic partitions for parallel processing
- Use non-reliable dispatch for non-critical webhooks
- Check Danube broker performance
- Monitor CPU and memory usage
- Consider horizontal scaling (multiple connector instances)

## 🎯 Use Cases

### Stripe Webhooks with Schema Validation

```toml
# Schema validation for payment webhooks
[[schemas]]
topic = "/stripe/payments"
subject = "stripe-payment-v1"
schema_type = "json_schema"
schema_file = "schemas/payment.json"
auto_register = true
version_strategy = "latest"

[[endpoints]]
path = "/webhooks/stripe/payments"
danube_topic = "/stripe/payments"
partitions = 4
reliable_dispatch = true  # Critical payment events

[auth]
type = "hmac"
secret_env = "STRIPE_WEBHOOK_SECRET"
header = "Stripe-Signature"
algorithm = "sha256"
```

### GitHub Webhooks

```toml
[[endpoints]]
path = "/webhooks/github/push"
danube_topic = "/github/push"
partitions = 2
reliable_dispatch = false  # Non-critical notifications

[auth]
type = "hmac"
secret_env = "GITHUB_WEBHOOK_SECRET"
header = "X-Hub-Signature-256"
algorithm = "sha256"
```

### Internal Microservices

```toml
[[endpoints]]
path = "/webhooks/orders"
danube_topic = "/internal/orders"
partitions = 8
reliable_dispatch = true

[auth]
type = "apikey"
secret_env = "INTERNAL_API_KEY"
header = "x-api-key"
```

## License

Apache License 2.0 - See [LICENSE](../../LICENSE)
