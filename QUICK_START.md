# YoloRouter + Claude Code 快速配置指南

## ⚡ 快速开始（5 分钟）

### 1️⃣ 构建 YoloRouter

```bash
cd /Users/sternelee/www/github/YoloRouter
cargo build --release
```

✅ 编译成功，二进制文件位于 `target/release/yolo-router`

### 2️⃣ 配置环境变量

```bash
# 设置你的 Anthropic API Key
export ANTHROPIC_API_KEY="sk-ant-xxxxxxxxxxxxx"
```

从 [console.anthropic.com](https://console.anthropic.com) 获取你的 API Key。

### 3️⃣ 启动服务器

```bash
YOLO_CONFIG=config.toml ./target/release/yolo-router
```

你应该看到：
```
2026-04-13T06:23:55.499959Z  INFO yolo_router: Starting YoloRouter daemon
2026-04-13T06:23:55.500092Z  INFO yolo_router: Listening on 127.0.0.1:8989
2026-04-13T06:23:55.506838Z  INFO yolo_router::server: Starting YoloRouter HTTP server on 127.0.0.1:8989
```

### 4️⃣ 验证服务

```bash
# 健康检查
curl http://127.0.0.1:8989/health

# 查看统计
curl http://127.0.0.1:8989/stats
```

### 5️⃣ 配置 Claude Code

在 Claude Code 的设置中，配置以下代理参数：

```json
{
  "ANTHROPIC_AUTH_TOKEN": "sk-ant-xxxxxxxxxxxxx",
  "ANTHROPIC_BASE_URL": "http://127.0.0.1:8989/v1/anthropic",
  "ANTHROPIC_MODEL": "auto"
}
```

**参数说明**：
- `ANTHROPIC_AUTH_TOKEN` — 你的 Anthropic API Key（推荐）
- `ANTHROPIC_BASE_URL` — YoloRouter 的 Anthropic 端点
- `ANTHROPIC_MODEL` — `auto` 表示由 YoloRouter 智能选择

### 6️⃣ 测试连接

在 Claude Code 中发送任何消息，你应该在 YoloRouter 日志中看到：

```
INFO POST /v1/anthropic - 200 OK (1234ms)
INFO Routing: scenario=coding, provider=github_copilot, model=gpt-5-mini
```

✅ **完成！** Claude Code 现在通过 YoloRouter 路由请求。

---

## 常见问题

### Q: 端口已被占用怎么办？

编辑 `config.toml`，修改端口：

```toml
[daemon]
port = 8990  # 改为其他端口
```

然后重新启动服务器，在 Claude Code 中也要更新 `ANTHROPIC_BASE_URL`：

```
http://127.0.0.1:8990/v1/anthropic
```

### Q: 连接被拒绝

确保服务器正在运行：

```bash
# 检查进程
ps aux | grep yolo-router

# 启动（如果没运行）
./target/release/yolo-router --config config.toml
```

### Q: 400 错误：`invalid_request_error`

检查：
1. `ANTHROPIC_AUTH_TOKEN` 是否有效
2. `ANTHROPIC_BASE_URL` 是否正确（以 `/v1/anthropic` 结尾）
3. 检查 YoloRouter 日志

### Q: 如何选择特定的 AI 模型？

修改 `config.toml` 中的场景定义：

```toml
[scenarios.coding]
models = [
  { provider = "github_copilot", model = "gpt-5-mini", cost_tier = "low" },
  { provider = "github_copilot", model = "gpt-4-turbo", cost_tier = "medium" }
]
```

或在 Claude Code 中指定：

```json
{
  "ANTHROPIC_MODEL": "claude-opus"
}
```

---

## 进阶配置

### 多提供商故障转移

```toml
[providers.anthropic]
type = "anthropic"
api_key = "${ANTHROPIC_API_KEY}"

[providers.openai]
type = "openai"
api_key = "${OPENAI_API_KEY}"

[scenarios.coding]
models = [
  { provider = "github_copilot", model = "gpt-5-mini" },
  { provider = "anthropic", model = "claude-opus" },
  { provider = "openai", model = "gpt-4" }
]
```

YoloRouter 会按顺序尝试，失败时自动转移到下一个。

### 监控和统计

```bash
# 实时监控请求
watch -n 2 'curl -s http://127.0.0.1:8989/stats | jq .'

# 查看详细日志
RUST_LOG=debug ./target/release/yolo-router --config config.toml
```

---

## 文件和文档

| 文件 | 说明 |
|------|------|
| `target/release/yolo-router` | 编译后的二进制 |
| `config.toml` | 配置文件 |
| `CLAUDE_CODE_SETUP.md` | 完整集成指南 |
| `CLAUDE_CODE_FIX_SUMMARY.md` | 问题修复说明 |
| `USER_GUIDE.md` | 用户文档 |
| `AGENTS.md` | 开发者参考 |

---

## 下一步

✅ **基础配置完成** → 查看 `CLAUDE_CODE_SETUP.md` 了解高级功能

✅ **遇到问题** → 检查 `CLAUDE_CODE_SETUP.md` 的故障排除部分

✅ **开发相关** → 查看 `AGENTS.md` 和 `.github/copilot-instructions.md`

---

**状态**：✅ 所有测试通过 (51/51)
**编译**：✅ Release 构建成功
**集成**：✅ Claude Code 支持已启用
