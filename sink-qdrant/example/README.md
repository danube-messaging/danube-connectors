# Qdrant Sink Connector Example

Complete working example demonstrating the Qdrant Sink Connector for building RAG (Retrieval Augmented Generation) pipelines.

## Overview

This example shows how to:
1. Run Danube broker, Qdrant, and the connector with Docker Compose
2. Generate embeddings and send them to Danube
3. Automatically stream vectors to Qdrant
4. Perform semantic search over the vectors

## Architecture

```
┌─────────────────┐
│  Test Producer  │
│ (Docker Tools)  │
└────────┬────────┘
         │ Embeddings
         ▼
┌─────────────────┐
│ Danube Broker   │
│  Topic: vectors │
│  Embedded Raft  │
└────────┬────────┘
         │ Stream
         ▼
┌─────────────────┐
│ Qdrant Sink     │
│   Connector     │
└────────┬────────┘
         │ Batch Upsert
         ▼
┌─────────────────┐
│    Qdrant       │
│  Vector Store   │
└─────────────────┘
```

## Quick Start

### 1. Start the Stack

```bash
# Start all services (Danube, Topic Init, Qdrant, Connector)
docker-compose up -d

# Check logs
docker-compose logs -f qdrant-sink

# Verify all services are healthy
docker-compose ps
```

**Startup Sequence:**
1. **Danube Broker** starts as a single-node broker using embedded Raft metadata
2. **Topic Init** registers schema and creates `/default/vectors` topic with validation (depends on Danube)
3. **Qdrant** starts independently and becomes healthy
4. **Qdrant Sink** starts (depends on topic creation + Qdrant health)

**Shared Danube broker config:**
- The example mounts `../../example_shared/danube_broker_no_auth.yml`
- Update that single file when Danube broker config changes for all connector examples

Services:
- **Danube Broker**: `http://localhost:6650`
- **Danube Admin API**: `http://localhost:50051`
- **Danube Metrics**: `http://localhost:9040/metrics`
- **Qdrant HTTP**: `http://localhost:6333`
- **Qdrant gRPC**: `http://localhost:6334`
- **Connector Metrics**: `http://localhost:9090/metrics`
- **Docker Tools Profile**: `embeddings-generator`, `test-producer`, `vector-search`

### 2. Use the Dockerized Helper Tools

The example no longer requires local Python or `danube-cli` for the main workflow.

On first use, Docker Compose builds a small Python tools image from `Dockerfile.tools` and reuses it for embedding generation, message production, and vector search.

When overriding helper connection settings, use the Docker service names inside the Compose network:
- Danube: `http://danube-broker:6650`
- Qdrant: `http://qdrant:6333`

### 3. Generate and Send Test Data

**Step 1: Generate embeddings**

```bash
# Generate 10 sample embeddings (384 dimensions)
docker-compose --profile tools run --rm embeddings-generator --count 10

# Use a different model (768 dimensions)
docker-compose --profile tools run --rm embeddings-generator --count 20 --model all-mpnet-base-v2

# Without sentence-transformers (random vectors)
docker-compose --profile tools run --rm embeddings-generator --count 10
```

This creates `embeddings.jsonl` with sample messages and their vector embeddings.

**Step 2: Send to Danube**

```bash
# Send embeddings using the Dockerized Python producer
docker-compose --profile tools run --rm test-producer

# Custom configuration
DANUBE_URL=http://danube-broker:6650 \
TOPIC=/default/vectors \
docker-compose --profile tools run --rm test-producer
```

The workflow:
1. `docker-compose --profile tools run --rm embeddings-generator` runs the embedding generator inside Docker
2. `docker-compose --profile tools run --rm test-producer` runs a single long-lived Danube Python producer inside Docker
3. Messages are validated against the registered `embeddings-v1` schema
4. Connector automatically streams validated data to Qdrant

### 4. Search Vectors

Perform semantic search:

```bash
# Search for similar messages
docker-compose --profile tools run --rm vector-search --query "password reset help"

# Get more results
docker-compose --profile tools run --rm vector-search --query "billing question" --limit 10

# Show Danube metadata
docker-compose --profile tools run --rm vector-search --query "technical issue" --show-metadata

# List all collections
docker-compose --profile tools run --rm vector-search --list

# Show collection info
docker-compose --profile tools run --rm vector-search --info
```

## Configuration

### Using Configuration File 

