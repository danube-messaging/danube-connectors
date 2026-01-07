# MQTT Source Connector Configuration Guide

Complete configuration reference for the MQTT Source Connector.

## Table of Contents

- [Configuration File Structure](#configuration-file-structure)
- [Core Settings](#core-settings)
- [MQTT Connection Settings](#mqtt-connection-settings)
- [Topic Mappings](#topic-mappings)
- [Environment Variable Overrides](#environment-variable-overrides)
- [Configuration Examples](#configuration-examples)
- [Best Practices](#best-practices)

## Configuration File Structure

The connector uses a TOML configuration file with three main sections:

```toml
# Core connector settings
connector_name = "mqtt-source"
danube_service_url = "http://localhost:6650"

# MQTT broker connection
[mqtt]
broker_host = "localhost"
broker_port = 1883
# ... more MQTT settings

# Topic routing rules
[[mqtt.topic_mappings]]
mqtt_topic = "sensors/#"
danube_topic = "/iot/sensors"
# ... more mappings
```

## Core Settings

### Required Fields

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `connector_name` | string | Unique identifier for this connector instance | `"mqtt-production-1"` |
| `danube_service_url` | string | Danube broker URL | `"http://danube-broker:6650"` |

### Optional Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `metrics_port` | integer | `9090` | Prometheus metrics port |
| `log_level` | string | `"info"` | Log level: `error`, `warn`, `info`, `debug`, `trace` |

## MQTT Connection Settings

### Required MQTT Fields

```toml
[mqtt]
broker_host = "mosquitto"      # MQTT broker hostname
broker_port = 1883              # MQTT broker port
client_id = "danube-connector-1" # Unique MQTT client ID
```

### Optional MQTT Fields

```toml
[mqtt]
# Authentication
username = "mqtt_user"
password = "mqtt_pass"

# TLS/SSL
use_tls = false
# ca_cert_path = "/path/to/ca.crt"
# client_cert_path = "/path/to/client.crt"
# client_key_path = "/path/to/client.key"

# Connection parameters
keep_alive_secs = 60
connection_timeout_secs = 30
max_packet_size = 10485760  # 10 MB

# Session settings
clean_session = true
include_metadata = true
```

### MQTT Connection Parameters

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `broker_host` | string | **required** | MQTT broker hostname or IP |
| `broker_port` | integer | `1883` | MQTT broker port (1883 for TCP, 8883 for TLS) |
| `client_id` | string | **required** | Unique MQTT client identifier |
| `username` | string | none | MQTT authentication username |
| `password` | string | none | MQTT authentication password |
| `use_tls` | boolean | `false` | Enable TLS/SSL encryption |
| `keep_alive_secs` | integer | `60` | MQTT keep-alive interval |
| `connection_timeout_secs` | integer | `30` | Connection timeout |
| `max_packet_size` | integer | `10485760` | Maximum MQTT packet size (bytes) |
| `clean_session` | boolean | `true` | Start with clean session |
| `include_metadata` | boolean | `true` | Include MQTT metadata as message attributes |

## Topic Mappings

Topic mappings define how MQTT topics are routed to Danube topics.

### Basic Mapping

```toml
[[mqtt.topic_mappings]]
mqtt_topic = "sensors/temperature"
danube_topic = "/iot/temperature"
qos = "AtLeastOnce"
partitions = 4
```

### Wildcard Patterns

MQTT supports two wildcard characters:

- **`+`** (single-level wildcard) - Matches one topic level
- **`#`** (multi-level wildcard) - Matches zero or more topic levels

```toml
# Match sensors/temp/zone1, sensors/temp/zone2, etc.
[[mqtt.topic_mappings]]
mqtt_topic = "sensors/temp/+"
danube_topic = "/iot/temperature"
qos = "AtLeastOnce"
partitions = 4

# Match all sensor topics
[[mqtt.topic_mappings]]
mqtt_topic = "sensors/#"
danube_topic = "/iot/all_sensors"
qos = "AtMostOnce"
partitions = 8
```

### Topic Mapping Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `mqtt_topic` | string | ✅ | MQTT topic pattern (wildcards supported) |
| `danube_topic` | string | ✅ | Target Danube topic (`/{namespace}/{topic}`) |
| `qos` | string | ✅ | MQTT QoS level (see below) |
| `partitions` | integer | ✅ | Number of Danube topic partitions (0 = non-partitioned) |
| `reliable_dispatch` | boolean | optional | Override QoS-based reliable delivery |

### QoS Levels

| QoS Value | MQTT Semantics | Reliable Dispatch Default |
|-----------|----------------|---------------------------|
| `"AtMostOnce"` | QoS 0 - Fire and forget | `false` |
| `"AtLeastOnce"` | QoS 1 - At least once delivery | `true` |
| `"ExactlyOnce"` | QoS 2 - Exactly once delivery | `true` |

### Danube Topic Format

Danube topics must follow the format: `/{namespace}/{topic_name}`

**Valid:**
- `/iot/sensors`
- `/default/telemetry`
- `/production/temperature`

**Invalid:**
- `/iot/sensors/zone1` (too many segments)
- `iot/sensors` (missing leading slash)
- `/iot` (only one segment)

## Environment Variable Overrides

Environment variables can override **only secrets and connection URLs**:

### Supported Environment Variables

| Variable | Overrides | Use Case |
|----------|-----------|----------|
| `CONNECTOR_CONFIG_PATH` | - | **Required**: Path to TOML config file |
| `DANUBE_SERVICE_URL` | `danube_service_url` | Different environments (dev/staging/prod) |
| `CONNECTOR_NAME` | `connector_name` | Multiple connector instances |
| `MQTT_BROKER_HOST` | `mqtt.broker_host` | Different MQTT brokers |
| `MQTT_BROKER_PORT` | `mqtt.broker_port` | Different ports |
| `MQTT_CLIENT_ID` | `mqtt.client_id` | Unique client IDs |
| `MQTT_USERNAME` | `mqtt.username` | **Secret** - Authentication |
| `MQTT_PASSWORD` | `mqtt.password` | **Secret** - Authentication |
| `MQTT_USE_TLS` | `mqtt.use_tls` | Enable/disable TLS |

### NOT Supported via Environment Variables

The following **must** be in the TOML file:
- Topic mappings (`[[mqtt.topic_mappings]]`)
- QoS levels
- Partition configuration
- Retry/processing settings
- Batch sizes and timeouts

### Example Usage

```bash
# Set config file path (required)
export CONNECTOR_CONFIG_PATH=/etc/connector.toml

# Override for production
export DANUBE_SERVICE_URL=http://prod-broker:6650
export CONNECTOR_NAME=mqtt-prod-1
export MQTT_BROKER_HOST=prod-mqtt.internal
export MQTT_USERNAME=prod_user
export MQTT_PASSWORD=${VAULT_PASSWORD}

# Run connector
./danube-source-mqtt
```

## Configuration Examples

### Example 1: Simple IoT Sensor Data

```toml
connector_name = "iot-sensors"
danube_service_url = "http://localhost:6650"

[mqtt]
broker_host = "mosquitto"
broker_port = 1883
client_id = "danube-iot-1"

[[mqtt.topic_mappings]]
mqtt_topic = "sensors/temperature"
danube_topic = "/iot/temperature"
qos = "AtLeastOnce"
partitions = 4
```

### Example 2: Multi-Zone with Wildcards

```toml
connector_name = "multi-zone-sensors"
danube_service_url = "http://danube-broker:6650"

[mqtt]
broker_host = "mosquitto"
broker_port = 1883
client_id = "danube-multi-zone"
username = "iot_user"
# password via env: MQTT_PASSWORD

# High-volume sensor data from zone1
[[mqtt.topic_mappings]]
mqtt_topic = "sensors/+/zone1"
danube_topic = "/iot/zone1"
qos = "AtMostOnce"
partitions = 8

# Critical alerts from all zones
[[mqtt.topic_mappings]]
mqtt_topic = "alerts/#"
danube_topic = "/iot/alerts"
qos = "ExactlyOnce"
partitions = 2
reliable_dispatch = true
```

### Example 3: Production with TLS

```toml
connector_name = "production-mqtt"
danube_service_url = "https://prod-danube:6650"

[mqtt]
broker_host = "prod-mqtt.company.com"
broker_port = 8883
client_id = "danube-prod-1"
use_tls = true
# username/password via env variables
keep_alive_secs = 120
connection_timeout_secs = 60

[[mqtt.topic_mappings]]
mqtt_topic = "devices/+/telemetry"
danube_topic = "/production/telemetry"
qos = "AtLeastOnce"
partitions = 16
```

### Example 4: Fleet Management

```toml
connector_name = "fleet-management"
danube_service_url = "http://danube-broker:6650"

[mqtt]
broker_host = "fleet-mqtt"
broker_port = 1883
client_id = "danube-fleet"

# Vehicle GPS data
[[mqtt.topic_mappings]]
mqtt_topic = "vehicles/+/gps"
danube_topic = "/fleet/gps"
qos = "AtLeastOnce"
partitions = 32

# Vehicle diagnostics
[[mqtt.topic_mappings]]
mqtt_topic = "vehicles/+/diagnostics"
danube_topic = "/fleet/diagnostics"
qos = "AtLeastOnce"
partitions = 16

# Emergency alerts
[[mqtt.topic_mappings]]
mqtt_topic = "vehicles/+/emergency"
danube_topic = "/fleet/emergency"
qos = "ExactlyOnce"
partitions = 4
reliable_dispatch = true
```

## Best Practices

### Topic Mapping Order

Topic mappings are evaluated in order. Place more specific patterns before general ones:

```toml
# ✅ Good: Specific first
[[mqtt.topic_mappings]]
mqtt_topic = "sensors/critical/#"
danube_topic = "/iot/critical"

[[mqtt.topic_mappings]]
mqtt_topic = "sensors/#"
danube_topic = "/iot/all"

# ❌ Bad: General pattern catches everything
[[mqtt.topic_mappings]]
mqtt_topic = "sensors/#"
danube_topic = "/iot/all"

[[mqtt.topic_mappings]]
mqtt_topic = "sensors/critical/#"  # Never reached!
danube_topic = "/iot/critical"
```

### QoS Selection

Choose QoS based on your requirements:

- **QoS 0 (AtMostOnce)**: High-volume, loss-tolerant data (sensor readings, metrics)
- **QoS 1 (AtLeastOnce)**: Important data that can tolerate duplicates (events, logs)
- **QoS 2 (ExactlyOnce)**: Critical data requiring exactly-once semantics (financial transactions, commands)

### Partition Configuration

- **High-volume topics**: More partitions (8-32) for parallel processing
- **Low-volume topics**: Fewer partitions (1-4) to reduce overhead
- **Ordered data**: Use 1 partition or partition by key

### Security

- **Never** put passwords in TOML files
- Use environment variables for secrets: `MQTT_PASSWORD`, `MQTT_USERNAME`
- Enable TLS for production: `use_tls = true`
- Use strong, unique `client_id` values

### Performance Tuning

```toml
[mqtt]
# Increase for high-throughput scenarios
max_packet_size = 20971520  # 20 MB

# Reduce for low-latency requirements
keep_alive_secs = 30

# Increase for unstable networks
connection_timeout_secs = 60
```

## Running with Configuration

### With Cargo

```bash
export CONNECTOR_CONFIG_PATH=config/connector.toml
cargo run --release
```

### With Docker

```bash
docker run -d \
  --name mqtt-source \
  -v $(pwd)/connector.toml:/etc/connector.toml:ro \
  -e CONNECTOR_CONFIG_PATH=/etc/connector.toml \
  -e DANUBE_SERVICE_URL=http://danube-broker:6650 \
  -e CONNECTOR_NAME=mqtt-source \
  -e MQTT_USERNAME=user \
  -e MQTT_PASSWORD=password \
  danube/source-mqtt:latest
```

## Troubleshooting

### Connection Issues

**Problem:** Connector can't connect to MQTT broker

**Solutions:**
- Verify `broker_host` and `broker_port`
- Check network connectivity: `ping <broker_host>`
- Verify authentication credentials
- Check TLS settings if using encrypted connection

### Topic Not Matching

**Problem:** Messages not being routed to Danube

**Solutions:**
- Verify MQTT topic pattern matches actual topics
- Check topic mapping order (specific before general)
- Enable debug logging: `RUST_LOG=debug`
- Test MQTT subscription independently

### QoS Issues

**Problem:** Messages being lost or duplicated

**Solutions:**
- Verify QoS level matches requirements
- Check `reliable_dispatch` setting
- Monitor Danube broker health
- Review network stability

## Additional Resources

- [MQTT 3.1.1 Specification](https://docs.oasis-open.org/mqtt/mqtt/v3.1.1/mqtt-v3.1.1.html)
- [Danube Documentation](https://github.com/danrusei/danube)
- [rumqttc Client Documentation](https://docs.rs/rumqttc/)
- [Complete Example](../../../examples/source-mqtt/)
