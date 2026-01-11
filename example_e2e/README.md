# End-to-End Example: Webhook Source → Danube → SurrealDB Sink

This example demonstrates a complete real-time data pipeline using Danube Connect with pre-built Docker images.

## Architecture

```
External System (Stripe/GitHub)
        ↓
   HTTP POST /webhooks
        ↓
┌─────────────────────┐
│ Webhook Connector   │  (Port 8080)
│  - Validates API Key │
│  - Validates Schema  │
└─────────────────────┘
        ↓
    Publishes to
        ↓
┌─────────────────────┐
│  Danube Broker      │  (Port 6650)
│  Topic: /stripe/    │
│         payments    │
└─────────────────────┘
        ↓
    Consumes from
        ↓
┌─────────────────────┐
│ SurrealDB Sink      │
│  - Batch Processing │
│  - Schema Validation│
└─────────────────────┘
        ↓
    Writes to
        ↓
┌─────────────────────┐
│    SurrealDB        │  (Port 8000)
│ Table: stripe_      │
│        payments     │
└─────────────────────┘
```

## Components

### Infrastructure
- **etcd** - Distributed metadata storage for Danube broker
- **danube-broker** - Message broker with schema registry

### Connectors (Pre-built Images)
- **webhook-connector** - `ghcr.io/danube-messaging/danube-source-webhook:v0.2.0`
  - Receives HTTP webhooks
  - Validates API keys
  - Enforces JSON schema validation
  - Publishes to Danube topics

- **surrealdb-sink** - `ghcr.io/danube-messaging/danube-sink-surrealdb:v0.2.0`
  - Consumes from Danube topics
  - Batch processing (50 msgs/batch)
  - Writes to SurrealDB tables

### Database
- **surrealdb** - Multi-model database (in-memory mode)

## Files Overview

```
example_end_to_end/
├── docker-compose.yml              # Complete pipeline setup
├── webhook-connector.toml          # Webhook source configuration
├── surrealdb-sink-connector.toml   # SurrealDB sink configuration
├── danube_broker.yml               # Danube broker settings
├── schemas/
│   └── payment.json                # Payment event JSON Schema
├── send-webhook.sh                 # Script to send test webhooks
└── README.md                       # This file
```

### Configuration Files

**webhook-connector.toml**
- Defines webhook endpoints and target Danube topics
- Configures authentication (API key)
- Maps schemas to topics for validation

**surrealdb-sink-connector.toml**
- Maps Danube topics to SurrealDB tables
- Configures subscription and batch settings
- Defines schema validation rules

**schemas/payment.json**
- JSON Schema for Stripe-style payment events
- Validates required fields: `event`, `amount`, `currency`, `timestamp`

## Prerequisites

- Docker and Docker Compose installed
- Ports available: `2379`, `2380`, `6650`, `8000`, `8080`, `9040`, `9091`, `9092`, `50051`

## Step-by-Step Usage

### 1. Start the Pipeline

```bash
cd example_end_to_end
docker-compose up -d
```

This will start all services. Wait ~15 seconds for initialization.

### 2. Verify Services are Running

```bash
# Check all containers
docker-compose ps

# Check logs
docker-compose logs -f webhook-connector surrealdb-sink
```

You should see:
- Webhook connector: "HTTP server started on 0.0.0.0:8080"
- SurrealDB sink: "Subscribed to topic /stripe/payments"

### 3. Send Test Webhooks

Use the provided script to send sample payment webhooks:

```bash
# Send a single webhook
./send-webhook.sh

# Send multiple webhooks
./send-webhook.sh 10

# Load test with 100 webhooks
./send-webhook.sh load
```

Or manually with curl:

```bash
curl -X POST http://localhost:8080/webhooks/stripe/payments \
  -H "Content-Type: application/json" \
  -H "x-api-key: demo-api-key-e2e-12345" \
  -d '{
    "event": "payment.succeeded",
    "amount": 9999,
    "currency": "USD",
    "timestamp": '$(date +%s)',
    "customer_id": "cus_abc123"
  }'
```

Expected response:
```json
{"status":"accepted","message":"Webhook received"}
```

### 4. Query Data in SurrealDB

Connect to the SurrealDB SQL shell:

```bash
docker exec -it e2e-surrealdb /surreal sql \
  --conn http://localhost:8000 \
  --user root \
  --pass root \
  --ns default \
  --db default
```

Run queries:

```sql
-- View all payments
SELECT * FROM stripe_payments ORDER BY timestamp DESC LIMIT 10;

-- Count by event type
SELECT event, count() AS total FROM stripe_payments GROUP BY event;

-- Calculate total revenue
SELECT math::sum(amount) / 100 AS total_usd 
FROM stripe_payments 
WHERE event = 'payment.succeeded';

-- Recent successful payments
SELECT * FROM stripe_payments 
WHERE event = 'payment.succeeded' 
ORDER BY timestamp DESC 
LIMIT 5;
```

