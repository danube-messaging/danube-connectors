# MQTT Source Connector - Schema Validation Testing

This example demonstrates end-to-end **schema validation testing** of the MQTT Source Connector with Danube's built-in Schema Registry.

## ⚡ Key Features

- ✅ **3 Schemas Enabled** - Auto-register and validate on startup
- ✅ **Schema Registry Built-In** - No separate service needed
- ✅ **Automated Testing** - Test publisher sends compliant messages every 5s
- ✅ **Real Validation** - See valid/invalid messages handled differently
- ✅ **Production-Ready** - Same setup you'd use in production

## 🎯 What This Tests

- **Schema Registry Integration** - Auto-registration and validation
- **JSON Schema Validation** - Structured data validation (sensor-data.json, device-status.json)
- **String Schema** - Plain text validation
- **MQTT → Connector → Danube pipeline** - Complete data flow
- Topic mapping and wildcards
- Message transformation and metadata
- QoS handling
- Valid vs. Invalid message handling

### Data Flow (With Schema Validation)

```
MQTT Publisher (schema-compliant messages)
    ↓
Mosquitto MQTT Broker
    ↓ Subscribe
MQTT Source Connector
    ↓ Transform to SourceRecord
danube-connect-core Runtime
    ├─ Validates against JSON Schema
    ├─ ✅ Valid: Serialize & publish
    └─ ❌ Invalid: Reject & log error
    ↓
Danube Broker (with Schema Registry)
    ├─ /iot/sensors_zone1 (validated)
    ├─ /iot/device_telemetry (validated)
    ├─ /iot/debug (string, any text)
    └─ /iot/temperature (no schema)
    ↓ danube-cli consumer
Your terminal (validated data)
```

## 📁 Files in This Example

**Configuration:**
- **`connector.toml`** - Connector config with **3 schemas enabled** for validation testing
- **`docker-compose.yml`** - Complete stack (Danube, Mosquitto, connector, test publisher)
- **`../../example_shared/danube_broker_no_auth.yml`** - Shared Danube broker configuration used by all connector examples
- **`mosquitto.conf`** - MQTT broker configuration

**Schema Files (schemas/):**
- **`sensor-data.json`** - JSON Schema for IoT sensor telemetry (temperature, humidity, etc.)
- **`device-status.json`** - JSON Schema for device health/status information

**Testing:**
- **`test-publisher.sh`** - Publishes schema-compliant messages every 5 seconds
- **`SCHEMA_TESTING.md`** - Detailed schema validation testing guide
- **`README.md`** - This file (quick start guide)

## 🚀 Quick Start (5 Minutes)

> **💡 This example is pre-configured for schema validation testing!**  
> - 3 schemas are **enabled** and will auto-register
> - Test publisher sends **schema-compliant** messages
> - You'll see validation in action in the logs

### Prerequisites

- Docker & Docker Compose
- 8GB RAM recommended
- Ports available: 1883, 6650, 9001

### 1. Start Everything

```bash
cd source-mqtt/example
docker-compose up -d
```

This starts:
- **danube-broker** - Message broker with built-in Schema Registry
- **mosquitto** - MQTT broker
- **mqtt-connector** - Source connector with schema validation enabled
- **mqtt-test-publisher** - Publishes schema-compliant messages every 5 seconds

**Shared Danube broker config:**
- The example mounts `../../example_shared/danube_broker_no_auth.yml`
- Update that single file when Danube broker config changes for all connector examples

### 2. Watch the Logs

```bash
# Watch connector logs (with schema auto-registration)
docker logs -f mqtt-example-connector

# You should see:
# - Schema auto-registration messages
# - Schema validation in action
# - Messages being published to Danube

# Watch test publisher
docker logs -f mqtt-example-publisher

# Watch Danube broker
docker logs -f danube-broker

# Watch MQTT broker
docker logs -f mqtt-example-broker
```

Expected connector output:
```
[INFO] Configuration loaded and validated successfully
[INFO] Schemas configured: 3
[INFO] Routes: 4 configured
[INFO] Schema auto-registering: sensor-telemetry-v1
[INFO] Schema auto-registering: device-telemetry-v1  
[INFO] Schema auto-registering: debug-logs
[DEBUG] Received MQTT message: topic=sensors/temp/zone1, qos=0
[INFO] Polled 4 records
[DEBUG] Committed 4 offsets
```

## 📊 Configured Schemas

This example has **3 schemas configured** in `connector.toml`:

| Danube Topic | Schema Subject | Type | Schema File | Validation |
|--------------|---------------|------|-------------|------------|
| `/iot/sensors_zone1` | `sensor-telemetry-v1` | JSON Schema | `/etc/schemas/sensor-data.json` | ✅ Validates structure & types |
| `/iot/device_telemetry` | `device-telemetry-v1` | JSON Schema | `/etc/schemas/device-status.json` | ✅ Validates structure & types |
| `/iot/debug` | `debug-logs` | String | `""` (empty) | ✅ Any text accepted |
| `/iot/temperature` | - | None | N/A | ❌ No validation |

