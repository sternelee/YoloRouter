# Claude Code 完整配置指南（2024年4月更新）

## 问题解决历程

### 问题 1: HTTPS vs HTTP ✅ 已修复
**错误**: `stream error: request parse error: invalid Header provided`  
**原因**: 配置使用了 `https://` 但 YoloRouter 运行在 `http://`  
**修复**: 更改为 `http://127.0.0.1:8989/v1/anthropic`

### 问题 2: provider:model 格式不支持 ✅ 已修复
**错误**: `stream=true only supports Anthropic models. Got provider prefix 'github_copilot'`  
**原因**: 旧版本只支持 Anthropic 模型  
**修复**: v0.1.0+ 现在支持任意 `provider:model` 格式

## ✅ 当前推荐配置

您的配置文件: `~/.claude/settings.json`

### 配置 1: GitHub Copilot（推荐 - 如果您有 Copilot 订阅）

```json
{
  "env": {
    "ANTHROPIC_BASE_URL": "http://127.0.0.1:8989/v1/anthropic",
    "ANTHROPIC_MODEL": "github_copilot:gpt-5.4",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL": "github_copilot:gpt-5.4-mini",
    "ANTHROPIC_DEFAULT_OPUS_MODEL": "github_copilot:gpt-5.4",
    "ANTHROPIC_DEFAULT_SONNET_MODEL": "github_copilot:gpt-5.4",
    "ANTHROPIC_REASONING_MODEL": "github_copilot:gpt-5.4"
  }
}
```

### 配置 2: Auto 自动选择（最智能）

```json
{
  "env": {
    "ANTHROPIC_BASE_URL": "http://127.0.0.1:8989/v1/anthropic",
    "ANTHROPIC_MODEL": "auto"
  }
}
```

## 快速验证

### 1. 检查配置是否正确

```bash
cat ~/.claude/settings.json | grep ANTHROPIC_BASE_URL
# 应该显示: "ANTHROPIC_BASE_URL": "http://127.0.0.1:8989/v1/anthropic"
# 注意: 是 http:// 不是 https://
```

### 2. 启动 YoloRouter

```bash
cd /path/to/YoloRouter
cargo run --release
```

### 3. 测试连接

```bash
curl -X POST http://127.0.0.1:8989/v1/anthropic \
  -H "Content-Type: application/json" \
  -d '{
    "model": "github_copilot:gpt-5.4",
    "messages": [{"role": "user", "content": "Say hello"}],
    "stream": true,
    "max_tokens": 20
  }' -N | head -5
```

### 4. 重启 Claude Code

配置修改后需要重启 Claude Code 才能生效。

## 支持的所有格式

| 格式 | 示例 | 流式支持 | 说明 |
|------|------|---------|------|
| auto | `"auto"` | ✅ | 自动选择 |
| GitHub Copilot | `"github_copilot:gpt-5.4"` | ✅ | 需要 Copilot 订阅 |
| OpenAI | `"openai:gpt-4"` | ✅ | 需要 API key |
| Anthropic | `"claude-opus-4"` | ✅ | 需要 API key |
| Codex | `"codex:gpt-5-mini"` | ✅ | 需要 ChatGPT Pro |

## 故障排查清单

- [ ] 确认使用 `http://` 而不是 `https://`
- [ ] 确认 YoloRouter 正在运行（`lsof -i :8989`）
- [ ] 确认端口是 8989
- [ ] 确认路径是 `/v1/anthropic`（不要末尾斜杠）
- [ ] 确认 YoloRouter 版本是 v0.1.0+
- [ ] 重启了 Claude Code

## 完整测试脚本

```bash
#!/bin/bash
echo "Testing YoloRouter with Claude Code config..."

# 1. Check config
echo "1. Checking config..."
grep "ANTHROPIC_BASE_URL" ~/.claude/settings.json

# 2. Check server
echo "2. Checking if server is running..."
curl -s http://127.0.0.1:8989/health || echo "❌ Server not running!"

# 3. Test streaming
echo "3. Testing streaming with github_copilot:gpt-5.4..."
curl -X POST http://127.0.0.1:8989/v1/anthropic \
  -H "Content-Type: application/json" \
  -d '{
    "model": "github_copilot:gpt-5.4",
    "messages": [{"role": "user", "content": "test"}],
    "stream": true,
    "max_tokens": 10
  }' -N | head -3

echo "✅ If you see SSE data above, everything works!"
```

## 下一步

1. **确认配置已修改**: `cat ~/.claude/settings.json | grep http`
2. **重启 YoloRouter**: `cargo run --release`
3. **重启 Claude Code**
4. **发送测试消息**

如果仍有问题，运行:
```bash
export RUST_LOG=debug
cargo run --release
```

然后在 Claude Code 中发送请求，查看详细日志。
