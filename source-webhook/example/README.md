# Webhook Source Connector Example

This example demonstrates how to use the **HTTP/Webhook Source Connector** to receive webhooks from external services and publish them to Danube topics.

## Overview

The webhook connector acts as an HTTP server that:
- ✅ Receives webhooks via HTTP POST requests
- ✅ Authenticates requests (API Key, HMAC, JWT, or None)
- ✅ Rate limits requests (per-endpoint and per-IP)
- ✅ Routes webhooks to different Danube topics based on endpoint path
- ✅ Enriches messages with metadata (timestamp, IP, headers)
- ✅ Supports both partitioned and non-partitioned topics
- ✅ Configurable reliable/non-reliable dispatch per endpoint

## Architecture

```
External Services          Webhook Connector              Danube Broker
   (Stripe, GitHub, etc)
                                                           
   POST /webhooks/stripe    ┌─────────────────┐          ┌──────────────┐
   ──────────────────────> │  HTTP Server    │          │              │
                           │  (Port 8080)    │          │   Topics:    │
   POST /webhooks/github   │                 │          │              │
   ──────────────────────> │  • Auth Check   │ ───────> │ /stripe/*    │
                           │  • Rate Limit   │          │ /github/*    │
   POST /webhooks/generic  │  • Validation   │          │ /webhooks/*  │
   ──────────────────────> │                 │          │              │
                           └─────────────────┘          └──────────────┘
```

## Components

This example includes:

1. **etcd** - Metadata store for Danube
2. **Danube Broker** - Message broker with 4 namespaces (default, webhooks, stripe, github)
3. **Webhook Connector** - HTTP server receiving webhooks
4. **Test Publisher** - Simulates external services sending webhooks

## Quick Start

### 1. Start the Stack

```bash
cd examples/source-webhook
docker-compose up -d
```

This will start:
- etcd (metadata store)
- Danube broker (message broker)
- Webhook connector (HTTP server on port 8080)
- Test publisher (sends sample webhooks every 5 seconds)

### 2. Check Status

```bash
# Check all containers are running
docker-compose ps

# Check webhook connector logs
docker-compose logs -f webhook-connector

# Check Danube broker logs
docker-compose logs -f danube-broker

# Check test publisher logs
docker-compose logs -f webhook-test-publisher
```

### 3. Test Manually

Send a test webhook:

```bash
curl -X POST http://localhost:8080/webhooks/stripe/payments \
  -H "Content-Type: application/json" \
  -H "x-api-key: test-api-key-12345" \
  -d '{
    "event": "payment.succeeded",
    "amount": 5000,
    "currency": "usd",
    "customer_id": "cus_test123",
    "timestamp": 1234567890
  }'
```

Expected response: `200 OK`

### 4. Test Authentication

**Valid API Key:**
```bash
curl -X POST http://localhost:8080/webhooks/generic \
  -H "Content-Type: application/json" \
  -H "x-api-key: test-api-key-12345" \
  -d '{"event": "test"}'
```
Response: `200 OK`

**Invalid API Key:**
```bash
curl -X POST http://localhost:8080/webhooks/generic \
  -H "Content-Type: application/json" \
  -H "x-api-key: wrong-key" \
  -d '{"event": "test"}'
```
Response: `401 Unauthorized`

**Missing API Key:**
```bash
curl -X POST http://localhost:8080/webhooks/generic \
  -H "Content-Type: application/json" \
  -d '{"event": "test"}'
```
Response: `401 Unauthorized`

### 5. Test Invalid Endpoint

```bash
curl -X POST http://localhost:8080/webhooks/invalid \
  -H "Content-Type: application/json" \
  -H "x-api-key: test-api-key-12345" \
  -d '{"event": "test"}'
```
Response: `404 Not Found`

### 6. Health Check

```bash
curl http://localhost:8080/health
```

Response:
```json
{
  "status": "healthy",
  "timestamp": "2024-12-23T10:00:00Z"
}
```

### 7. Use Test Publisher Script

Run the standalone test publisher:

```bash
./test-publisher.sh
```

