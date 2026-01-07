#!/bin/bash
# Test script to send sample events to Danube using danube-cli

set -e

# Configuration
DANUBE_URL=${DANUBE_URL:-http://localhost:6650}
TOPIC=${TOPIC:-/default/events}
COUNT=${COUNT:-10}
INTERVAL=${INTERVAL:-500}

# Auto-detect danube-cli (check local directory first, then PATH)
if [ -z "${DANUBE_CLI}" ]; then
    if [ -f "./danube-cli-linux" ]; then
        DANUBE_CLI="./danube-cli-linux"
    elif [ -f "./danube-cli-macos" ]; then
        DANUBE_CLI="./danube-cli-macos"
    elif [ -f "./danube-cli-windows.exe" ]; then
        DANUBE_CLI="./danube-cli-windows.exe"
    elif [ -f "./danube-cli" ]; then
        DANUBE_CLI="./danube-cli"
    elif command -v danube-cli &> /dev/null; then
        DANUBE_CLI="danube-cli"
    else
        DANUBE_CLI="danube-cli"  # Will fail later with helpful message
    fi
fi

echo "=" | tr '=' '\n' | head -c 60 && echo
echo "📤 Danube Producer for Delta Lake Sink Connector"
echo "=" | tr '=' '\n' | head -c 60 && echo
echo "Danube URL: ${DANUBE_URL}"
echo "Topic: ${TOPIC}"
echo "Count: ${COUNT}"
echo "Interval: ${INTERVAL}ms"
echo ""

# Check if danube-cli is available
if ! command -v ${DANUBE_CLI} &> /dev/null && [ ! -f "${DANUBE_CLI}" ]; then
    echo "❌ Error: danube-cli not found"
    echo ""
    echo "💡 Download danube-cli from:"
    echo "   https://github.com/danube-messaging/danube/releases"
    echo ""
    echo "   # Linux"
    echo "   wget https://github.com/danube-messaging/danube/releases/download/v0.5.2/danube-cli-linux"
    echo "   chmod +x danube-cli-linux"
    echo ""
    echo "   # macOS"
    echo "   wget https://github.com/danube-messaging/danube/releases/download/v0.5.2/danube-cli-macos"
    echo "   chmod +x danube-cli-macos"
    echo ""
    echo "Or specify custom path:"
    echo "   DANUBE_CLI=/path/to/danube-cli ./test_producer.sh"
    echo ""
    exit 1
fi

echo "Using danube-cli: ${DANUBE_CLI}"
echo ""

# Check Danube connectivity (TCP port check)
echo "🔍 Checking Danube connectivity..."
DANUBE_HOST=$(echo "${DANUBE_URL}" | sed -E 's|.*://([^:/]+).*|\1|')
DANUBE_PORT=$(echo "${DANUBE_URL}" | sed -E 's|.*:([0-9]+).*|\1|')

if ! timeout 2 bash -c "cat < /dev/null > /dev/tcp/${DANUBE_HOST}/${DANUBE_PORT}" 2>/dev/null; then
    echo "⚠️  Warning: Cannot reach Danube at ${DANUBE_URL}"
    echo "   Make sure Danube broker is running:"
    echo "   docker-compose ps danube-broker"
    echo ""
    read -p "Continue anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
else
    echo "✅ Danube broker is reachable"
fi

echo "✅ Ready to send messages"
echo ""
echo "Press Ctrl+C to stop"
echo ""

# Sample event types
declare -a EVENT_TYPES=("user_signup" "user_login" "purchase" "page_view" "api_call")
declare -a USER_IDS=("user_001" "user_002" "user_003" "user_004" "user_005")
declare -a PRODUCTS=("laptop" "phone" "tablet" "monitor" "keyboard")

# Generate a sample message (all messages will be the same for simplicity)
# In a real scenario, you'd want dynamic messages, but danube-cli --count sends the same message
user_id=${USER_IDS[$RANDOM % ${#USER_IDS[@]}]}
product=${PRODUCTS[$RANDOM % ${#PRODUCTS[@]}]}
amount=$((RANDOM % 1000 + 50))

message=$(cat <<EOF
{
  "event_type": "purchase",
  "user_id": "${user_id}",
  "product": "${product}",
  "amount": ${amount},
  "currency": "USD",
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
)

# Send messages using danube-cli with --count flag (creates ONE producer)
echo "Sending ${COUNT} messages with ${INTERVAL}ms interval..."
if ${DANUBE_CLI} produce \
    --service-addr "${DANUBE_URL}" \
    --topic "${TOPIC}" \
    --message "$message" \
    --count ${COUNT} \
    --interval ${INTERVAL} \
    --reliable; then
    success=${COUNT}
    failed=0
    echo "✅ Successfully sent ${COUNT} messages"
else
    success=0
    failed=${COUNT}
    echo "❌ Failed to send messages"
fi

# Summary
echo ""
echo "=" | tr '=' '\n' | head -c 60 && echo
echo "📊 Summary"
echo "=" | tr '=' '\n' | head -c 60 && echo
echo "Total: ${COUNT}"
echo "Success: ${success}"
echo "Failed: ${failed}"
echo "=" | tr '=' '\n' | head -c 60 && echo

if [ ${success} -gt 0 ]; then
    echo ""
    echo "💡 Next steps:"
    echo "   1. Check connector logs:"
    echo "      docker-compose logs -f deltalake-sink"
    echo ""
    echo "   2. Access MinIO Console:"
    echo "      http://localhost:9001 (minioadmin/minioadmin)"
    echo "      Browse to bucket: delta-tables/events"
    echo ""
    echo "   3. Query Delta table with Python:"
    echo "      pip install deltalake pandas"
    echo "      python3 -c 'import deltalake as dl; dt = dl.DeltaTable(\"s3://delta-tables/events\", storage_options={\"AWS_ACCESS_KEY_ID\": \"minioadmin\", \"AWS_SECRET_ACCESS_KEY\": \"minioadmin\", \"AWS_ENDPOINT_URL\": \"http://localhost:9000\", \"AWS_REGION\": \"us-east-1\", \"AWS_ALLOW_HTTP\": \"true\"}); print(dt.to_pandas())'"
    echo ""
fi
