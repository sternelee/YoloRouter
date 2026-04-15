#!/bin/bash

# Test streaming support for all proxy endpoints

PORT=8989
BASE_URL="http://localhost:$PORT"

echo "Testing streaming support for all endpoints..."
echo "=============================================="
echo ""

# Test Anthropic endpoint with streaming
echo "1. Testing /v1/anthropic with stream=true..."
curl -X POST "$BASE_URL/v1/anthropic" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-opus",
    "messages": [{"role": "user", "content": "Hello"}],
    "stream": true,
    "max_tokens": 100
  }' -N --no-buffer 2>&1 | head -5
echo -e "\n"

# Test Anthropic endpoint with auto model and streaming
echo "2. Testing /v1/anthropic with model=auto and stream=true..."
curl -X POST "$BASE_URL/v1/anthropic" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "auto",
    "messages": [{"role": "user", "content": "What is 2+2?"}],
    "stream": true,
    "max_tokens": 50
  }' -N --no-buffer 2>&1 | head -5
echo -e "\n"

# Test OpenAI endpoint with streaming
echo "3. Testing /v1/openai with stream=true..."
curl -X POST "$BASE_URL/v1/openai" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4",
    "messages": [{"role": "user", "content": "Hello"}],
    "stream": true,
    "max_tokens": 100
  }' -N --no-buffer 2>&1 | head -5
echo -e "\n"

# Test Gemini endpoint with streaming
echo "4. Testing /v1/gemini with stream=true..."
curl -X POST "$BASE_URL/v1/gemini" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gemini-pro",
    "messages": [{"role": "user", "content": "Hello"}],
    "stream": true,
    "max_tokens": 100
  }' -N --no-buffer 2>&1 | head -5
echo -e "\n"

# Test Codex endpoint with streaming
echo "5. Testing /v1/codex with stream=true..."
curl -X POST "$BASE_URL/v1/codex" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-5-mini",
    "messages": [{"role": "user", "content": "Hello"}],
    "stream": true,
    "max_tokens": 100
  }' -N --no-buffer 2>&1 | head -5
echo -e "\n"

# Test auto endpoint with streaming
echo "6. Testing /v1/auto with stream=true..."
curl -X POST "$BASE_URL/v1/auto" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "auto",
    "messages": [{"role": "user", "content": "Simple question"}],
    "stream": true,
    "max_tokens": 50
  }' -N --no-buffer 2>&1 | head -5
echo -e "\n"

# Test provider:model format with streaming
echo "7. Testing provider:model format with stream=true..."
curl -X POST "$BASE_URL/v1/auto" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "openai:gpt-4",
    "messages": [{"role": "user", "content": "Test"}],
    "stream": true,
    "max_tokens": 50
  }' -N --no-buffer 2>&1 | head -5
echo -e "\n"

echo "=============================================="
echo "Streaming tests complete!"
