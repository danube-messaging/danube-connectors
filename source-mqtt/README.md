# MQTT Source Connector

A high-performance MQTT source connector that bridges MQTT-based IoT devices with Danube Messaging. Subscribes to MQTT topics and publishes messages to Danube topics with full metadata preservation.

## ✨ Features

- 🚀 **MQTT 3.1.1 Protocol** - Full support via rumqttc client
- 🎯 **Wildcard Subscriptions** - `+` (single-level) and `#` (multi-level) patterns
- 📊 **All QoS Levels** - QoS 0 (fire-and-forget), QoS 1 (at-least-once), QoS 2 (exactly-once)
- 🔄 **Flexible Topic Routing** - Multiple MQTT patterns → Danube topics with per-topic configuration
- 🔒 **Schema Registry Support** - JSON Schema validation with auto-registration (v0.2.0+)
- 📝 **Metadata Preservation** - MQTT attributes (topic, QoS, retain, dup) as message attributes
- 🛡️ **Reliable Dispatch** - Automatic QoS-based reliable delivery to Danube

**Use Cases:** Industrial IoT, smart devices, edge computing, sensor networks, fleet management

## 🚀 Quick Start

### Running with Docker

```bash
docker run -d \
  --name mqtt-source \
  -v $(pwd)/connector.toml:/etc/connector.toml:ro \
  -e CONNECTOR_CONFIG_PATH=/etc/connector.toml \
  -e DANUBE_SERVICE_URL=http://danube-broker:6650 \
  -e CONNECTOR_NAME=mqtt-source \
  -e MQTT_BROKER_HOST=mosquitto \
  -e MQTT_USERNAME=user \
  -e MQTT_PASSWORD=password \
  danube/source-mqtt:latest
```