This will continuously send webhooks to all configured endpoints and show the results.

## Configuration

### Endpoints

The example includes 4 webhook endpoints:

| Endpoint | Danube Topic | Partitions | Reliable | Use Case |
|----------|--------------|------------|----------|----------|
| `/webhooks/stripe/payments` | `/stripe/payments` | 4 | ✅ Yes | Critical payment events |
| `/webhooks/github/push` | `/github/push` | 2 | ❌ No | Git push notifications |
| `/webhooks/generic` | `/webhooks/generic` | 0 (non-partitioned) | ✅ Yes | Generic events |
| `/webhooks/alerts` | `/webhooks/alerts` | 0 (non-partitioned) | ❌ No | Monitoring alerts |

### Authentication

The example uses **API Key** authentication:
- Header: `x-api-key`
- Value: `test-api-key-12345` (set via `WEBHOOK_API_KEY` env var)

To change authentication method, edit `connector.toml`:

```toml
[auth]
type = "apikey"  # Options: "none", "apikey", "hmac", "jwt"
secret_env = "WEBHOOK_API_KEY"
header = "x-api-key"
```

### Rate Limiting

Platform-wide rate limiting:
- **100 requests/second** per endpoint
- **200 burst** capacity
- **10 requests/second** per IP (when enabled)

## Monitoring

### Connector Logs

```bash
docker-compose logs -f webhook-connector
```

You'll see:
- Incoming webhook requests
- Authentication results
- Rate limiting events
- Message publishing to Danube

### Danube Metrics

Prometheus metrics available at:
```bash
curl http://localhost:9040/metrics
```

### Test Publisher Logs

```bash
docker-compose logs -f webhook-test-publisher
```

Shows HTTP response codes for each webhook sent.

## Advanced Usage

### Custom Webhook Payload

Send your own webhook with custom data:

```bash
curl -X POST http://localhost:8080/webhooks/generic \
  -H "Content-Type: application/json" \
  -H "x-api-key: test-api-key-12345" \
  -d '{
    "event": "custom.event",
    "user_id": "user_123",
    "action": "login",
    "metadata": {
      "ip": "192.168.1.1",
      "user_agent": "Mozilla/5.0"
    },
    "timestamp": 1234567890
  }'
```

### Simulate Stripe Webhook

```bash
curl -X POST http://localhost:8080/webhooks/stripe/payments \
  -H "Content-Type: application/json" \
  -H "x-api-key: test-api-key-12345" \
  -d '{
    "id": "evt_1234567890",
    "object": "event",
    "type": "payment_intent.succeeded",
    "data": {
      "object": {
        "id": "pi_1234567890",
        "amount": 2000,
        "currency": "usd",
        "status": "succeeded"
      }
    }
  }'
```

### Simulate GitHub Webhook

```bash
curl -X POST http://localhost:8080/webhooks/github/push \
  -H "Content-Type: application/json" \
  -H "x-api-key: test-api-key-12345" \
  -d '{
    "ref": "refs/heads/main",
    "repository": {
      "name": "danube-connect",
      "full_name": "danube-messaging/danube-connect"
    },
    "pusher": {
      "name": "developer",
      "email": "dev@example.com"
    },
    "commits": [
      {
        "id": "abc123",
        "message": "Fix bug",
        "author": {
          "name": "Developer",
          "email": "dev@example.com"
        }
      }
    ]
  }'
```

## Consuming Messages from Danube

To verify webhook messages are reaching Danube, consume them using **danube-cli**.

### Download danube-cli

**GitHub Releases:** https://github.com/danube-messaging/danube/releases  
**Documentation:** https://danube-docs.dev-state.com/danube_clis/danube_cli/consumer/

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

**Available platforms:**
- Linux: `danube-cli-linux`
- macOS (Apple Silicon): `danube-cli-macos`
- Windows: `danube-cli-windows.exe`

Or use the Docker image:
```bash
docker pull ghcr.io/danube-messaging/danube-cli:v0.5.2
```

### Consume Webhook Messages

**Topic Mappings (Webhook Endpoint → Danube):**

The connector routes webhook requests to Danube topics based on `connector.toml`:

