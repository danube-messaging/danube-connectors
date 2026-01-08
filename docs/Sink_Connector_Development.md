# Sink Connector Development Guide

A practical guide for building Danube sink connectors using `danube-connect-core` v0.4.0+.

## Table of Contents

- [What is a Sink Connector?](#what-is-a-sink-connector)
- [Core Concepts](#core-concepts)
- [Development Approach](#development-approach)
- [Key Components](#key-components)
- [Implementation Guide](#implementation-guide)
- [Best Practices](#best-practices)

---

## What is a Sink Connector?

A **sink connector** streams data from Danube topics into external systems (databases, search engines, storage, APIs).

**Your connector:**
- Consumes messages from topics
- Transforms data to target format
- Writes in batches for efficiency
- Handles errors gracefully

**The runtime provides:**
- Message consumption and buffering
- Schema validation and deserialization
- Lifecycle management
- Metrics and monitoring

**Reference examples:**
- `sink-surrealdb` - Multi-model database with time-series support
- `sink-qdrant` - Vector database with embedding generation

---

## Core Concepts

### High-Level Flow

```
┌─────────────────┐
│ Danube Topics   │
│  - /events      │
│  - /logs        │
│  - /metrics     │
└────────┬────────┘
         │ Streaming
         ▼
┌─────────────────┐
│  Sink Runtime   │ ← danube-connect-core
│  (buffering)    │
└────────┬────────┘
         │ SinkRecord batches
         ▼
┌─────────────────┐
│ Your Connector  │ ← You implement
│ (transform)     │
└────────┬────────┘
         │ Writes
         ▼
┌─────────────────┐
│  Target System  │
└─────────────────┘
```

### The SinkRecord

The runtime provides messages as `SinkRecord`:
- `data: Value` - Deserialized JSON payload (pre-validated)
- `topic: String` - Source topic
- `offset: u64` - Message offset
- `publish_time: u64` - Timestamp (microseconds)
- `attributes: HashMap<String, String>` - Optional metadata

**Key insight:** Schema validation and deserialization are handled by the runtime. You receive structured, validated data.

---

## Development Approach

### 1. Define Your Use Case

**Ask yourself:**
- What's the target system? (Database, search engine, API)
- How do topics map? (One-to-one, or flexible routing)
- What transformations? (Field mapping, enrichment, type conversion)
- Performance needs? (Throughput, latency, batch size)

**Examples:**
- **SurrealDB sink:** Multi-topic routing, optional time-series mode
- **Qdrant sink:** Embedding generation, vector dimensions

### 2. Design Configuration

**Three layers:**

**Core:** Connector name, Danube URL, metrics port

**Target System:** Connection URL, credentials, timeouts, global batch settings

**Topic Mappings (per topic):**
- `topic` - Danube topic
- `subscription` - Consumer group
- `subscription_type` - Shared/Exclusive/FailOver
- `target_entity` - Table/collection/index name
- `expected_schema_subject` - Optional schema validation
- `batch_size`, `flush_interval` - Optional overrides

**Configuration principle:**
- TOML file → Structure (topics, mappings, logic)
- Environment variables → Secrets and URLs only

### 3. Model Transformation

**Pipeline:** `SinkRecord → Transform → Write`

**Common patterns:**
1. **Direct** - Use `data` as-is
2. **Extract** - Pull specific fields from nested JSON
3. **Enrich** - Add metadata (topic, offset, timestamp)
4. **Convert** - Transform types (strings → dates)

**Key decisions:**
- Add Danube metadata? (Make configurable)
- Record IDs? (Use `record_id` attribute or auto-generate)
- Timestamps? (Use `publish_time` for time-series)

### 4. Handle Batching

**Two flush triggers:**
- Size: `batch_size` records buffered
- Time: `flush_interval_ms` elapsed

**Configuration:**
- Global defaults (batch_size: 100, flush_interval: 1000ms)
- Per-topic overrides for different workloads

**Performance guidance:**
- High throughput: batch_size 500-1000
- Low latency: batch_size 10-50
- Balanced: batch_size 100

---

## Key Components

### 1. Configuration (`config.rs`)

**Implement `ConnectorConfig`:**
- `load()` - Parse TOML + environment overrides
- `validate()` - Check required fields, valid values
- `consumer_configs()` - Generate per-topic consumer config

**Design tips:**
- Fail fast at startup (validate early)
- Provide sensible defaults
- Clear error messages

### 2. Connector (`connector.rs`)

**Implement `SinkConnector` trait:**
- `initialize()` - Connect to target system
- `process_batch()` - Transform and write records
- `flush()` - Force flush buffered data
- `shutdown()` - Graceful cleanup

**Lifecycle:**
```
initialize → process_batch (loop) → flush → shutdown
```

### 3. Transformation (`record.rs`)

**Transform logic:**
- Convert `SinkRecord` to target format
- Add metadata if configured
- Handle record IDs and timestamps
- Pure functions (easily testable)

### 4. Client Wrapper (optional)

**Target system client:**
- Connection management
- Batch write operations
- Error handling
- Retry logic

---

## Multi-Topic & Subscriptions

### Subscription Types

**Shared** (most common)
- Load balancing across consumers
- Horizontal scaling
- Use for: High-volume parallel processing

**Exclusive**
- One consumer only
- Ordered processing
- Use for: Order-dependent logic

**FailOver**
- Active-passive setup
- Automatic failover
- Use for: Ordered + high availability

### Multi-Topic Routing

Each topic mapping is independent:
- Different target entities
- Different batch sizes
- Different transformations
- Different schema validation

**Pattern:** Store topic → config map, lookup per batch

---

## Schema Validation

### How It Works 

**Validation flow:**
1. Schema registered in Danube Schema Registry
2. Topic created with schema subject
3. Producer validates before sending
4. Runtime deserializes and validates
5. Connector receives validated JSON

**Your connector doesn't validate.** The runtime does.

### Configuration

```toml
expected_schema_subject = "events-v1"  # Enable validation
```

**Use schema validation for:**
- ✅ Structured data with known schema
- ✅ Multiple producers (enforce consistency)
- ✅ Data quality requirements

**Skip for:**
- Free-form logs
- Binary data
- Schema evolution scenarios

---

## Implementation Guide

### Module Structure

```
src/
├── main.rs          # Entry point
├── config.rs        # ConnectorConfig implementation
├── connector.rs     # SinkConnector trait
├── record.rs        # Transformation logic
└── client.rs        # Target system wrapper (optional)
```

### Basic Flow

**1. Configuration**
```rust
// Load from TOML + environment
let config = YourConfig::load()?;
config.validate()?;
```

**2. Initialization**
```rust
// Connect to target system
let connector = YourConnector::new(config).await?;
connector.initialize().await?;
```

**3. Runtime**
```rust
// Provided by danube-connect-core
let runtime = SinkRuntime::new(connector).await?;
runtime.run().await?;
```

**4. Processing**
```rust
// Called by runtime for each batch
async fn process_batch(&mut self, records: Vec<SinkRecord>) {
    let transformed = records.into_iter()
        .map(|r| transform(r))
        .collect();
    
    self.client.write_batch(transformed).await?;
}
```

### Error Handling

**Configuration errors:** Fail fast at startup
**Transient errors:** Retry with backoff
**Data errors:** Log and skip
**Fatal errors:** Graceful shutdown

---

## Best Practices

### Configuration
- ✅ Validate at startup
- ✅ Use sensible defaults
- ✅ Secrets via environment only
- ✅ Document all fields

### Transformation
- ✅ Pure functions (testable)
- ✅ Handle null/missing fields
- ✅ Make metadata optional
- ✅ Document added fields

### Batching
- ✅ Size + time triggers
- ✅ Per-topic overrides
- ✅ Flush on shutdown
- ✅ Document performance

### Error Handling
- ✅ Distinguish error types
- ✅ Log with context
- ✅ Exponential backoff
- ✅ Clear error messages

### Documentation
- ✅ Working example (Docker Compose)
- ✅ Configuration guide
- ✅ Troubleshooting section
- ✅ Performance tuning

### Monitoring
- ✅ Prometheus metrics
- ✅ Health check endpoint
- ✅ Log important events
- ✅ Track errors/batches

---

## E2E Test (Docker Compose)
- Full stack: Danube + Schema Registry + Target + Connector
- Produce messages, verify in target system
- Check metrics

---

## Resources

**Reference Examples:**
- `sink-surrealdb` - Multi-model database, time-series
- `sink-qdrant` - Vector database, embeddings

**Core Framework:**
- `danube-connect-core` v0.4.0+

**Questions?** Open an issue in [danube-connectors](https://github.com/danrusei/danube-connectors)
