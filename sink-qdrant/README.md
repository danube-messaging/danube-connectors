# Qdrant Sink Connector

High-performance sink connector for streaming vector embeddings from Danube to Qdrant. Perfect for building RAG (Retrieval Augmented Generation) pipelines and AI-powered search applications.

## ✨ Features

- 🚀 **Native Rust** - Built with qdrant-client for maximum performance
- 🔒 **Schema Validation** - Runtime validation with Danube Schema Registry
- 🎯 **Multi-Topic Routing** - Route multiple Danube topics to different Qdrant collections
- 📦 **Intelligent Batching** - Configurable batching per collection for optimal throughput
- 🔄 **Auto-Collection Management** - Automatically creates collections with proper configuration
- 📊 **Metadata Enrichment** - Optionally includes Danube metadata for traceability
- 🎨 **Flexible Configuration** - Per-topic vector dimensions, distance metrics, and batch settings
- ⚡ **High Throughput** - Async processing with connection pooling and independent collection batching
- 🛡️ **Robust Error Handling** - Early validation, retry logic, and graceful degradation

## 🚀 Quick Start

### Running with Docker

```bash
docker run -d \
  --name qdrant-sink \
  -v $(pwd)/connector.toml:/etc/connector.toml:ro \
  -e CONNECTOR_CONFIG_PATH=/etc/connector.toml \
  -e DANUBE_SERVICE_URL=http://danube-broker:6650 \
  -e CONNECTOR_NAME=qdrant-sink \
  -e QDRANT_URL=http://qdrant:6334 \
  -e QDRANT_API_KEY=${QDRANT_API_KEY} \
  danube/sink-qdrant:latest
```

