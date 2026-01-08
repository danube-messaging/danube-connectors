# Qdrant Sink Connector Configuration

Simple guide for configuring the Qdrant Sink Connector.

## Configuration Files

- **[connector.toml](connector.toml)** - Single-topic example
- **[connector-multi-topic.toml](connector-multi-topic.toml)** - Multi-topic example

---

## Quick Start

### Minimal Configuration

```toml
# Core settings
connector_name = "qdrant-sink-1"
danube_service_url = "http://localhost:6650"

# Qdrant connection
[qdrant]
url = "http://localhost:6334"

# Topic mapping
[[qdrant.topic_mappings]]
topic = "/default/vectors"
subscription = "qdrant-sink-sub"
collection_name = "vectors"
vector_dimension = 384
distance = "Cosine"
auto_create_collection = true
include_danube_metadata = true
```

---

## Core Settings

### Connection

```toml
connector_name = "qdrant-sink-1"          # Connector instance name
danube_service_url = "http://localhost:6650"  # Danube broker URL
```

### Retry (Optional)

```toml
[retry]
max_retries = 3
retry_backoff_ms = 1000
max_backoff_ms = 30000
```

### Processing (Optional)

```toml
[processing]
batch_size = 100
batch_timeout_ms = 1000
poll_interval_ms = 100
metrics_port = 9090
log_level = "info"
```

---

## Qdrant Configuration

### Connection

```toml
[qdrant]
url = "http://localhost:6334"  # gRPC port (not 6333)
# api_key = "your-key"          # For Qdrant Cloud
timeout_secs = 30
```

**Environment overrides:** `QDRANT_URL`, `QDRANT_API_KEY`

### Batching

```toml
[qdrant]
batch_size = 50             # Points per batch
batch_timeout_ms = 1000     # Max wait time
```

---

## Topic Mappings

### Required Fields

```toml
[[qdrant.topic_mappings]]
topic = "/default/vectors"              # Danube topic (/{namespace}/{name})
subscription = "qdrant-sink-sub"       # Consumer group name
subscription_type = "Exclusive"        # "Exclusive", "Shared", or "FailOver"
collection_name = "vectors"            # Qdrant collection
vector_dimension = 384                 # Vector size (must match embeddings)
distance = "Cosine"                    # "Cosine", "Euclid", "Dot", "Manhattan"
auto_create_collection = true         # Create collection if missing
include_danube_metadata = true        # Add _danube_* fields to payload
```

### Common Vector Dimensions

| Model | Dimension |
|-------|-----------|
| sentence-transformers/all-MiniLM-L6-v2 | 384 |
| sentence-transformers/all-mpnet-base-v2 | 768 |
| OpenAI text-embedding-ada-002 | 1536 |
| OpenAI text-embedding-3-large | 3072 |
| Cohere embed-english-v3.0 | 1024 |

### Schema Validation (Optional)

```toml
[[qdrant.topic_mappings]]
# ... other fields ...
expected_schema_subject = "embeddings-v1"  # Registered schema name
```

Enables automatic validation and deserialization by the runtime.

### Per-Topic Overrides (Optional)

```toml
[[qdrant.topic_mappings]]
# ... other fields ...
batch_size = 100           # Override global batch_size
batch_timeout_ms = 500     # Override global timeout
```

---

## Environment Variables

### Required

```bash
CONNECTOR_CONFIG_PATH=/path/to/connector.toml  # Path to config file
```

### Optional Overrides

```bash
DANUBE_SERVICE_URL=http://danube-broker:6650  # Broker URL
CONNECTOR_NAME=qdrant-sink-prod                # Connector name
QDRANT_URL=http://qdrant:6334                  # Qdrant URL
QDRANT_API_KEY=your-api-key                    # Qdrant API key (secret)
```

---

## Multi-Topic Example

```toml
# Multiple topics → different collections
[[qdrant.topic_mappings]]
topic = "/default/chat_embeddings"
collection_name = "chat_vectors"
vector_dimension = 384
distance = "Cosine"
batch_size = 25                    # Smaller batches
batch_timeout_ms = 500             # Low latency

[[qdrant.topic_mappings]]
topic = "/default/wiki_embeddings"
collection_name = "wiki_knowledge"
vector_dimension = 768
distance = "Cosine"
# Uses global defaults

[[qdrant.topic_mappings]]
topic = "/default/code_embeddings"
collection_name = "code_search"
vector_dimension = 1536
distance = "Cosine"
batch_size = 200                   # Larger batches
batch_timeout_ms = 2000            # High throughput
```

---

## Message Format

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

---

## Performance Tips

| Use Case | Batch Size | Timeout |
|----------|-----------|---------|
| Real-time | 10-25 | 100-500ms |
| Balanced | 50-100 | 1000ms |
| Bulk import | 200-500 | 2-5s |

---

## Examples

- **[connector.toml](connector.toml)** - Single-topic reference
- **[connector-multi-topic.toml](connector-multi-topic.toml)** - Multi-topic setup
- **[../example/](../example/)** - Docker Compose example
