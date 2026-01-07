# Qdrant Sink Connector Configuration Guide

Complete reference for configuring the Qdrant Sink Connector.

## Configuration Files

- **[connector.toml](connector.toml)** - Single-topic reference configuration
- **[connector-multi-topic.toml](connector-multi-topic.toml)** - Multi-topic example with multiple collections

## Table of Contents

- [Configuration Structure](#configuration-structure)
- [Core Configuration](#core-configuration)
- [Qdrant Configuration](#qdrant-configuration)
- [Topic Mappings](#topic-mappings)
- [Environment Variables](#environment-variables)
- [Multi-Topic Routing](#multi-topic-routing)
- [Why Vector Dimension and Distance Matter](#why-vector-dimension-and-distance-matter)
- [Configuration Patterns](#configuration-patterns)

---

## Configuration Structure

The connector uses a unified configuration format combining core Danube Connect settings with Qdrant-specific options:

```toml
# Core Danube Connect Configuration
connector_name = "qdrant-sink-1"
danube_service_url = "http://localhost:6650"

[retry]
max_retries = 3
retry_backoff_ms = 1000

[processing]
batch_size = 100
metrics_port = 9090

# Qdrant-Specific Configuration
[qdrant]
url = "http://localhost:6334"
batch_size = 50

# Topic Mappings
[[qdrant.topic_mappings]]
topic = "/default/vectors"
collection_name = "my_vectors"
vector_dimension = 384
```

---

## Core Configuration

### Connector Identity

```toml
# Unique name for this connector instance
connector_name = "qdrant-sink-1"

# Danube broker gRPC endpoint
danube_service_url = "http://localhost:6650"
```

**Environment Variable Overrides:**
- `DANUBE_SERVICE_URL` - Override for different environments
- `CONNECTOR_NAME` - Override for different deployments

---

### Retry Configuration

```toml
[retry]
# Maximum retry attempts for failed operations
max_retries = 3

# Initial backoff duration (milliseconds)
retry_backoff_ms = 1000

# Maximum backoff duration (milliseconds)
max_backoff_ms = 30000
```

**Note:** Retry settings must be configured in the TOML file. They are NOT available as environment variables.

**Behavior:**
- Exponential backoff: `backoff = min(retry_backoff_ms * 2^attempt, max_backoff_ms)`
- Retries on transient failures (network errors, Qdrant unavailable)
- Permanent errors (invalid data) are not retried

---

### Processing Configuration

```toml
[processing]
# Batch size for processing records
batch_size = 100

# Batch timeout (milliseconds)
batch_timeout_ms = 1000

# Poll interval for new messages (milliseconds)
poll_interval_ms = 100

# Prometheus metrics port
metrics_port = 9090

# Log level: "error", "warn", "info", "debug", "trace"
log_level = "info"
```

**Note:** Processing settings must be configured in the TOML file. They are NOT available as environment variables.

---

## Qdrant Configuration

### Connection Settings

```toml
[qdrant]
# Qdrant gRPC endpoint (not HTTP!)
# Local:  http://localhost:6334
# Docker: http://qdrant:6334
# Cloud:  https://your-cluster.qdrant.io:6334
url = "http://localhost:6334"

# API key (required for Qdrant Cloud)
api_key = "your-api-key"

# Operation timeout (seconds)
timeout_secs = 30
```

**Environment Variable Overrides:**
- `QDRANT_URL` - Override Qdrant server URL (for different environments)
- `QDRANT_API_KEY` - API key for authentication (secret, should NOT be in TOML)

**Important:** Use the **gRPC port** (6334), not HTTP port (6333).

---

### Global Batch Settings

```toml
[qdrant]
# Default batch size (points per batch)
batch_size = 50

# Default batch timeout (milliseconds)
batch_timeout_ms = 1000
```

**Note:** Batch settings must be configured in the TOML file. They are NOT available as environment variables.

**Behavior:**
- Batches are flushed when reaching `batch_size` OR `batch_timeout_ms`
- Can be overridden per topic mapping
- See [Performance Tuning](#performance-tuning) for optimization

---

## Topic Mappings

Topic mappings define how Danube topics map to Qdrant collections.

### Single Topic Example

```toml
[[qdrant.topic_mappings]]
topic = "/default/vectors"
subscription = "qdrant-sink-sub"
subscription_type = "Exclusive"
collection_name = "vectors"
vector_dimension = 384
distance = "Cosine"
auto_create_collection = true
include_danube_metadata = true
```

### Configuration Fields

#### `topic` (required)
```toml
topic = "/default/vectors"
```
- **Format:** `/{namespace}/{topic_name}` (exactly 2 segments)
- **Valid:** `/default/vectors`, `/production/embeddings`, `/chat/messages`
- **Invalid:** `/vectors` (missing namespace), `/a/b/c` (too many segments)

#### `subscription` (required)
```toml
subscription = "qdrant-sink-sub"
```
- Consumer group subscription name
- Different subscriptions = independent consumers
- Same subscription = load-balanced consumers (if `subscription_type = "Shared"`)

#### `subscription_type` (required)
```toml
subscription_type = "Exclusive"  # or "Shared", "FailOver"
```
- **`Exclusive`** (default): Only one consumer per subscription
- **`Shared`**: Multiple consumers share the load (load balancing)
- **`FailOver`**: Active-passive failover

#### `collection_name` (required)
```toml
collection_name = "my_vectors"
```
- Target Qdrant collection name
- Created automatically if `auto_create_collection = true`
- Must exist if `auto_create_collection = false`

#### `vector_dimension` (required)
```toml
vector_dimension = 384
```
- Vector embedding dimension
- **Critical for:**
  1. **Early validation** - Rejects invalid vectors before sending to Qdrant
  2. **Auto-creation** - Creates collection with correct schema
- See [Why Vector Dimension Matters](#why-vector-dimension-and-distance-matter)

**Common dimensions:**
| Model | Dimension |
|-------|-----------|
| `all-MiniLM-L6-v2` | 384 |
| `all-mpnet-base-v2` | 768 |
| `text-embedding-ada-002` | 1536 |
| `text-embedding-3-large` | 3072 |
| Cohere `embed-english-v3.0` | 1024 |

#### `distance` (required)
```toml
distance = "Cosine"  # or "Euclid", "Dot", "Manhattan"
```
- Similarity metric for vector search
- **`Cosine`** - Best for normalized vectors (most common)
- **`Euclid`** - Euclidean distance for absolute positioning
- **`Dot`** - Dot product similarity
- **`Manhattan`** - L1 distance for sparse vectors
- See [Distance Metrics](#distance-metrics)

#### `auto_create_collection` (required)
```toml
auto_create_collection = true
```
- **`true`**: Creates collection automatically if it doesn't exist
- **`false`**: Collection must exist before connector starts (fails otherwise)

#### `include_danube_metadata` (required)
```toml
include_danube_metadata = true
```
- **`true`**: Adds Danube metadata to point payload
- **`false`**: Only includes user payload

**Metadata fields added:**
- `_danube_topic` - Source topic name
- `_danube_offset` - Message offset
- `_danube_timestamp` - Publish timestamp (microseconds)
- `_danube_producer` - Producer name
- `_danube_attr_*` - Custom message attributes

**Benefits:**
- Message traceability and debugging
- Temporal queries and filtering
- Multi-tenant isolation
- Source tracking

#### `batch_size` (optional)
```toml
batch_size = 100  # Override global default
```
- Per-topic batch size override
- If omitted, uses global `qdrant.batch_size`
- Useful for optimizing different data sources

#### `batch_timeout_ms` (optional)
```toml
batch_timeout_ms = 500  # Override global default
```
- Per-topic batch timeout override
- If omitted, uses global `qdrant.batch_timeout_ms`
- Lower for real-time updates, higher for throughput

---

## Environment Variables

### Configuration File (Required)

The connector requires a TOML configuration file:

```bash
export CONNECTOR_CONFIG_PATH=/path/to/connector.toml
```

**All structural configuration must be in the TOML file:**
- Topic mappings
- Collection settings
- Vector dimensions
- Batch sizes
- Retry settings
- Processing settings

### Environment Variable Overrides

Environment variables can override **only secrets and connection URLs**:

```bash
# Required: Path to TOML config file
export CONNECTOR_CONFIG_PATH=/etc/connector.toml

# Optional: Core Danube overrides (for different environments)
export DANUBE_SERVICE_URL=http://danube-broker:6650
export CONNECTOR_NAME=qdrant-sink-production

# Optional: Qdrant connection overrides
export QDRANT_URL=http://qdrant:6334

# Optional: Secrets (should NOT be in TOML file)
export QDRANT_API_KEY=your-api-key
```

**Supported Environment Variables:**

| Variable | Purpose | Example |
|----------|---------|----------|
| `CONNECTOR_CONFIG_PATH` | Path to TOML config (required) | `/etc/connector.toml` |
| `DANUBE_SERVICE_URL` | Override Danube broker URL | `http://prod-broker:6650` |
| `CONNECTOR_NAME` | Override connector name | `qdrant-sink-prod` |
| `QDRANT_URL` | Override Qdrant URL | `https://cluster.qdrant.io:6334` |
| `QDRANT_API_KEY` | Qdrant API key (secret) | `your-api-key` |

### Docker Deployment Example

```yaml
services:
  qdrant-sink:
    image: danube-sink-qdrant
    volumes:
      - ./connector.toml:/etc/connector.toml:ro
    environment:
      # Required
      - CONNECTOR_CONFIG_PATH=/etc/connector.toml
      
      # Optional: Core overrides
      - DANUBE_SERVICE_URL=http://danube-broker:6650
      - CONNECTOR_NAME=qdrant-sink-example
      
      # Optional: Qdrant overrides
      - QDRANT_URL=http://qdrant:6334
      - QDRANT_API_KEY=${QDRANT_API_KEY}
```

---

## Multi-Topic Routing

Route multiple Danube topics to different Qdrant collections with independent configurations.

### Example: RAG Pipeline with Multiple Sources

```toml
[qdrant]
url = "http://localhost:6334"
batch_size = 50  # Default for all topics

# Chat messages (real-time, small batches)
[[qdrant.topic_mappings]]
topic = "/default/chat_embeddings"
collection_name = "chat_vectors"
vector_dimension = 384  # MiniLM model
distance = "Cosine"
batch_size = 25  # Override: smaller batches for low latency
batch_timeout_ms = 500

# Wiki articles (comprehensive, balanced)
[[qdrant.topic_mappings]]
topic = "/default/wiki_embeddings"
collection_name = "wiki_knowledge"
vector_dimension = 768  # MPNet model
distance = "Cosine"
# Uses global defaults: batch_size=50, batch_timeout_ms=1000

# Code snippets (high volume, large batches)
[[qdrant.topic_mappings]]
topic = "/default/code_embeddings"
collection_name = "code_search"
vector_dimension = 1536  # OpenAI model
distance = "Cosine"
include_danube_metadata = false  # Save space
batch_size = 200  # Override: maximize throughput
batch_timeout_ms = 2000
```

### Benefits of Multiple Collections

#### ✅ Different Embedding Models → Different Dimensions
```
chat_vectors:    384 dims (MiniLM)
wiki_knowledge:  768 dims (MPNet)
code_search:    1536 dims (OpenAI)
visual_search:   512 dims (CLIP)
```

#### ✅ Different Search Characteristics → Different Metrics
```
text_data:   Cosine similarity
image_data:  Euclidean distance
sparse_data: Manhattan distance
```

#### ✅ Isolation & Organization
- Multi-tenant applications (tenant-specific collections)
- Development vs production data
- Different data sources (chat, docs, wiki, code)
- Separate access control

#### ✅ Performance Tuning
- **Real-time chat**: Small batches (25), low timeout (500ms)
- **Bulk imports**: Large batches (200), high timeout (2000ms)
- **Balanced workflows**: Default settings

#### ✅ Independent Batching
- Each collection maintains its own batch buffer
- No cross-collection blocking
- Collection A flushes independently from Collection B
- Optimal throughput per data source

### Architecture

```
Danube Topics          Consumers           Qdrant Collections
┌─────────────────┐   ┌──────────┐       ┌─────────────────┐
│ /chat/messages  │──▶│Consumer 1│──────▶│ chat_vectors    │
└─────────────────┘   └──────────┘       │ (384 dims)      │
                                          └─────────────────┘

┌─────────────────┐   ┌──────────┐       ┌─────────────────┐
│ /wiki/articles  │──▶│Consumer 2│──────▶│ wiki_knowledge  │
└─────────────────┘   └──────────┘       │ (768 dims)      │
                                          └─────────────────┘

┌─────────────────┐   ┌──────────┐       ┌─────────────────┐
│ /code/snippets  │──▶│Consumer 3│──────▶│ code_search     │
└─────────────────┘   └──────────┘       │ (1536 dims)     │
                                          └─────────────────┘
```

**Key Points:**
- One consumer per topic mapping
- Independent batch buffers per collection
- Per-collection statistics and monitoring
- Separate collection initialization

---

## Why Vector Dimension and Distance Matter

### 1. Early Validation (Fail Fast)

The connector validates vector dimensions **before** sending to Qdrant:

```rust
// Pseudo-code of validation logic
if message.vector.len() != expected_dimension {
    warn!("Vector dimension mismatch: expected {}, got {}", 
          expected, actual);
    // Skip invalid message, continue processing
    return Ok(());
}
```

**Benefits:**
- ✅ **Catches bad data immediately** - Doesn't wait for Qdrant to reject it
- ✅ **Clear error messages** - Shows topic, offset, and dimension mismatch
- ✅ **Resilient processing** - Skips invalid messages, doesn't crash pipeline
- ✅ **Performance** - Avoids network round-trip for invalid data

**Example Error:**
```
WARN Vector dimension mismatch: expected 384, got 1536
     topic=/default/vectors offset=12345 point_id=abc-123
     Skipping message
```

### 2. Auto-Collection Creation

If `auto_create_collection = true`, the connector creates collections with:

```rust
// Pseudo-code of collection creation
client.create_collection(
    name: "my_vectors",
    vectors_config: VectorParams {
        size: 384,              // From vector_dimension
        distance: Distance::Cosine  // From distance
    }
)
```

**Why this matters:**
- Qdrant collections require vector dimension **at creation time**
- Dimension **cannot be changed** after collection creation
- Distance metric must be set during collection creation
- Enables zero-configuration startup

**Without these configs:**
- ❌ No early validation → All errors happen at Qdrant (slower, worse messages)
- ❌ Cannot auto-create collections → Manual setup required
- ❌ Generic errors → Harder to debug dimension mismatches

---

## Configuration Patterns

### Pattern 1: Single Collection (Simple)

```toml
[qdrant]
url = "http://localhost:6334"

[[qdrant.topic_mappings]]
topic = "/default/vectors"
subscription = "qdrant-sink-sub"
collection_name = "vectors"
vector_dimension = 384
distance = "Cosine"
auto_create_collection = true
include_danube_metadata = true
```

**Use case:** Simple RAG pipeline with one embedding model

---

### Pattern 2: Multi-Source RAG

```toml
[[qdrant.topic_mappings]]
topic = "/chat/messages"
collection_name = "chat_vectors"
vector_dimension = 384

[[qdrant.topic_mappings]]
topic = "/docs/articles"
collection_name = "documentation"
vector_dimension = 768

[[qdrant.topic_mappings]]
topic = "/code/snippets"
collection_name = "code_search"
vector_dimension = 1536
```

**Use case:** Multiple data sources with different embedding models

---

### Pattern 3: Real-Time + Batch

```toml
# Real-time updates (chat)
[[qdrant.topic_mappings]]
topic = "/realtime/chat"
collection_name = "chat_live"
batch_size = 10
batch_timeout_ms = 100

# Batch imports (historical data)
[[qdrant.topic_mappings]]
topic = "/batch/historical"
collection_name = "chat_archive"
batch_size = 500
batch_timeout_ms = 5000
```

**Use case:** Mixed latency requirements
```

**Use case:** Separate environments

---

## Performance Tuning

### Batch Size Guidelines

| Scenario | Batch Size | Timeout | Rationale |
|----------|-----------|---------|-----------|
| Real-time chat | 10-25 | 100-500ms | Low latency |
| Balanced workflow | 50-100 | 1000ms | Default |
| Bulk import | 200-500 | 2000-5000ms | Max throughput |

### Network Optimization

```toml
# High-latency networks
timeout_secs = 60  # Increase timeout

# Local deployments
timeout_secs = 10  # Decrease timeout
```

### Memory Considerations

- Larger batches = more memory per collection
- Monitor: `batch_size × vector_dimension × sizeof(f32) × num_collections`
- Example: `100 × 1536 × 4 bytes × 3 collections = 1.8 MB`

---

## Validation

The connector validates configuration on startup:

**Checks:**
- ✅ `url` is not empty
- ✅ `topic_mappings` has at least one entry
- ✅ Each mapping has required fields
- ✅ `vector_dimension > 0`
- ✅ Topic format: `/{namespace}/{topic_name}`
- ✅ Collection names are valid

**Example error:**
```
ERROR Configuration validation failed: 
      topic_mappings cannot be empty, must configure at least one topic mapping
```

---

## Examples

See the reference configurations:
- **[connector.toml](connector.toml)** - Comprehensive single-topic example
- **[connector-multi-topic.toml](connector-multi-topic.toml)** - Advanced multi-topic setup

For a working deployment example, see:
- **[../../examples/sink-qdrant](../../examples/sink-qdrant)** - Docker Compose setup

---

## References

- [Main Connector README](../README.md) - Overview and quick start
- [Example Setup](../../examples/sink-qdrant/README.md) - Complete working example
- [Qdrant Documentation](https://qdrant.tech/documentation/)
- [Connector Development Guide](../../../info/connector-development-guide.md)
