#!/bin/bash
# Test script to publish schema-validated MQTT messages via Docker
# Sends messages matching sensor-data.json and device-status.json schemas

set -e

# Use Docker to run mosquitto_pub (no need to install on host)
NETWORK=${NETWORK:-example_danube-mqtt-network}
MQTT_CONTAINER=${MQTT_CONTAINER:-mqtt-example-broker}

echo "Publishing schema-validated test messages to MQTT broker via Docker"
echo "Network: ${NETWORK}"
echo "Broker: ${MQTT_CONTAINER}"
echo "Schemas: sensor-data.json, device-status.json, string"
echo "Press Ctrl+C to stop"
echo ""

# Check if broker is running
if ! docker ps --format '{{.Names}}' | grep -q "^${MQTT_CONTAINER}$"; then
    echo "Error: MQTT broker container '${MQTT_CONTAINER}' is not running"
    echo "Start the example first: docker-compose up -d"
    exit 1
fi

count=0
while true; do
    count=$((count + 1))
    timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    
    # Generate random values
    temp=$((RANDOM % 30 + 10))
    humidity=$((RANDOM % 40 + 40))
    battery=$((RANDOM % 100))
    signal=$((40 + RANDOM % 51))
    signal=-$signal
    uptime=$RANDOM
    
    # Temperature sensor zone1 (schema-validated: sensor-data.json)
    docker run --rm --network "${NETWORK}" eclipse-mosquitto:2 \
        mosquitto_pub -h "${MQTT_CONTAINER}" -t "sensors/temp/zone1" \
        -m "{\"device_id\":\"sensor-temp-001\",\"sensor_type\":\"temperature\",\"value\":${temp},\"unit\":\"celsius\",\"timestamp\":\"${timestamp}\",\"battery_level\":${battery}}"
    
    # Humidity sensor zone1 (schema-validated: sensor-data.json)
    docker run --rm --network "${NETWORK}" eclipse-mosquitto:2 \
        mosquitto_pub -h "${MQTT_CONTAINER}" -t "sensors/humidity/zone1" \
        -m "{\"device_id\":\"sensor-hum-001\",\"sensor_type\":\"humidity\",\"value\":${humidity},\"unit\":\"percent\",\"timestamp\":\"${timestamp}\",\"signal_strength\":${signal}}"
    
    # Device telemetry (schema-validated: device-status.json)
    docker run --rm --network "${NETWORK}" eclipse-mosquitto:2 \
        mosquitto_pub -h "${MQTT_CONTAINER}" -t "devices/device001/telemetry" \
        -m "{\"device_id\":\"device001\",\"status\":\"online\",\"battery_level\":${battery},\"uptime_seconds\":${uptime},\"last_seen\":\"${timestamp}\",\"firmware_version\":\"1.2.3\"}"
    
    # Debug log (string schema - any text)
    docker run --rm --network "${NETWORK}" eclipse-mosquitto:2 \
        mosquitto_pub -h "${MQTT_CONTAINER}" -t "debug/app" \
        -m "Debug: System running normally, battery: ${battery}%"
    
    echo "[$(date +%T)] Published batch #${count}: temp=${temp}°C, humidity=${humidity}%, battery=${battery}%, signal=${signal}dBm - Schema-validated"
    
    sleep 2
done
