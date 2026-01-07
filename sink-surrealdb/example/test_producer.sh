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
echo "📤 Danube Producer for SurrealDB Sink Connector"
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

# Send messages
success=0
failed=0

for i in $(seq 1 ${COUNT}); do
    # Generate random event data
    event_type=${EVENT_TYPES[$RANDOM % ${#EVENT_TYPES[@]}]}
    user_id=${USER_IDS[$RANDOM % ${#USER_IDS[@]}]}
    product=${PRODUCTS[$RANDOM % ${#PRODUCTS[@]}]}
    amount=$((RANDOM % 1000 + 50))
    
    # Create JSON message based on event type
    case $event_type in
        "purchase")
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
            ;;
        "user_signup")
            message=$(cat <<EOF
{
  "event_type": "user_signup",
  "user_id": "${user_id}",
  "email": "${user_id}@example.com",
  "source": "web",
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
)
            ;;
        "user_login")
            message=$(cat <<EOF
{
  "event_type": "user_login",
  "user_id": "${user_id}",
  "ip_address": "192.168.1.$((RANDOM % 255))",
  "device": "desktop",
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
)
            ;;
        "page_view")
            message=$(cat <<EOF
{
  "event_type": "page_view",
  "user_id": "${user_id}",
  "page": "/products/${product}",
  "duration_ms": $((RANDOM % 60000)),
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
)
            ;;
        "api_call")
            message=$(cat <<EOF
{
  "event_type": "api_call",
  "user_id": "${user_id}",
  "endpoint": "/api/v1/${product}",
  "method": "GET",
  "status_code": 200,
  "response_time_ms": $((RANDOM % 500)),
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
)
            ;;
    esac
    
    # Send message using danube-cli
    if ${DANUBE_CLI} produce \
        --service-addr "${DANUBE_URL}" \
        --topic "${TOPIC}" \
        --message "$message" \
        --interval ${INTERVAL} \
        --reliable \
        > /dev/null 2>&1; then
        success=$((success + 1))
        echo "✅ [${i}/${COUNT}] Sent: ${event_type} (${user_id})"
    else
        failed=$((failed + 1))
        echo "❌ [${i}/${COUNT}] Failed: ${event_type}"
    fi
    
    # Small delay between messages
    sleep 0.1
    
done

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
    echo "      docker-compose logs -f surrealdb-sink"
    echo ""
    echo "   2. Query SurrealDB (from container):"
    echo "      docker exec -it surrealdb /surreal sql --endpoint http://localhost:8000 --username root --password root --namespace default --database default"
    echo "      Then run: SELECT * FROM events;"
    echo ""
    echo "   3. Or use HTTP API:"
    echo "      curl -X POST http://localhost:8000/sql \\"
    echo "        -H 'Content-Type: application/json' \\"
    echo "        -H 'Accept: application/json' \\"
    echo "        -u root:root \\"
    echo "        -d '{\"query\": \"SELECT * FROM events LIMIT 10;\"}'"
    echo ""
fi
