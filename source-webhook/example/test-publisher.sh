#!/bin/bash
# Test webhook publisher script
# Sends test webhooks to the webhook connector to simulate external services

set -e

# Configuration
WEBHOOK_HOST="${WEBHOOK_HOST:-localhost}"
WEBHOOK_PORT="${WEBHOOK_PORT:-8080}"
API_KEY="${API_KEY:-test-api-key-12345}"
NETWORK="${NETWORK:-source-webhook_danube-webhook-network}"
WEBHOOK_CONTAINER="${WEBHOOK_CONTAINER:-webhook-example-connector}"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== Webhook Test Publisher ===${NC}"
echo "Target: http://${WEBHOOK_HOST}:${WEBHOOK_PORT}"
echo "API Key: ${API_KEY}"
echo ""

# Check if connector is running
if ! docker ps --format '{{.Names}}' | grep -q "^${WEBHOOK_CONTAINER}$"; then
    echo -e "${RED}Error: Webhook connector container '${WEBHOOK_CONTAINER}' is not running${NC}"
    echo "Start the example first: docker-compose up -d"
    exit 1
fi

# Check if connector is healthy
echo "Checking connector health..."
if ! curl -s -f "http://${WEBHOOK_HOST}:${WEBHOOK_PORT}/health" > /dev/null 2>&1; then
    echo -e "${YELLOW}Warning: Health check failed, but continuing...${NC}"
fi

echo -e "${GREEN}Starting to send test webhooks...${NC}"
echo ""

count=0
while true; do
    count=$((count + 1))
    timestamp=$(date +%s)
    
    echo -e "${BLUE}[$(date +%T)] Batch #${count}${NC}"
    
    # 1. Stripe payment webhook (partitioned, reliable)
    amount=$((RANDOM % 10000 + 1000))
    customer_id="cus_$(head /dev/urandom | tr -dc A-Za-z0-9 | head -c 10)"
    response=$(curl -X POST "http://${WEBHOOK_HOST}:${WEBHOOK_PORT}/webhooks/stripe/payments" \
        -H "Content-Type: application/json" \
        -H "x-api-key: ${API_KEY}" \
        -d "{
            \"event\": \"payment.succeeded\",
            \"amount\": ${amount},
            \"currency\": \"usd\",
            \"customer_id\": \"${customer_id}\",
            \"timestamp\": ${timestamp}
        }" \
        -s -w "\n%{http_code}" -o /dev/null)
    
    if [ "$response" = "200" ]; then
        echo -e "  ${GREEN}✓${NC} Stripe Payment: \$$(echo "scale=2; ${amount}/100" | bc) (${response})"
    else
        echo -e "  ${RED}✗${NC} Stripe Payment: HTTP ${response}"
    fi
    
    # 2. GitHub push webhook (partitioned, non-reliable)
    commits=$((RANDOM % 5 + 1))
    branch="feature/branch-${count}"
    response=$(curl -X POST "http://${WEBHOOK_HOST}:${WEBHOOK_PORT}/webhooks/github/push" \
        -H "Content-Type: application/json" \
        -H "x-api-key: ${API_KEY}" \
        -d "{
            \"event\": \"push\",
            \"repository\": \"danube-messaging/danube-connect\",
            \"branch\": \"${branch}\",
            \"commits\": ${commits},
            \"author\": \"test-user\",
            \"timestamp\": ${timestamp}
        }" \
        -s -w "\n%{http_code}" -o /dev/null)
    
    if [ "$response" = "200" ]; then
        echo -e "  ${GREEN}✓${NC} GitHub Push: ${commits} commits to ${branch} (${response})"
    else
        echo -e "  ${RED}✗${NC} GitHub Push: HTTP ${response}"
    fi
    
    # 3. Generic webhook (non-partitioned, reliable)
    event_types=("user.created" "user.updated" "user.deleted" "order.placed" "order.shipped")
    event_type=${event_types[$RANDOM % ${#event_types[@]}]}
    response=$(curl -X POST "http://${WEBHOOK_HOST}:${WEBHOOK_PORT}/webhooks/generic" \
        -H "Content-Type: application/json" \
        -H "x-api-key: ${API_KEY}" \
        -d "{
            \"event\": \"${event_type}\",
            \"data\": {
                \"id\": \"${count}\",
                \"value\": \"test-data-${count}\"
            },
            \"timestamp\": ${timestamp}
        }" \
        -s -w "\n%{http_code}" -o /dev/null)
    
    if [ "$response" = "200" ]; then
        echo -e "  ${GREEN}✓${NC} Generic Event: ${event_type} (${response})"
    else
        echo -e "  ${RED}✗${NC} Generic Event: HTTP ${response}"
    fi
    
    # 4. Alert webhook (non-partitioned, non-reliable)
    severity=$((RANDOM % 3 + 1))
    severity_names=("info" "warning" "critical")
    severity_name=${severity_names[$((severity - 1))]}
    response=$(curl -X POST "http://${WEBHOOK_HOST}:${WEBHOOK_PORT}/webhooks/alerts" \
        -H "Content-Type: application/json" \
        -H "x-api-key: ${API_KEY}" \
        -d "{
            \"event\": \"alert.triggered\",
            \"severity\": ${severity},
            \"severity_name\": \"${severity_name}\",
            \"message\": \"Test alert #${count}\",
            \"source\": \"monitoring-system\",
            \"timestamp\": ${timestamp}
        }" \
        -s -w "\n%{http_code}" -o /dev/null)
    
    if [ "$response" = "200" ]; then
        echo -e "  ${GREEN}✓${NC} Alert: ${severity_name} severity (${response})"
    else
        echo -e "  ${RED}✗${NC} Alert: HTTP ${response}"
    fi
    
    # 5. Test invalid endpoint (should return 404)
    if [ $((count % 10)) -eq 0 ]; then
        response=$(curl -X POST "http://${WEBHOOK_HOST}:${WEBHOOK_PORT}/webhooks/invalid" \
            -H "Content-Type: application/json" \
            -H "x-api-key: ${API_KEY}" \
            -d "{\"test\": \"invalid\"}" \
            -s -w "\n%{http_code}" -o /dev/null)
        
        if [ "$response" = "404" ]; then
            echo -e "  ${YELLOW}✓${NC} Invalid Endpoint Test: 404 as expected"
        else
            echo -e "  ${RED}✗${NC} Invalid Endpoint Test: HTTP ${response} (expected 404)"
        fi
    fi
    
    # 6. Test authentication failure (every 15 requests)
    if [ $((count % 15)) -eq 0 ]; then
        response=$(curl -X POST "http://${WEBHOOK_HOST}:${WEBHOOK_PORT}/webhooks/generic" \
            -H "Content-Type: application/json" \
            -H "x-api-key: wrong-api-key" \
            -d "{\"test\": \"auth\"}" \
            -s -w "\n%{http_code}" -o /dev/null)
        
        if [ "$response" = "401" ]; then
            echo -e "  ${YELLOW}✓${NC} Auth Failure Test: 401 as expected"
        else
            echo -e "  ${RED}✗${NC} Auth Failure Test: HTTP ${response} (expected 401)"
        fi
    fi
    
    echo ""
    sleep 5
done
