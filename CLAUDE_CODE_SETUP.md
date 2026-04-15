# Claude Code 集成指南

YoloRouter 现已完全支持 Claude Code 和其他 Anthropic 客户端。本文档说明如何配置你的 Claude Code 以通过 YoloRouter 路由请求。

## 问题解决

### 错误：`Invalid model name 'auth'`

这个错误通常是因为将 `ANTHROPIC_MODEL` 设置为了 `"auth"` 而不是 `"auto"`。

**常见原因**:
- 打字错误：将 `"auto"` 误输入为 `"auth"`
- 混淆了 API key 和模型名配置

**解决方案**:
1. 对于自动路由,将模型设置为 `"auto"`(不是 `"auth"`)
2. 或者指定一个具体的 Anthropic 模型:
   - `"claude-opus-4"`
   - `"claude-sonnet-4-5"`
   - `"claude-haiku-4"`

### ✅ 流式响应现已支持 "auto" 模型!

**新功能(v0.1.0+)**: 现在可以在流式请求中使用 `model="auto"`,YoloRouter 会:
1. 根据请求内容自动分析最佳模型
2. 选择合适的 Anthropic 模型
3. 开始流式传输响应

**配置示例**:
```json
{
  "ANTHROPIC_AUTH_TOKEN": "sk-ant-xxxxxxxxxxxxx",
  "ANTHROPIC_BASE_URL": "http://127.0.0.1:8989/v1/anthropic",
  "ANTHROPIC_MODEL": "auto"
}
```

这个配置现在可以用于:
- ✅ 非流式请求(stream=false)
- ✅ 流式请求(stream=true) - **新支持!**

### 错误：`invalid type: sequence, expected a string`

这个错误表示你的客户端发送的 `system` 字段格式不被支持。最新版本的 YoloRouter 现已支持多种 `system` 字段格式:

- **字符串格式**:`"system": "You are a helpful assistant"`
- **Content blocks 格式**(Claude Code 发送):`"system": [{"type": "text", "text": "..."}]`

**解决方案**:将 YoloRouter 升级到最新版本(已修复此问题)。

## Claude Code 配置

### 1. 启动 YoloRouter

```bash
cd /Users/sternelee/www/github/YoloRouter
cargo build --release
YOLO_CONFIG=config.toml ./target/release/yolo-router
```

确保服务器在 `http://127.0.0.1:8989` 运行(或根据你的 `config.toml` 设置的端口)。

### 2. 配置 Claude Code 的代理参数

**推荐配置(支持流式 + 自动路由)**:

```json
{
  "ANTHROPIC_AUTH_TOKEN": "sk-ant-xxxxxxxxxxxxx",
  "ANTHROPIC_BASE_URL": "http://127.0.0.1:8989/v1/anthropic",
  "ANTHROPIC_MODEL": "auto"
}
```

**固定模型配置(如果你想指定模型)**:

```json
{
  "ANTHROPIC_AUTH_TOKEN": "sk-ant-xxxxxxxxxxxxx",
  "ANTHROPIC_BASE_URL": "http://127.0.0.1:8989/v1/anthropic",
  "ANTHROPIC_MODEL": "claude-opus-4"
}
```

**参数说明**:

