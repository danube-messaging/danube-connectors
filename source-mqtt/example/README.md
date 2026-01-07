# MQTT Source Connector - Integration Testing

This example demonstrates end-to-end integration testing of the MQTT Source Connector, showing how MQTT messages flow into Danube.

## 🎯 What This Tests

- MQTT broker → Connector → Danube broker pipeline
- Topic mapping and wildcards
- Message transformation and metadata
- QoS handling

### Data Flow

```
MQTT Publisher (test messages)
    ↓
Mosquitto MQTT Broker
    ↓ Subscribe
MQTT Source Connector
    ↓ Publish  
Danube Broker (/iot/sensors topic)
    ↓ danube-cli consumer
Your terminal
```

## 📁 Files in This Example

- **`docker-compose.yml`** - Orchestrates the complete test stack (etcd, Danube, Mosquitto, connector)
- **`connector.toml`** - MQTT connector configuration with topic mappings and settings
- **`danube_broker.yml`** - Danube broker configuration
- **`mosquitto.conf`** - MQTT broker configuration (listeners, logging)
- **`test-publisher.sh`** - Automated test script to publish sample MQTT messages
- **`README.md`** - This file (integration testing guide)

## 🚀 Quick Start (5 Minutes)

### Prerequisites

- Docker & Docker Compose
- 8GB RAM recommended
- Ports available: 1883, 2379, 6650, 9001

### 1. Start Everything

```bash
cd examples/source-mqtt
docker-compose up -d
```

This starts:
- etcd (Danube's metadata store)
- Danube broker
- Mosquitto MQTT broker
- MQTT source connector
- Test message publisher

### 2. Watch the Logs

```bash
# Watch connector logs
docker logs -f mqtt-example-connector

# Watch test publisher
docker logs -f mqtt-example-publisher

# Watch Danube broker
docker logs -f danube-broker

# Watch MQTT broker
docker logs -f mqtt-example-broker
```

You should see messages flowing:
```
[DEBUG] Received MQTT message: topic=sensors/temp/zone1, qos=0, size=58
[INFO] Polled 3 records
[DEBUG] Committed 3 offsets
```

### 3. Publish Your Own Messages

```bash
# Temperature reading
docker exec mqtt-example-broker mosquitto_pub \
  -t sensors/temp/zone2 \
  -m '{"temperature": 25.5, "unit": "celsius"}'

# Device telemetry
docker exec mqtt-example-broker mosquitto_pub \
  -t devices/mydevice/telemetry \
  -m '{"battery": 87, "signal": 95}'

# Pressure sensor
docker exec mqtt-example-broker mosquitto_pub \
  -t sensors/pressure/factory1 \
  -m '{"pressure": 101.3, "unit": "kPa"}'
```

### 4. Subscribe to MQTT Topics

```bash
# See all sensor messages
docker exec mqtt-example-broker mosquitto_sub -t 'sensors/#' -v

# See specific device
docker exec mqtt-example-broker mosquitto_sub -t 'devices/+/telemetry' -v
```

### 5. Consume from Danube

To verify messages are reaching Danube, consume them using **danube-cli**.

**Download danube-cli:**
- GitHub Releases: https://github.com/danube-messaging/danube/releases
- Documentation: https://danube-docs.dev-state.com/danube_clis/danube_cli/consumer/

**danube-cli** (for sending/receiving messages to Danube):

Download the latest release for your system from [Danube Releases](https://github.com/danube-messaging/danube/releases):

```bash
# Linux
wget https://github.com/danube-messaging/danube/releases/download/v0.5.2/danube-cli-linux
chmod +x danube-cli-linux

# macOS (Apple Silicon)
wget https://github.com/danube-messaging/danube/releases/download/v0.5.2/danube-cli-macos
chmod +x danube-cli-macos

# Windows
# Download danube-cli-windows.exe from the releases page
```

**Note:** The `test_producer.sh` script automatically detects `danube-cli-linux`, `danube-cli-macos`, or `danube-cli` in the current directory, so you don't need to install it system-wide.

**Available platforms:**
- Linux: `danube-cli-linux`
- macOS (Apple Silicon): `danube-cli-macos`
- Windows: `danube-cli-windows.exe`

Or use the Docker image:
```bash
docker pull ghcr.io/danube-messaging/danube-cli:v0.5.2
```


**Topic Mappings (MQTT → Danube):**

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

### 6. Stop Everything

```bash
docker-compose down
```

## Automated Load Testing

Use `test-publisher.sh` to continuously publish sample messages:

```bash
# Make script executable
chmod +x test-publisher.sh

# Start automated publishing (sends temp, humidity, pressure, telemetry every 5s)
./test-publisher.sh

# Output:
# [10:30:15] Published batch #1: temp=22°C, humidity=65%, pressure=1013hPa, battery=78%
# [10:30:20] Published batch #2: temp=25°C, humidity=58%, pressure=1009hPa, battery=82%
# ...

# In another terminals, consume from Danube
danube-cli consume   --service-addr http://localhost:6650   --topic /iot/temperature   --subscription telemetry-sub

danube-cli consume   --service-addr http://localhost:6650   --topic /iot/sensors_zone1   --subscription telemetry-sub

danube-cli consume   --service-addr http://localhost:6650   --topic /iot/device_telemetry   --subscription telemetry-sub

# Press Ctrl+C to stop the publisher
```

This script simulates:
- Temperature sensors (`sensors/temp/zone1`)
- Humidity sensors (`sensors/humidity/zone1`)
- Pressure sensors (`sensors/pressure/factory1`)
- Device telemetry (`devices/device001/telemetry`)

## 🔍 Troubleshooting

```bash
# Verify MQTT broker is running
docker exec mqtt-example-broker mosquitto_sub -t '#' -v

# Check connector logs
docker logs mqtt-example-connector

# Check Danube broker
curl http://localhost:6650/health

# Restart if needed
docker-compose restart mqtt-example-connector
```

## 📚 Related Documentation

- [MQTT Connector Development Guide](../../connectors/source-mqtt/DEVELOPMENT.md)
- [MQTT Connector README](../../connectors/source-mqtt/README.md)
- [danube-cli Documentation](https://danube-docs.dev-state.com/danube_clis/danube_cli/consumer/)
- [danube-cli Releases](https://github.com/danube-messaging/danube/releases)
