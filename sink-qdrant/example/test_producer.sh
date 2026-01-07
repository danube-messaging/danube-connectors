#!/bin/bash
# Test script to send embeddings to Danube using danube-cli

set -e

# Configuration
DANUBE_URL=${DANUBE_URL:-http://localhost:6650}
TOPIC=${TOPIC:-/default/vectors}
EMBEDDINGS_FILE=${EMBEDDINGS_FILE:-embeddings.jsonl}
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
echo "📤 Danube Producer for Qdrant Sink Connector"
echo "=" | tr '=' '\n' | head -c 60 && echo
echo "Danube URL: ${DANUBE_URL}"
echo "Topic: ${TOPIC}"
echo "Input File: ${EMBEDDINGS_FILE}"
echo "Interval: ${INTERVAL}ms"
echo ""

# Check if embeddings file exists
if [ ! -f "${EMBEDDINGS_FILE}" ]; then
    echo "❌ Error: Embeddings file not found: ${EMBEDDINGS_FILE}"
    echo ""
    echo "💡 Generate embeddings first:"
    echo "   python3 generate_embeddings.py --count 10"
    echo ""
    exit 1
fi

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

# Count total messages
total=$(wc -l < "${EMBEDDINGS_FILE}")
echo "📊 Found ${total} messages in ${EMBEDDINGS_FILE}"
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

# Send messages
count=0
success=0
failed=0

while IFS= read -r message; do
    count=$((count + 1))
    
    # Extract just the text for display (if it exists)
    text=$(echo "$message" | jq -r '.payload.text // "N/A"' 2>/dev/null || echo "N/A")
    text_short="${text:0:50}"
    
    # Send message using danube-cli (use string schema, connector will parse JSON)
    if ${DANUBE_CLI} produce \
        --service-addr "${DANUBE_URL}" \
        --topic "${TOPIC}" \
        --message "$message" \
        --interval ${INTERVAL} \
        --reliable \
        > /dev/null 2>&1; then
        success=$((success + 1))
        echo "✅ [${count}/${total}] Sent: ${text_short}..."
    else
        failed=$((failed + 1))
        echo "❌ [${count}/${total}] Failed: ${text_short}..."
    fi
    
    # Small delay between messages
    sleep 0.1
    
done < "${EMBEDDINGS_FILE}"

# Summary
echo ""
echo "=" | tr '=' '\n' | head -c 60 && echo
echo "📊 Summary"
echo "=" | tr '=' '\n' | head -c 60 && echo
echo "Total: ${total}"
echo "Success: ${success}"
echo "Failed: ${failed}"
echo "=" | tr '=' '\n' | head -c 60 && echo

if [ ${success} -gt 0 ]; then
    echo ""
    echo "💡 Next steps:"
    echo "   1. Check connector logs:"
    echo "      docker-compose logs -f qdrant-sink"
    echo ""
    echo "   2. View Qdrant dashboard:"
    echo "      http://localhost:6333/dashboard"
    echo ""
    echo "   3. Search vectors:"
    echo "      ./search_vectors.py --query 'password reset'"
    echo ""
fi