**Schema file patterns:**
- **JSON Schema**: Requires path to `.json` file: `schema_file = "/etc/schemas/sensor-data.json"`
- **String/Number/Bytes**: Use empty string: `schema_file = ""`

**How it works:**
1. **Connector starts** → Reads schema files from `/etc/schemas/`
2. **Auto-registers** → Schemas registered with Danube broker's schema registry
3. **Validates messages** → Runtime validates each message before publishing
4. **Rejects invalid** → Messages that don't match schema are logged and rejected

See [`SCHEMA_TESTING.md`](./SCHEMA_TESTING.md) for detailed validation testing guide.

### 3. Test Schema Validation

**Test with VALID messages (matches schema):**

```bash
# Valid sensor data (sensor-data.json schema)
docker exec mqtt-example-broker mosquitto_pub \
  -t sensors/temp/zone1 \
  -m '{
    "device_id": "sensor-001",
    "sensor_type": "temperature",
    "value": 23.5,
    "unit": "celsius",
    "timestamp": "2024-01-09T20:30:00Z"
  }'

# Valid device status (device-status.json schema)
docker exec mqtt-example-broker mosquitto_pub \
  -t devices/device001/telemetry \
  -m '{
    "device_id": "device001",
    "status": "online",
    "battery_level": 87,
    "uptime_seconds": 3600,
    "last_seen": "2024-01-09T20:30:00Z",
    "firmware_version": "1.2.3"
  }'

# Valid debug log (string schema - accepts any text)
docker exec mqtt-example-broker mosquitto_pub \
  -t debug/test \
  -m 'This is a debug message - any text is valid'
```

**Test with INVALID messages (schema validation fails):**

```bash
# INVALID: Missing required fields
docker exec mqtt-example-broker mosquitto_pub \
  -t sensors/temp/zone1 \
  -m '{"value": 23.5}'
# Expected: Schema validation error in connector logs

# INVALID: Wrong sensor_type (not in enum)
docker exec mqtt-example-broker mosquitto_pub \
  -t sensors/temp/zone1 \
  -m '{
    "device_id": "sensor-001",
    "sensor_type": "INVALID_TYPE",
    "value": 23.5,
    "timestamp": "2024-01-09T20:30:00Z"
  }'
# Expected: Schema validation error

# INVALID: Wrong data type
docker exec mqtt-example-broker mosquitto_pub \
  -t devices/device001/telemetry \
  -m '{
    "device_id": "device001",
    "status": "online",
    "battery_level": "not-a-number",
    "last_seen": "2024-01-09T20:30:00Z"
  }'
# Expected: Schema validation error
```

### 4. Publish to Topics Without Schemas

```bash
# Temperature topic (no schema configured) - accepts any format
docker exec mqtt-example-broker mosquitto_pub \
  -t sensors/temp/other \
  -m '{"temperature": 25.5, "any": "format", "works": true}'
```

### 5. Subscribe to MQTT Topics

```bash
# See all sensor messages
docker exec mqtt-example-broker mosquitto_sub -t 'sensors/#' -v

# See specific device
docker exec mqtt-example-broker mosquitto_sub -t 'devices/+/telemetry' -v
```

### 6. Consume from Danube

To verify messages are reaching Danube, consume them using **danube-cli**.

**Download danube-cli:**
- GitHub Releases: https://github.com/danube-messaging/danube/releases
- Documentation: https://danube-docs.dev-state.com/danube_clis/danube_cli/consumer/

**danube-cli** (for sending/receiving messages to Danube):