| 参数 | 值 | 说明 |
|------|-----|------|
| `ANTHROPIC_AUTH_TOKEN` | 你的 Anthropic API Key | 从 [console.anthropic.com](https://console.anthropic.com) 获取 |
| `ANTHROPIC_BASE_URL` | `http://127.0.0.1:8989/v1/anthropic` | YoloRouter 的 Anthropic 端点(不要修改路径部分) |
| `ANTHROPIC_MODEL` | `auto` 或具体模型名 | **推荐 `auto`** - 智能选择最佳模型<br>或指定如 `claude-opus-4` |

**有效的模型名示例**:
- `auto` — **推荐** - YoloRouter 智能选择(支持流式!)
- `claude-opus-4` — 最强大的模型
- `claude-sonnet-4-5` — 平衡性能和成本
- `claude-haiku-4` — 最快最便宜
- `anthropic:claude-opus-4` — 带 provider 前缀(可选)

### 3. 配置场景(可选,推荐)

在 `config.toml` 中添加场景定义,YoloRouter 会根据请求内容自动选择:

```toml
[scenarios.coding]
models = [
  { provider = "anthropic", model = "claude-opus-4", cost_tier = "high" },
  { provider = "anthropic", model = "claude-sonnet-4", cost_tier = "medium" }
]
match_task_types = ["coding"]

[scenarios.general]
models = [
  { provider = "anthropic", model = "claude-sonnet-4", cost_tier = "medium" },
  { provider = "anthropic", model = "claude-haiku-4", cost_tier = "low" }
]
is_default = true
```

当使用 `model="auto"` 时:
- 编程相关请求 → 选择 `coding` 场景 → `claude-opus-4`
- 其他请求 → 选择 `general` 场景 → `claude-sonnet-4`

### 4. 验证连接

启动 Claude Code 后,你应该看到:

```bash
# 在 YoloRouter 日志中:
[INFO] POST /v1/anthropic - 200 OK (1234ms)
[INFO] Auto-selected model for streaming request model=claude-opus-4 scenario=Some("coding")
```

## 高级配置

### A. 场景感知路由

在你的 `config.toml` 中定义场景，YoloRouter 会根据请求自动选择最佳模型：

```toml
[scenarios.coding]
models = [
  { provider = "anthropic", model = "claude-opus", cost_tier = "high" },
  { provider = "anthropic", model = "claude-sonnet", cost_tier = "medium" }
]
match_task_types = ["coding"]

[scenarios.general]
models = [
  { provider = "anthropic", model = "claude-sonnet", cost_tier = "medium" },
  { provider = "anthropic", model = "claude-haiku", cost_tier = "low" }
]
is_default = true
```

### B. 多提供商故障转移

配置多个 AI 提供商，在一个失败时自动转移到下一个：

```toml
[providers.anthropic]
type = "anthropic"
api_key = "${ANTHROPIC_API_KEY}"

[providers.openai]
type = "openai"
api_key = "${OPENAI_API_KEY}"

[scenarios.critical]
models = [
  { provider = "anthropic", model = "claude-opus" },
  { provider = "openai", model = "gpt-4" },
  { provider = "anthropic", model = "claude-sonnet" }
]
```

### C. 性能优化

Claude Code 会发送系统提示（system prompt）和对话历史。YoloRouter 自动处理：

- ✅ Content blocks 格式的 system 字段（支持文本和图像）
- ✅ 流式响应（stream=true）
- ✅ Token 计数和成本追踪
- ✅ 故障转移和自动重试

## 请求流程

当你在 Claude Code 中提交请求时：

```
Claude Code
    ↓
    POST http://127.0.0.1:8989/v1/anthropic
    {
      "model": "claude-opus",
      "messages": [...],
      "system": [{"type": "text", "text": "..."}],  ← Content blocks
      "max_tokens": 1000
    }
    ↓
YoloRouter
    ↓
    [解析 Anthropic 格式]
    ↓
    [15D 分析请求特征]
    ↓
    [选择最佳场景和模型]
    ↓
    [转发到 Anthropic API 或其他提供商]
    ↓
    [返回 Anthropic 格式响应]
    ↓
Claude Code
    ↓
    显示结果
```

## 故障排除

### 问题：连接被拒绝

```
Error: Connection refused
```

**原因**：YoloRouter 未运行或端口配置错误

**解决**：
```bash
# 检查 YoloRouter 是否运行
curl http://127.0.0.1:8989/health

# 如果失败，启动服务器
cargo run --release -- --config config.toml
```

### 问题：`invalid_request_error`

```json
{
  "type": "error",
  "error": {
    "type": "invalid_request_error",
    "message": "..."
  }
}
```

**原因**：请求格式不对、缺少字段或 API Key 无效

**解决**：
1. 验证 `ANTHROPIC_AUTH_TOKEN` 有效
2. 检查 `ANTHROPIC_BASE_URL` 正确（应以 `/v1/anthropic` 结尾）
3. 确保 `model` 和 `max_tokens` 字段存在
4. 检查 YoloRouter 日志：`tail -f yolo-router.log`

### 问题：请求超时

```
Error: Request timeout
```

**原因**：Anthropic API 响应缓慢或网络问题

**解决**：
1. 在 `config.toml` 增加超时时间：
   ```toml
   [routing]
   timeout_ms = 60000  # 60 秒
   ```
2. 检查网络连接：`curl https://api.anthropic.com/health`
3. 检查 Anthropic API 状态：[status.anthropic.com](https://status.anthropic.com)

### 问题：模型不可用

```json
{
  "error": {
    "type": "invalid_request_error",
    "message": "Model 'claude-opus' not available"
  }
}
```

**原因**：模型名称错误或你的 API Key 没有权限

**解决**：
1. 验证模型名称（当前支持：claude-opus, claude-sonnet, claude-haiku）
2. 检查 API Key 权限：登录 [console.anthropic.com](https://console.anthropic.com)
3. 在 `config.toml` 中将模型改为可用的版本

## 最佳实践

### 1. 使用环境变量保护 API Key

```bash
export ANTHROPIC_API_KEY="sk-ant-xxxxxxxxxxxxx"
```

**不要**在 `config.toml` 中硬编码密钥。

### 2. 启用故障转移确保可用性

```toml
[routing]
fallback_enabled = true
retry_count = 2
```

### 3. 监控请求统计

```bash
curl http://127.0.0.1:8989/stats | jq .
```

输出示例：
```json
{
  "total_requests": 150,
  "total_successes": 145,
  "total_errors": 5,
  "average_response_time_ms": 1250.5,
  "providers_called": {
    "anthropic": 100,
    "openai": 45
  }
}
```

### 4. 定期检查日志

```bash
# 实时监控日志
RUST_LOG=info cargo run --release -- --config config.toml

# 或使用已构建的二进制
RUST_LOG=debug ./target/release/yolo-router --config config.toml
```

## 技术详情

### Anthropic 端点兼容性

YoloRouter 的 `/v1/anthropic` 端点完全兼容：

- ✅ Claude Code IDE
- ✅ Anthropic Python SDK (`anthropic.Anthropic()`)
- ✅ Node.js SDK (`new Anthropic()`)
- ✅ 其他 Anthropic 客户端库

### 支持的 Request 字段

```json
{
  "model": "string",           // 必需
  "messages": [                // 必需
    {"role": "user|assistant", "content": "string"}
  ],
  "max_tokens": "number",      // 必需，Anthropic 要求
  "system": "string|array",    // 可选，支持文本和 content blocks
  "temperature": "number",     // 可选，0.0-1.0
  "top_p": "number"            // 可选，0.0-1.0
}
```

### 支持的 Response 字段

```json
{
  "id": "msg_...",
  "type": "message",
  "role": "assistant",
  "model": "claude-opus",
  "content": [
    {"type": "text", "text": "..."}
  ],
  "stop_reason": "end_turn|max_tokens",
  "usage": {
    "input_tokens": 100,
    "output_tokens": 50
  }
}
```

## 版本兼容性

| 版本 | 功能 | 状态 |
|------|------|------|
| >= 0.1.0 | 基础 Anthropic 路由 | ✅ 稳定 |
| >= 0.2.0 | Content blocks system 字段 | ✅ 稳定（最新） |
| 计划中 | 流式响应（SSE） | ⏳ 即将推出 |
| 计划中 | Vision（图像处理） | ⏳ 即将推出 |

## 获取帮助

如果遇到问题：

1. **检查日志**：`RUST_LOG=debug cargo run --release -- --config config.toml`
2. **验证配置**：`curl http://127.0.0.1:8989/config | jq .`
3. **检查健康状态**：`curl http://127.0.0.1:8989/health`
4. **测试连接**：
   ```bash
   curl -X POST http://127.0.0.1:8989/v1/anthropic \
     -H "Content-Type: application/json" \
     -d '{
       "model": "claude-opus",
       "messages": [{"role": "user", "content": "Hello"}],
       "max_tokens": 100
     }'
   ```

---

**更新日期**：2024年 | **YoloRouter 版本**：>= 0.1.0
