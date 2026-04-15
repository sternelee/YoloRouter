#!/bin/bash

# Quick diagnostic script for YoloRouter HTTP issues

echo "=== YoloRouter Diagnostic Tool ==="
echo ""

PORT=8989
BASE_URL="http://127.0.0.1:$PORT"

# Check if server is running
echo "1. Checking if server is running on port $PORT..."
if lsof -Pi :$PORT -sTCP:LISTEN -t >/dev/null 2>&1; then
    echo "   ✅ Server is running"
else
    echo "   ❌ Server is NOT running"
    echo "   Please start with: cargo run --release"
    exit 1
fi
echo ""

# Health check
echo "2. Health check..."
response=$(curl -s -w "\n%{http_code}" $BASE_URL/health)
http_code=$(echo "$response" | tail -n 1)
body=$(echo "$response" | head -n -1)

if [ "$http_code" = "200" ]; then
    echo "   ✅ Health check passed"
    echo "   Response: $body"
else
    echo "   ❌ Health check failed (HTTP $http_code)"
    exit 1
fi
echo ""

# Test non-streaming request
echo "3. Testing non-streaming request..."
response=$(curl -s -w "\n%{http_code}" -X POST $BASE_URL/v1/anthropic \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-opus",
    "messages": [{"role": "user", "content": "Say hello"}],
    "stream": false,
    "max_tokens": 10
  }')

http_code=$(echo "$response" | tail -n 1)
body=$(echo "$response" | head -n -1)

if [ "$http_code" = "200" ]; then
    echo "   ✅ Non-streaming request succeeded"
    echo "   Response: $(echo "$body" | jq -c '.choices[0].message.content // .error' 2>/dev/null || echo "$body" | head -c 100)"
else
    echo "   ❌ Non-streaming request failed (HTTP $http_code)"
    echo "   Error: $body"
fi
echo ""

# Test streaming request
echo "4. Testing streaming request..."
response=$(curl -s -w "\n%{http_code}" -X POST $BASE_URL/v1/anthropic \
  -H "Content-Type: application/json" \
  -d '{
    "model": "auto",
    "messages": [{"role": "user", "content": "Hi"}],
    "stream": true,
    "max_tokens": 20
  }' -N --max-time 5 2>&1)

if echo "$response" | grep -q "event:\|data:"; then
    echo "   ✅ Streaming request succeeded"
    echo "   First lines:"
    echo "$response" | head -3 | sed 's/^/   /'
else
    echo "   ⚠️  Streaming response format unexpected"
    echo "   Response: $(echo "$response" | head -c 200)"
fi
echo ""

# Test auto model selection
echo "5. Testing auto model selection..."
response=$(curl -s -w "\n%{http_code}" -X POST $BASE_URL/v1/anthropic \
  -H "Content-Type: application/json" \
  -d '{
    "model": "auto",
    "messages": [{"role": "user", "content": "test"}],
    "max_tokens": 10
  }')

http_code=$(echo "$response" | tail -n 1)

if [ "$http_code" = "200" ]; then
    echo "   ✅ Auto model selection works"
else
    echo "   ❌ Auto model selection failed (HTTP $http_code)"
fi
echo ""

# Test large payload
echo "6. Testing large payload (to check size limits)..."
large_content=$(python3 -c "print('a' * 5000)")
response=$(curl -s -w "\n%{http_code}" -X POST $BASE_URL/v1/anthropic \
  -H "Content-Type: application/json" \
  -d "{
    \"model\": \"claude-opus\",
    \"messages\": [{\"role\": \"user\", \"content\": \"$large_content\"}],
    \"max_tokens\": 10
  }")

http_code=$(echo "$response" | tail -n 1)

if [ "$http_code" = "200" ]; then
    echo "   ✅ Large payload accepted (5KB message)"
else
    echo "   ❌ Large payload rejected (HTTP $http_code)"
fi
echo ""

# Check stats
echo "7. Checking stats..."
stats=$(curl -s $BASE_URL/stats)
total=$(echo "$stats" | jq -r '.total_requests // "N/A"')
echo "   Total requests processed: $total"
echo ""

echo "=== Diagnostic Summary ==="
echo "If all tests passed (✅), your YoloRouter is working correctly."
echo "If you see errors only when using Claude Code, the issue is likely"
echo "with the headers Claude Code is sending."
echo ""
echo "Next steps:"
echo "  - Check Claude Code configuration"
echo "  - Enable debug logs: export RUST_LOG=debug,actix_web=debug"
echo "  - Review: docs/HTTP_HEADER_ERROR.md"
echo ""