**Single Topic with Schema Validation:** `connector.toml`
```toml
[[qdrant.routes]]
from = "/default/vectors"
subscription = "qdrant-sink-sub"
to = "vectors"
vector_dimension = 384
distance = "Cosine"
auto_create_collection = true
include_danube_metadata = true

# Schema validation (v0.2.0)
expected_schema_subject = "embeddings-v1"
```

**Multi-Topic:** `connector-multi-topic.toml`
```toml
# Route different topics to different collections
[[qdrant.routes]]
from = "/default/chat_embeddings"
subscription = "qdrant-chat-sub"
to = "chat_vectors"
vector_dimension = 384

[[qdrant.routes]]
from = "/default/wiki_embeddings"
subscription = "qdrant-wiki-sub"
to = "wiki_knowledge"
vector_dimension = 768

[[qdrant.routes]]
from = "/default/code_embeddings"
subscription = "qdrant-code-sub"
to = "code_search"
vector_dimension = 1536
```

To use:
```bash
docker-compose down

# Edit connector.toml or connector-multi-topic.toml with your settings

# Update docker-compose.yml to mount the config file
# Then restart
docker-compose up -d
```

Update `docker-compose.yml`:

```yaml
qdrant-sink:
  environment:
    - CONNECTOR_CONFIG_PATH=/etc/connector.toml
  volumes:
    - ./connector.toml:/etc/connector.toml:ro  # or connector-multi-topic.toml
```

## Monitoring

### Qdrant Dashboard

Open http://localhost:6333/dashboard

- View collections
- Browse points
- Inspect payloads
- Monitor cluster health

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
docker-compose logs -f qdrant-sink

# Danube logs
docker-compose logs -f danube-broker

# Qdrant logs
docker-compose logs -f qdrant
```

### Check Data Flow

```bash
# View collection info
docker-compose --profile tools run --rm vector-search --info
```

### Test with Different Vector Dimensions

The example uses 384-dimensional vectors (sentence-transformers). To test other dimensions:

```bash
# Stop services
docker-compose down

# Edit connector.toml
# Change: vector_dimension = 384
# To:     vector_dimension = 1536  # for OpenAI

# Restart
docker-compose up -d

# Generate and send data with matching dimension
docker-compose --profile tools run --rm embeddings-generator --model all-mpnet-base-v2 --count 10
docker-compose --profile tools run --rm test-producer
```

## Troubleshooting

### Connector Not Starting

```bash
# Check logs
docker-compose logs qdrant-sink

# Common issues:
# 1. Danube not ready - wait for healthcheck
# 2. Qdrant not ready - wait for healthcheck
# 3. Invalid dimension - check QDRANT_VECTOR_DIMENSION
```

### No Results in Search

```bash
# Verify data was sent
docker-compose --profile tools run --rm vector-search --info

# Check if Points Count > 0
# If zero, resend data:
docker-compose --profile tools run --rm embeddings-generator --count 10
docker-compose --profile tools run --rm test-producer

# Wait a few seconds for runtime processing
sleep 3

# Try search again
docker-compose --profile tools run --rm vector-search --query "test"
```

### Vector Dimension Mismatch

**Error:** `Vector dimension mismatch: expected 384, got 1536`

**Solution:** 
1. Update `vector_dimension` in `connector.toml`
2. Restart connector: `docker-compose restart qdrant-sink`
3. Or recreate collection with correct dimension

### Collection Not Found

```bash
# List collections
docker-compose --profile tools run --rm vector-search --list

# If collection doesn't exist, check:
# 1. auto_create_collection is enabled
# 2. Connector initialized successfully
docker-compose logs qdrant-sink | grep "collection"
```

## Performance Tips

### Tune Runtime Processing

For high throughput:

```toml
[processing]
batch_size = 200
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

## Next Steps

1. **Production Deployment**: See main [README](../README.md) for production setup
2. **Custom Embeddings**: Integrate your own embedding pipeline
3. **Multi-Topic Routing**: Use `connector-multi-topic.toml` to route multiple topics to different collections
4. **Qdrant Cloud**: Use managed Qdrant with `QDRANT_API_KEY`
5. **Advanced Monitoring**: Set up Prometheus + Grafana dashboards for metrics

## Resources

- [Qdrant Documentation](https://qdrant.tech/documentation/)
- [Sentence Transformers](https://www.sbert.net/)
- [RAG Tutorial](https://qdrant.tech/articles/what-is-rag-in-ai/)
- [Configuration Guide](../config/README.md)

## Support

For issues or questions:
- Check [connector logs](../README.md#troubleshooting)
- Review [configuration guide](../config/README.md)
- Open an issue on GitHub
