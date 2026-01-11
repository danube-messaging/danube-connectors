#!/bin/bash
# =============================================================================
# Manual Webhook Test Script
# =============================================================================
# This script sends test webhooks to the connector for manual testing
#
# Usage:
#   ./send-webhook.sh              # Send a single webhook
#   ./send-webhook.sh 10           # Send 10 webhooks
#   ./send-webhook.sh load         # Load test (100 webhooks)

set -e

# Configuration
WEBHOOK_URL="http://localhost:8080/webhooks/stripe/payments"
API_KEY="demo-api-key-e2e-12345"

# Event types for Stripe payments
EVENT_TYPES=("payment.succeeded" "payment.failed" "payment.canceled" "payment.refunded")

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to send a single webhook
send_webhook() {
    local count=$1
    local timestamp=$(date +%s)
    local amount=$((RANDOM % 50000 + 1000))
    local event_type=${EVENT_TYPES[$((RANDOM % 4))]}
    local customer_id="cus_test${count}"
    
    local payload=$(cat <<EOF
{
  "event": "${event_type}",
  "amount": ${amount},
  "currency": "USD",
  "timestamp": ${timestamp},
  "customer_id": "${customer_id}"
}
EOF
)
    
    local response=$(curl -s -w "\n%{http_code}" -X POST "${WEBHOOK_URL}" \
        -H "Content-Type: application/json" \
        -H "x-api-key: ${API_KEY}" \
        -d "${payload}")
    
    local http_code=$(echo "$response" | tail -n1)
    local body=$(echo "$response" | head -n-1)
    
    if [ "$http_code" = "200" ] || [ "$http_code" = "201" ] || [ "$http_code" = "202" ]; then
        echo -e "${GREEN}✓${NC} Webhook #${count}: ${event_type} | \$$(echo "scale=2; ${amount}/100" | bc) USD | HTTP ${http_code}"
    else
        echo -e "${RED}✗${NC} Webhook #${count}: ${event_type} | HTTP ${http_code} | ${body}"
    fi
}

# Main script
echo "==================================================================="
echo "        Webhook Test Script - End-to-End Example"
echo "==================================================================="
echo ""
echo "Target: ${WEBHOOK_URL}"
echo "API Key: ${API_KEY}"
echo ""

# Parse arguments
if [ "$1" = "load" ]; then
    echo "Running load test (100 webhooks)..."
    echo ""
    for i in {1..100}; do
        send_webhook $i &
    done
    wait
    echo ""
    echo -e "${GREEN}Load test complete!${NC}"
elif [ ! -z "$1" ]; then
    count=$1
    echo "Sending ${count} webhooks..."
    echo ""
    for i in $(seq 1 $count); do
        send_webhook $i
        sleep 0.5
    done
    echo ""
    echo -e "${GREEN}Done! Sent ${count} webhooks.${NC}"
else
    echo "Sending a single test webhook..."
    echo ""
    send_webhook 1
    echo ""
    echo -e "${GREEN}Done!${NC}"
fi

echo ""
echo "==================================================================="
echo "  Check SurrealDB to see the data:"
echo "  docker exec -it e2e-surrealdb /surreal sql \\"
echo "    --conn http://localhost:8000 --user root --pass root \\"
echo "    --ns default --db default"
echo ""
echo "  Then run: SELECT * FROM stripe_payments ORDER BY timestamp DESC;"
echo "==================================================================="
