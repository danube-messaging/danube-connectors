# 🌊 Danube Connectors

<div align="center">

**Connectors for Danube Messaging**

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![danube-connect-core](https://img.shields.io/crates/v/danube-connect-core.svg)](https://crates.io/crates/danube-connect-core)

[Core SDK](https://github.com/danube-messaging/danube-connect-core) | [Connector Development Guide](https://github.com/danube-messaging/danube-connect-core/blob/main/info/connector-development-guide.md) | [Danube Messaging](https://github.com/danube-messaging/danube)

</div>

## Overview

This repository contains connectors for [Danube Messaging](https://github.com/danube-messaging/danube), built with the [`danube-connect-core`](https://github.com/danube-messaging/danube-connect-core) SDK. Each connector enables seamless integration with external systems through Docker-deployable binaries.

## Features

- 🔌 **Plug-and-Play Connectors** - Ready-to-use integrations for popular systems
- 🦀 **Pure Rust** - Memory-safe, high-performance connector framework
- 🔄 **Bidirectional** - Support for both source and sink connectors
- 📦 **Modular** - Clean separation between framework and connector implementations
- 🚀 **Cloud Native** - Docker-first with Kubernetes support
- 📊 **Observable** - Built-in metrics, tracing, and health checks
- ⚡ **High Performance** - Batching, connection pooling, and parallel processing

## Architecture

```text
External Systems ↔ Connectors ↔ danube-connect-core ↔ danube-client ↔ Danube Broker
```

Each connector:
- Built with [`danube-connect-core`](https://crates.io/crates/danube-connect-core) SDK
- Runs as a standalone process (Docker container recommended)
- Communicates with Danube brokers via gRPC
- Independently versioned and deployable

## Quick Start

### Running a Connector

Run any connector using Docker:

```bash
docker run -e DANUBE_SERVICE_URL=http://localhost:6650 \
           -e CONNECTOR_NAME=my-qdrant-sink \
           -v ./config.toml:/etc/connector.toml \
           ghcr.io/danube-messaging/danube-sink-qdrant:latest
```

### Building Your Own Connector

* See the [Build your own Source Connector](https://danube-docs.dev-state.com/integrations/source_connector_development/) for detailed instructions.
* See the [Build your own Sink Connector](https://danube-docs.dev-state.com/integrations/sink_connector_development/) for detailed instructions.

## Available Connectors

### Sink Connectors (Danube → External)

| Connector | Status | Description | Documentation |
|-----------|--------|-------------|---------------|
| [Qdrant](./sink-qdrant/) | ✅ Available | Vector embeddings for RAG/AI | [README](./sink-qdrant/README.md) |
| [SurrealDB](./sink-surrealdb/) | ✅ Available | Multi-model database (documents, time-series) | [README](./sink-surrealdb/README.md) |
| [Delta Lake](./sink-deltalake/) | ✅ Available | ACID data lake ingestion (S3/Azure/GCS) | [README](./sink-deltalake/README.md) |
| LanceDB | 🚧 Planned | Serverless vector DB for RAG pipelines | - |
| ClickHouse | 🚧 Planned | Real-time analytics and feature stores | - |
| GreptimeDB | 🚧 Planned | Unified observability (metrics/logs/traces) | - |

### Source Connectors (External → Danube)

| Connector | Status | Description | Documentation |
|-----------|--------|-------------|---------------|
| [MQTT](./source-mqtt/) | ✅ Available | IoT device integration (MQTT 3.1.1) | [README](./source-mqtt/README.md) |
| [HTTP/Webhook](./source-webhook/) | ✅ Available | Universal webhook ingestion from SaaS platforms | [README](./source-webhook/README.md) |
| OpenTelemetry | 🚧 Planned | Lightweight OTLP receiver (traces/metrics/logs) | - |
| PostgreSQL CDC | 🚧 Planned | Change Data Capture from Postgres | - |

## Releasing Connectors

Connectors are released independently with their own versions and tags. To release a connector:

1. **Update the version** in the connector's `Cargo.toml`
2. **Commit your changes** to the repository
3. **Create and push a tag** with the connector name prefix:

```bash
# Example: Releasing source-webhook v0.2.0
git tag source-webhook/v0.2.0
git push origin source-webhook/v0.2.0
```

The GitHub Actions workflow will automatically build multi-platform binaries and Docker images, then create a release with all artifacts.

## Contributing

We welcome contributions! Here's how you can help:

- **New Connectors**: Implement connectors for popular systems using [`danube-connect-core`](https://github.com/danube-messaging/danube-connect-core)
- **Improve Existing Connectors**: Add features, optimize performance, or fix bugs
- **Documentation**: Improve connector READMEs and examples
- **Testing**: Add test coverage and integration tests
- **Bug Reports**: Open issues with detailed information

### Adding a New Connector

* See the [Build your own Source Connector](https://danube-docs.dev-state.com/integrations/source_connector_development/) for detailed instructions.
* See the [Build your own Sink Connector](https://danube-docs.dev-state.com/integrations/sink_connector_development/) for detailed instructions.

## License

Apache License 2.0 - See [LICENSE](LICENSE) for details.

## Community & Resources

- **Core SDK**: [danube-connect-core](https://github.com/danube-messaging/danube-connect-core)
- **GitHub Issues**: [Report bugs or request features](https://github.com/danube-messaging/danube-connectors/issues)
- **Danube Docs**: [Official Documentation](https://danube-docs.dev-state.com)
- **Main Project**: [Danube Messaging](https://github.com/danube-messaging/danube)

---

**Note:** Each connector in this repository is independently versioned and can be updated on its own schedule. Connectors use the [`danube-connect-core`](https://crates.io/crates/danube-connect-core) SDK and can pin different versions based on their requirements.
