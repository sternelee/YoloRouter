#!/bin/bash

# 测试用例: 模拟 Claude Code 发送 model="auth" 的请求

curl -s -X POST http://127.0.0.1:8989/v1/anthropic \
  -H "Content-Type: application/json" \
  -d '{
    "model": "auth",
    "messages": [{"role": "user", "content": "test"}],
    "max_tokens": 100,
    "stream": true
  }' | jq .

