#!/bin/bash
# Test script to publish sample MQTT messages via Docker

set -e

# Use Docker to run mosquitto_pub (no need to install on host)
NETWORK=${NETWORK:-source-mqtt_danube-mqtt-network}
MQTT_CONTAINER=${MQTT_CONTAINER:-mqtt-example-broker}

echo "Publishing test messages to MQTT broker via Docker"
echo "Network: ${NETWORK}"
echo "Broker: ${MQTT_CONTAINER}"
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
    timestamp=$(date +%s)
    
    # Temperature sensor zone1 (goes to /iot/sensors_zone1 via sensors/+/zone1 pattern)
    temp=$((RANDOM % 30 + 10))
    docker run --rm --network "${NETWORK}" eclipse-mosquitto:2 \
        mosquitto_pub -h "${MQTT_CONTAINER}" -t "sensors/temp/zone1" \
        -m "{\"temperature\":${temp},\"unit\":\"celsius\",\"timestamp\":${timestamp}}"
    
    # Temperature sensor zone2 (goes to /iot/temperature via sensors/temp/# pattern)
    temp2=$((RANDOM % 30 + 10))
    docker run --rm --network "${NETWORK}" eclipse-mosquitto:2 \
        mosquitto_pub -h "${MQTT_CONTAINER}" -t "sensors/temp/zone2" \
        -m "{\"temperature\":${temp2},\"unit\":\"celsius\",\"timestamp\":${timestamp}}"
    
    # Humidity sensor
    humidity=$((RANDOM % 40 + 40))
    docker run --rm --network "${NETWORK}" eclipse-mosquitto:2 \
        mosquitto_pub -h "${MQTT_CONTAINER}" -t "sensors/humidity/zone1" \
        -m "{\"humidity\":${humidity},\"unit\":\"percent\",\"timestamp\":${timestamp}}"
    
    # Pressure sensor
    pressure=$((RANDOM % 20 + 990))
    docker run --rm --network "${NETWORK}" eclipse-mosquitto:2 \
        mosquitto_pub -h "${MQTT_CONTAINER}" -t "sensors/pressure/factory1" \
        -m "{\"pressure\":${pressure},\"unit\":\"hPa\",\"timestamp\":${timestamp}}"
    
    # Device telemetry
    battery=$((RANDOM % 100))
    signal=$((RANDOM % 100))
    docker run --rm --network "${NETWORK}" eclipse-mosquitto:2 \
        mosquitto_pub -h "${MQTT_CONTAINER}" -t "devices/device001/telemetry" \
        -m "{\"battery\":${battery},\"signal\":${signal},\"timestamp\":${timestamp}}"
    
    echo "[$(date +%T)] Published batch #${count}: temp_z1=${temp}°C, temp_z2=${temp2}°C, humidity=${humidity}%, pressure=${pressure}hPa, battery=${battery}%"
    
    sleep 5
done