**Note:** All structural configuration (topics, collections, dimensions, batching) must be in `connector.toml`. See [Configuration](#configuration) section below.

### Complete Example

For a complete working setup with Docker Compose, embedding generation, and test data:

👉 **See [example/](example/README.md)**

The example includes:
- Docker Compose setup (Danube + ETCD + Qdrant)
- Pre-configured connector.toml
- Embedding generation scripts
- Test producers using danube-cli
- Query and search examples

## 📝 Message Format

The connector expects JSON messages with vector embeddings:

### **Standard Format**
```json
{
  "id": "optional-point-id",
  "vector": [0.1, 0.2, 0.3, ...],
  "payload": {
    "text": "Original content",
    "metadata": "any JSON value"
  }
}
```

### **Minimal Format**
```json
{
  "vector": [0.1, 0.2, 0.3, ...]
}
```

### **Schema Validation**

Register a JSON Schema for message validation:

```bash
# Create schema
cat > vector-schema.json << 'EOF'
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "VectorEmbedding",
  "type": "object",
  "properties": {
    "id": {"type": "string"},
    "vector": {
      "type": "array",
      "items": {"type": "number"},
      "minItems": 1
    },
    "payload": {"type": "object"}
  },
  "required": ["vector"]
}
EOF

# Register with Danube
danube-admin-cli schemas register chat-embeddings-v1 \
  --schema-type json_schema \
  --file vector-schema.json

# Produce validated messages
danube-cli produce \
  --topic /default/vectors \
  --schema-subject chat-embeddings-v1 \
  --message '{"vector": [0.1, 0.2, 0.3], "payload": {"text": "Hello"}}'
```

**Benefits:**
- ✅ Messages validated before reaching connector
- ✅ Type-safe deserialization by runtime
- ✅ Schema evolution support
- ✅ Clear error messages at producer

## ⚙️ Configuration

### 📖 Complete Configuration Guide

See **[config/README.md](config/README.md)** for comprehensive configuration documentation including:
- Core and connector-specific configuration options
- Single-topic vs multi-topic setup
- Environment variable reference
- Configuration patterns and best practices
- Performance tuning guidelines

### 📄 Reference Configurations

- **[config/connector.toml](config/connector.toml)** - Fully documented single-topic configuration
- **[config/connector-multi-topic.toml](config/connector-multi-topic.toml)** - Advanced multi-topic example

### 🎯 Common Use Cases

**Single Collection (Simple RAG):**
```toml
[[qdrant.topic_mappings]]
topic = "/default/vectors"
subscription = "qdrant-sink-sub"
collection_name = "vectors"
vector_dimension = 384
distance = "Cosine"
auto_create_collection = true

# Optional: Enable schema validation
expected_schema_subject = "embeddings-v1"
```

**Multiple Collections with Schema Validation (Advanced RAG):**
```toml
# Chat embeddings - validated with schema
[[qdrant.topic_mappings]]
topic = "/chat/embeddings"
collection_name = "chat_vectors"
vector_dimension = 384
distance = "Cosine"
expected_schema_subject = "chat-embeddings-v1"  # Schema validation

# Documentation - different dimension & schema
[[qdrant.topic_mappings]]
topic = "/docs/embeddings"
collection_name = "documentation"
vector_dimension = 768
distance = "Cosine"
expected_schema_subject = "doc-embeddings-v1"
```

**Without Schema Validation (Backward Compatible):**
```toml
[[qdrant.topic_mappings]]
topic = "/legacy/vectors"
collection_name = "legacy_data"
vector_dimension = 1536
distance = "Cosine"
# No expected_schema_subject - accepts any valid JSON
```

See [config/README.md](config/README.md) for more patterns.

## 🛠️ Development

### Building

### Local Build

```bash
cargo build --release
```

### Docker Build

```bash
docker build -t danube-sink-qdrant:latest .
```

### Running

#### With Cargo

```bash
# Run with configuration file
export CONNECTOR_CONFIG_PATH=config/connector.toml
cargo run --release

# With environment overrides
export CONNECTOR_CONFIG_PATH=config/connector.toml
export DANUBE_SERVICE_URL=http://localhost:6650
export QDRANT_URL=http://localhost:6334
export QDRANT_API_KEY=your-api-key
cargo run --release
```

#### With Docker

```bash
docker run -d \
  --name qdrant-sink \
  -v $(pwd)/connector.toml:/etc/connector.toml:ro \
  -e CONNECTOR_CONFIG_PATH=/etc/connector.toml \
  -e DANUBE_SERVICE_URL=http://danube-broker:6650 \
  -e CONNECTOR_NAME=qdrant-sink \
  -e QDRANT_URL=http://qdrant:6334 \
  -e QDRANT_API_KEY=${QDRANT_API_KEY} \
  danube-sink-qdrant:latest
```

**Note:** All structural configuration (topics, collections, dimensions, batching) must be in `connector.toml`.

### Monitoring

#### Prometheus Metrics

The connector exposes metrics on port 9090:

```bash
curl http://localhost:9090/metrics
```

**Key metrics:**
- `danube_connector_messages_processed_total` - Total messages processed
- `danube_connector_messages_failed_total` - Failed messages
- `danube_connector_batch_size` - Current batch sizes
- `danube_connector_processing_duration_seconds` - Processing latency

#### Logs

Configure log level with `RUST_LOG`:

```bash
# Info level (default)
RUST_LOG=info cargo run

# Debug level for troubleshooting
RUST_LOG=debug,danube_sink_qdrant=trace cargo run
```

**Log levels:**  `error`, `warn`, `info`, `debug`, `trace`

### Troubleshooting

#### Vector Dimension Mismatch

**Error:** `Vector dimension mismatch: expected 384, got 1536`

**Solution:**  Ensure `QDRANT_VECTOR_DIMENSION` matches your embedding model:
- `all-MiniLM-L6-v2`: 384
- `all-mpnet-base-v2`: 768
- `text-embedding-ada-002`: 1536

#### Collection Not Found

**Error:** `Collection 'my_vectors' does not exist`

**Solutions:**
1. Enable auto-creation in config: `auto_create_collection = true`
2. Create collection manually (see [config/README.md](config/README.md))

#### Connection Issues

**Error:** `Failed to connect to Qdrant`

**Checklist:**
- ✅ Qdrant is running: `docker ps | grep qdrant`
- ✅ Using gRPC port (6334), not HTTP port (6333)
- ✅ URL format: `http://localhost:6334` (not `https://` for local)

#### Invalid Messages

**Error:** `Failed to parse message as JSON`

**Solution:** Ensure messages have `vector` field:
```json
{"vector": [0.1, 0.2, ...], "payload": {...}}
```

#### Schema Validation Errors (v0.2.0)

**Error:** `Schema validation failed: missing required field 'vector'`

**Solution:** Register schema and ensure messages match:
```bash
# 1. Check registered schema
danube-admin-cli schemas get chat-embeddings-v1

# 2. Produce with correct structure
danube-cli produce \
  --topic /default/vectors \
  --schema-subject chat-embeddings-v1 \
  --message '{"vector": [0.1, 0.2], "payload": {}}'
```

**Error:** `Schema 'embeddings-v1' not found`

**Solution:** Register schema before starting connector:
```bash
danube-admin-cli schemas register embeddings-v1 \
  --schema-type json_schema \
  --file schema.json
```

For more troubleshooting, see the [configuration guide](config/README.md).

## 📚 Documentation

### Complete Working Example

See **[example/](example/)** for a complete setup with:
- Docker Compose (Danube + ETCD + Qdrant)
- Schema registration examples
- Embedding generation scripts
- Test producers using danube-cli with schemas
- Single and multi-topic configurations
- Monitoring and dashboards

### Configuration Examples

- **[config/connector.toml](config/connector.toml)** - Single-topic reference
- **[config/connector-multi-topic.toml](config/connector-multi-topic.toml)** - Multi-topic advanced setup

### Multi-Topic Routing

The connector supports routing multiple Danube topics to different Qdrant collections with independent configurations.

**Benefits:**
- ✅ Different embedding models → Different vector dimensions
- ✅ Different search metrics → Optimized for each data type
- ✅ Isolation & organization → Separate namespaces for tenants/environments
- ✅ Performance tuning → Per-collection batch settings
- ✅ Independent batching → No cross-collection blocking

### References

- **[Configuration Guide](config/README.md)** - Complete configuration reference
- **[Working Example](example/)** - Docker Compose setup
- [Qdrant Documentation](https://qdrant.tech/documentation/)
- [qdrant-client Rust Crate](https://docs.rs/qdrant-client/)
- [RAG with Qdrant](https://qdrant.tech/articles/what-is-rag-in-ai/)

## License

Apache License 2.0 - See [LICENSE](../../LICENSE)