Download the latest release for your system from [Danube Releases](https://github.com/danube-messaging/danube/releases):

```bash
# Linux
wget https://github.com/danube-messaging/danube/releases/download/v0.6.1/danube-cli-linux
chmod +x danube-cli-linux

# macOS (Apple Silicon)
wget https://github.com/danube-messaging/danube/releases/download/v0.6.1/danube-cli-macos
chmod +x danube-cli-macos

# Windows
# Download danube-cli-windows.exe from the releases page
```

**Note:** If you invoke `test-publisher.sh`, it publishes to MQTT via Docker and does not require a local `danube-cli` installation.

**Available platforms:**
- Linux: `danube-cli-linux`
- macOS (Apple Silicon): `danube-cli-macos`
- Windows: `danube-cli-windows.exe`

Or use the Docker image:
```bash
docker pull ghcr.io/danube-messaging/danube-cli:latest
```


**Routes (MQTT → Danube):**

The connector routes MQTT messages to Danube topics based on `connector.toml`:

| MQTT Topic Pattern | Danube Topic | Example MQTT Topics |
|-------------------|--------------|---------------------|
| `sensors/+/zone1` | `/iot/sensors_zone1` | `sensors/temp/zone1`, `sensors/humidity/zone1` |
| `devices/+/telemetry` | `/iot/device_telemetry` | `devices/device001/telemetry`, `devices/sensor42/telemetry` |
| `sensors/temp/#` | `/iot/temperature` | `sensors/temp/zone2`, `sensors/temp/floor2` |
| `debug/#` | `/iot/debug` | `debug/app`, `debug/system/error` |

> **⚠️ Important:** The connector routes to the **first matching pattern**. For example:
> - `sensors/temp/zone1` matches `sensors/+/zone1` first → goes to `/iot/sensors_zone1`
> - `sensors/temp/zone2` doesn't match `sensors/+/zone1`, matches `sensors/temp/#` → goes to `/iot/temperature`

**Consume messages from specific Danube topics:**

```bash
# Consume zone1 sensor data (temp, humidity, pressure from zone1)
danube-cli consume \
  --service-addr http://localhost:6650 \
  --topic /iot/sensors_zone1 \
  --subscription zone1-sub

# Consume all device telemetry
danube-cli consume \
  --service-addr http://localhost:6650 \
  --topic /iot/device_telemetry \
  --subscription telemetry-sub

# Consume temperature sensors only
danube-cli consume \
  --service-addr http://localhost:6650 \
  --topic /iot/temperature \
  --subscription temp-sub

# With exclusive subscription (only one consumer receives messages)
danube-cli consume \
  -s http://localhost:6650 \
  -t /iot/temperature \
  -m test-exclusive \
  --sub-type exclusive

# You should see MQTT messages appearing in real-time:
# Message received: {"temperature":22,"unit":"celsius","timestamp":1734539835}
```

### 7. Stop Everything

```bash
docker-compose down
```

## 📡 Test Publisher (Automated)

The `mqtt-test-publisher` container **automatically** publishes schema-compliant messages every 5 seconds:

**Messages published:**

1. **sensors/temp/zone1** → `/iot/sensors_zone1` (✅ JSON Schema validated)
   ```json
   {
     "device_id": "sensor-temp-001",
     "sensor_type": "temperature",
     "value": 22,
     "unit": "celsius",
     "timestamp": "2024-01-09T20:30:00Z",
     "battery_level": 75
   }
   ```

2. **sensors/humidity/zone1** → `/iot/sensors_zone1` (✅ JSON Schema validated)
   ```json
   {
     "device_id": "sensor-hum-001",
     "sensor_type": "humidity",
     "value": 65,
     "unit": "percent",
     "timestamp": "2024-01-09T20:30:00Z",
     "signal_strength": -45
   }
   ```

3. **devices/device001/telemetry** → `/iot/device_telemetry` (✅ JSON Schema validated)
   ```json
   {
     "device_id": "device001",
     "status": "online",
     "battery_level": 87,
     "uptime_seconds": 3600,
     "last_seen": "2024-01-09T20:30:00Z",
     "firmware_version": "1.2.3"
   }
   ```

4. **debug/app** → `/iot/debug` (✅ String schema, any text)
   ```
   "Debug: System running normally, battery: 75%"
   ```

All messages are **schema-compliant** and will pass validation!

Watch the publisher logs:
```bash
docker logs -f mqtt-test-publisher

# Output:
# [20:30:15] Published 4 messages at 20:30:15 - Schema-validated
# [20:30:20] Published 4 messages at 20:30:20 - Schema-validated
```

## 🧪 Manual Load Testing

For custom testing, use `test-publisher.sh`:

```bash
chmod +x test-publisher.sh
./test-publisher.sh

# Press Ctrl+C to stop
```

## 🔍 Troubleshooting

```bash
# Verify MQTT broker is running
docker exec mqtt-example-broker mosquitto_sub -t '#' -v

# Check connector logs
docker logs mqtt-example-connector

# Check Danube broker metrics endpoint
curl http://localhost:9040/metrics

# Restart if needed
docker-compose restart mqtt-example-connector
```

## 📚 Related Documentation

**This Example:**
- **[SCHEMA_TESTING.md](./SCHEMA_TESTING.md)** - Detailed schema validation testing guide
- [test-publisher.sh](./test-publisher.sh) - Manual testing script

**Connector Documentation:**
- [MQTT Source Connector README](../README.md)
- [Source Connector Development Guide](https://danube-docs.dev-state.com/integrations/source_connector_development/)

**Danube Documentation:**
- [Schema Registry Guide](https://danube-docs.dev-state.com/schema_registry/)
- [danube-cli Documentation](https://danube-docs.dev-state.com/danube_clis/danube_cli/consumer/)
- [danube-cli Releases](https://github.com/danube-messaging/danube/releases)

**Configuration Examples:**
- [connector.toml](./connector.toml) - Schema validation enabled (this example)
- [connector-with-schemas.toml](../config/connector-with-schemas.toml) - Advanced multi-schema example