### 5. Monitor the Pipeline

#### View Logs
```bash
# Webhook connector logs
docker-compose logs -f webhook-connector

# SurrealDB sink logs
docker-compose logs -f surrealdb-sink

# All logs
docker-compose logs -f
```

#### Check Metrics
- Webhook Connector: `http://localhost:9091/metrics`
- SurrealDB Sink: `http://localhost:9092/metrics`
- Danube Broker: `http://localhost:9040/metrics`

#### Health Checks
```bash
# Webhook connector
curl http://localhost:8080/health

# SurrealDB
curl http://localhost:8000/health
```

### 6. Stop the Pipeline

```bash
# Stop services
docker-compose down

# Stop and remove all data
docker-compose down -v
```

## Data Flow Example

1. **External system sends webhook**
   ```bash
   POST /webhooks/stripe/payments
   Content-Type: application/json
   x-api-key: demo-api-key-e2e-12345
   
   {
     "event": "payment.succeeded",
     "amount": 9999,
     "currency": "USD",
     "timestamp": 1704067200,
     "customer_id": "cus_abc123"
   }
   ```

2. **Webhook connector validates**
   - ✅ API key authentication
   - ✅ JSON schema validation (required fields, types, formats)
   - ✅ Publishes to Danube topic `/stripe/payments`

3. **Danube broker distributes**
   - Message stored in topic with 4 partitions
   - Schema metadata attached
   - Guaranteed delivery to subscribers

4. **SurrealDB sink processes**
   - Consumes message from subscription
   - Validates against registered schema
   - Batches up to 50 messages
   - Flushes every 2 seconds

5. **Data stored in SurrealDB**
   ```sql
   {
     event: "payment.succeeded",
     amount: 9999,
     currency: "USD",
     timestamp: 1704067200,
     customer_id: "cus_abc123",
     _danube_metadata: {
       topic: "/stripe/payments",
       offset: 123,
       message_id: "...",
       publish_time: "2024-01-01T00:00:00Z"
     }
   }
   ```

## Testing Scenarios

### Valid Webhook
```bash
curl -X POST http://localhost:8080/webhooks/stripe/payments \
  -H "Content-Type: application/json" \
  -H "x-api-key: demo-api-key-e2e-12345" \
  -d '{
    "event": "payment.succeeded",
    "amount": 5000,
    "currency": "USD",
    "timestamp": '$(date +%s)'
  }'
```
✅ Expected: HTTP 200, data appears in SurrealDB

### Invalid Schema (Missing Required Field)
```bash
curl -X POST http://localhost:8080/webhooks/stripe/payments \
  -H "Content-Type: application/json" \
  -H "x-api-key: demo-api-key-e2e-12345" \
  -d '{
    "event": "payment.succeeded",
    "amount": 5000
  }'
```
❌ Expected: HTTP 400 with schema validation error

### Invalid Authentication
```bash
curl -X POST http://localhost:8080/webhooks/stripe/payments \
  -H "Content-Type: application/json" \
  -H "x-api-key: wrong-key" \
  -d '{
    "event": "payment.succeeded",
    "amount": 5000,
    "currency": "USD",
    "timestamp": '$(date +%s)'
  }'
```
❌ Expected: HTTP 401 Unauthorized

## Troubleshooting

### Webhook connector fails to start
**Error**: "Unable to find the namespace stripe"

**Solution**: Ensure topic-init completed successfully
```bash
docker-compose logs topic-init
```
Should show: "Creating namespace: stripe... ✓"

### No data in SurrealDB
**Check**: SurrealDB sink logs
```bash
docker-compose logs surrealdb-sink
```
Look for: "Subscribed to topic /stripe/payments"

**Check**: If webhooks were sent
```bash
# Send a test webhook
./send-webhook.sh
```

### Schema validation errors
**Check**: Schema definition matches your payload
```bash
cat schemas/payment.json
```

Required fields:
- `event` (string, one of: payment.succeeded, payment.failed, payment.canceled, payment.refunded)
- `amount` (integer, >= 0)
- `currency` (string, 3-letter uppercase code)
- `timestamp` (integer, unix timestamp)

## Next Steps

- **Customize the schema**: Edit `schemas/payment.json` for your data model
- **Add more endpoints**: Update `webhook-connector.toml` to add webhook endpoints
- **Scale the sink**: Add more sink connector instances for higher throughput
- **Persistent storage**: Configure SurrealDB with file or TiKV storage instead of memory
- **Add monitoring**: Integrate Prometheus and Grafana for observability

## Additional Documentation

- [Webhook Source Connector Documentation](../source-webhook/README.md)
- [SurrealDB Sink Connector Documentation](../sink-surrealdb/README.md)
- [Danube Messaging Platform](https://danube-docs.dev-state.com)
- [Schema Registry Guide](https://danube-docs.dev-state.com/schema-registry/)

## License

Apache License 2.0