**Note:** All structural configuration (topic mappings, QoS, partitions) must be in `connector.toml`. See [Configuration](#configuration) section below.

### Complete Example with Schema Validation

For a complete working setup with Docker Compose, MQTT broker, schema validation, and step-by-step guide:

👉 **See [example/README.md](example/README.md)**

The example includes:
- **Schema Registry Integration** - 3 schemas configured and auto-registered
- Docker Compose setup (Danube + ETCD + Mosquitto MQTT)
- Schema-validated test publishers (JSON Schema + String schema)
- Pre-configured connector.toml with schemas
- Consuming messages with danube-cli

## ⚙️ Configuration

### 📖 Complete Configuration Guide

See **[config/README.md](config/README.md)** for comprehensive configuration documentation including:
- Core and MQTT connection settings
- Topic mapping patterns and wildcards
- QoS levels and reliable dispatch
- Environment variable reference
- Configuration examples and best practices

### 📄 Quick Reference

#### Minimal Configuration Example

```toml
# connector.toml
danube_service_url = "http://danube-broker:6650"
connector_name = "mqtt-iot-source"

[mqtt]
broker_host = "mosquitto"
broker_port = 1883
client_id = "danube-connector-1"

# Route sensors/temp/zone1 → /iot/sensors_zone1
[[mqtt.topic_mappings]]
mqtt_topic = "sensors/+/zone1"  # MQTT pattern (wildcards supported)
danube_topic = "/iot/sensors_zone1"  # Format: /{namespace}/{topic}
qos = "AtLeastOnce"  # QoS 1
partitions = 4       # Danube topic partitions

# Route devices telemetry → /iot/device_telemetry
[[mqtt.topic_mappings]]
mqtt_topic = "devices/+/telemetry"
danube_topic = "/iot/device_telemetry"
qos = "AtLeastOnce"
partitions = 2

# Optional: Schema validation (v0.2.0+)
[[schemas]]
topic = "/iot/sensors_zone1"
subject = "sensor-telemetry-v1"
schema_type = "json_schema"
schema_file = "schemas/sensor-data.json"  # Path to JSON Schema file
auto_register = true
version_strategy = "latest"
```

#### Environment Variable Overrides

Environment variables can override **only secrets and connection URLs**:

| Variable | Description | Example |
|----------|-------------|---------|
| `CONNECTOR_CONFIG_PATH` | Path to TOML config (required) | `/etc/connector.toml` |
| `DANUBE_SERVICE_URL` | Override Danube broker URL | `http://prod-broker:6650` |
| `CONNECTOR_NAME` | Override connector name | `mqtt-production` |
| `MQTT_BROKER_HOST` | Override MQTT broker host | `prod-mqtt.internal` |
| `MQTT_BROKER_PORT` | Override MQTT broker port | `1883` |
| `MQTT_CLIENT_ID` | Override MQTT client ID | `mqtt-prod-1` |
| `MQTT_USERNAME` | MQTT username (secret) | `prod_user` |
| `MQTT_PASSWORD` | MQTT password (secret) | `${VAULT_PASSWORD}` |
| `MQTT_USE_TLS` | Enable TLS | `true` |

**Note:** For schema validation examples, see:
- **[example/connector.toml](example/connector.toml)** - Basic example with 3 schemas
- **[config/connector-with-schemas.toml](config/connector-with-schemas.toml)** - Advanced multi-schema example

See [config/README.md](config/README.md) for complete configuration documentation.

## 🛠️ Development

### Building

```bash
# Build release binary
cargo build --release

# Run tests
cargo test

# Build Docker image
docker build -t danube/source-mqtt:latest .
```

### Running from Source

```bash
# Run with configuration file
export CONNECTOR_CONFIG_PATH=config/connector.toml
cargo run --release

# With environment overrides
export CONNECTOR_CONFIG_PATH=config/connector.toml
export DANUBE_SERVICE_URL=http://localhost:6650
export MQTT_BROKER_HOST=mosquitto
export MQTT_USERNAME=user
export MQTT_PASSWORD=password
cargo run --release
```

### Message Attributes

Each message published to Danube includes MQTT metadata as attributes:

```json
{
  "mqtt.topic": "sensors/temp/zone1",
  "mqtt.qos": "0",
  "mqtt.retain": "false",
  "mqtt.dup": "false",
  "source": "mqtt"
}
```

These attributes are queryable in Danube consumers and useful for filtering, routing, and debugging.

## 📚 Documentation

### Complete Working Example with Schema Validation

See **[example/README.md](example/README.md)** for a complete setup with:
- **Schema Registry Integration** - JSON Schema + String validation
- Docker Compose (Danube + ETCD + Mosquitto MQTT)
- Schema-validated test publishers
- Step-by-step testing guide
- Valid vs. Invalid message testing
- Consuming messages with danube-cli

### Configuration Examples

- **[example/connector.toml](example/connector.toml)** - Working example with 3 schemas
- **[config/connector-with-schemas.toml](config/connector-with-schemas.toml)** - Advanced multi-schema example
- **[config/connector.toml](config/connector.toml)** - Fully documented reference configuration
- **[config/README.md](config/README.md)** - Complete configuration guide

### How It Works

```
MQTT Broker → Connector subscribes → Validates (optional) → Publishes to Danube
                    ↓                        ↓                      ↓
           Topic pattern matching    Schema validation      Message + Metadata
            (wildcards supported)    (JSON Schema, etc.)   (MQTT attributes)
```

**Message Flow:**
1. Connector subscribes to configured MQTT topic patterns (e.g., `sensors/#`, `devices/+/telemetry`)
2. Receives messages from MQTT broker via rumqttc client
3. Matches MQTT topic against configured patterns (first match wins)
4. Validates against schema if configured (rejects invalid messages)
5. Routes to corresponding Danube topic with metadata
6. Commits offset after successful Danube publish

**Schema Validation (v0.2.0+):**
- Configure schemas per Danube topic for data quality
- Supports JSON Schema, String, Bytes, and Number types
- Auto-registers schemas with Danube's built-in schema registry
- Invalid messages are rejected and logged
- See [example/README.md](example/README.md) for schema validation testing

### References

- **[Schema Validation Example](example/README.md)** - Working schema registry integration
- **[Configuration Guide](config/README.md)** - Complete configuration reference
- **[Schema Testing Guide](example/SCHEMA_TESTING.md)** - Detailed schema validation testing
- [Source Connector Development](https://danube-docs.dev-state.com/integrations/source_connector_development/)
- [MQTT 3.1.1 Specification](https://docs.oasis-open.org/mqtt/mqtt/v3.1.1/mqtt-v3.1.1.html)
- [rumqttc Client](https://github.com/bytebeamio/rumqtt)
- [Danube Messaging](https://github.com/danrusei/danube)

## License

Apache License 2.0 - See [LICENSE](../../LICENSE)