| Webhook Endpoint | Danube Topic | Description |
|-----------------|--------------|-------------|
| `/webhooks/stripe/payments` | `/stripe/payments` | Stripe payment events (4 partitions, reliable) |
| `/webhooks/github/push` | `/github/push` | GitHub push events (2 partitions, non-reliable) |
| `/webhooks/generic` | `/webhooks/generic` | Generic webhooks (non-partitioned, reliable) |
| `/webhooks/alerts` | `/webhooks/alerts` | Alert webhooks (non-partitioned, non-reliable) |

**Consume messages from specific Danube topics:**

```bash
# Consume Stripe payment webhooks
danube-cli consume \
  --service-addr http://localhost:6650 \
  --topic /stripe/payments \
  --subscription stripe-sub

# Consume GitHub push events
danube-cli consume \
  --service-addr http://localhost:6650 \
  --topic /github/push \
  --subscription github-sub

# Consume generic webhooks
danube-cli consume \
  --service-addr http://localhost:6650 \
  --topic /webhooks/generic \
  --subscription generic-sub

# With exclusive subscription (only one consumer receives messages)
danube-cli consume \
  -s http://localhost:6650 \
  -t /webhooks/alerts \
  -m alert-exclusive \
  --sub-type exclusive

# You should see webhook payloads appearing in real-time:
# Message received: {"event":"payment.succeeded","amount":5000,"currency":"usd"}
```

### Using a Sink Connector

You can chain this with a sink connector to forward webhooks to:
- **SurrealDB** - Store webhook events in a database
- **Qdrant** - Index webhook data for vector search
- **Custom Sink** - Process webhooks with your own logic

## Configuration Reference

### Connector Configuration (`connector.toml`)

```toml
[core]
danube_service_url = "http://danube-broker:6650"
connector_name = "webhook-source-example"

[server]
host = "0.0.0.0"
port = 8080
timeout_seconds = 30
max_body_size = 1048576  # 1MB

[auth]
type = "apikey"          # none, apikey, hmac, jwt
secret_env = "WEBHOOK_API_KEY"
header = "x-api-key"

[rate_limit]
requests_per_second = 100
burst_size = 200
per_ip_enabled = true
per_ip_requests_per_second = 10

[[endpoints]]
path = "/webhooks/stripe/payments"
danube_topic = "/stripe/payments"
partitions = 4
reliable_dispatch = true
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `CONNECTOR_CONFIG_PATH` | Path to connector.toml | `/etc/connector.toml` |
| `DANUBE_SERVICE_URL` | Danube broker URL | From config |
| `CONNECTOR_NAME` | Unique connector name | From config |
| `WEBHOOK_API_KEY` | API key for authentication | Required if auth.type=apikey |
| `RUST_LOG` | Logging level | `info` |

## Troubleshooting

### Connector Not Starting

```bash
# Check logs
docker-compose logs webhook-connector

# Common issues:
# - Danube broker not ready
# - Invalid configuration
# - Missing API key environment variable
```

### Webhooks Returning 401

- Check API key is correct: `test-api-key-12345`
- Verify header name: `x-api-key`
- Check authentication type in config

### Webhooks Returning 404

- Verify endpoint path matches configuration
- Check `connector.toml` endpoints section
- Endpoint paths are case-sensitive

### Webhooks Returning 429

- Rate limit exceeded
- Adjust `requests_per_second` in config
- Check per-IP rate limiting settings

### Messages Not Appearing in Danube

```bash
# Check connector logs for publish errors
docker-compose logs webhook-connector | grep -i error

# Verify Danube broker is running
docker-compose ps danube-broker

# Check Danube broker logs
docker-compose logs danube-broker
```

## Cleanup

Stop and remove all containers:

```bash
docker-compose down
```

Remove volumes (data will be lost):

```bash
docker-compose down -v
```

## Resources

- [Danube Documentation](https://github.com/danube-messaging/danube)
- [Webhook Connector Source Code](../../connectors/source-webhook/)
- [Configuration Reference](../../connectors/source-webhook/config/connector.toml)
