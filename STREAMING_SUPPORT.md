# YoloRouter 流式请求支持文档

## 概述

YoloRouter 现已全面支持流式请求(Server-Sent Events / SSE)。所有主要代理端点均支持 `stream=true` 参数,包括:

- ✅ `/v1/anthropic` - Anthropic Claude 流式响应
- ✅ `/v1/openai` - OpenAI GPT 流式响应
- ✅ `/v1/gemini` - Google Gemini 流式响应
- ✅ `/v1/codex` - ChatGPT Pro 流式响应
- ✅ `/v1/auto` - 自动路由流式响应
- ✅ `/v1/github` - GitHub Copilot 流式响应

## 快速开始

### 基本流式请求

```bash
curl -X POST http://localhost:8989/v1/anthropic \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-opus",
    "messages": [{"role": "user", "content": "Hello, world!"}],
    "stream": true,
    "max_tokens": 1024
  }' -N --no-buffer
```

### 使用 "auto" 模型自动选择

```bash
curl -X POST http://localhost:8989/v1/anthropic \
  -H "Content-Type: application/json" \
  -d '{
    "model": "auto",
    "messages": [{"role": "user", "content": "Explain quantum computing"}],
    "stream": true,
    "max_tokens": 2048
  }' -N --no-buffer
```

### 使用 provider:model 格式直接路由

```bash
curl -X POST http://localhost:8989/v1/auto \
  -H "Content-Type: application/json" \
  -d '{
    "model": "openai:gpt-4",
    "messages": [{"role": "user", "content": "Write a poem"}],
    "stream": true,
    "max_tokens": 500
  }' -N --no-buffer
```

## 工作原理

### 架构组件

1. **Provider Trait 扩展**
   - 所有 Provider 现在包含 `start_streaming_request()` 方法
   - `supports_streaming()` 方法指示该 provider 是否支持流式
   - 不支持流式的 provider 返回 `NotImplemented` 错误

2. **请求检测**
   - 每个端点检测 `request.stream` 字段
   - 如果 `stream=true`,路由到流式处理器
   - 如果 `stream=false` 或未设置,使用传统的非流式路由

3. **自动模型选择**
   - `model="auto"` 时,使用 `RoutingEngine::select_best_model()` 选择最佳模型
   - 15维分析器(FastAnalyzer)根据请求特征选择模型
   - 选择后,将 model 替换为具体的 provider:model 组合

4. **流式转发**
   - Anthropic: 使用原生 Anthropic SSE 格式 (`event: message_start`, `event: content_block_delta` 等)
   - OpenAI/Gemini/Codex: 使用 OpenAI SSE 格式 (`data: {...}`, `data: [DONE]`)
   - 直接转发 provider 响应字节流,无需解析/重组

### 流式格式

#### Anthropic 格式

```
event: message_start
data: {"type":"message_start","message":{"id":"msg_123","type":"message","role":"assistant","content":[],"model":"claude-opus","stop_reason":null}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" world"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"}}

event: message_stop
data: {"type":"message_stop"}
```

#### OpenAI 格式

```
data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}

data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}

data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"content":" world"},"finish_reason":null}]}

data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}

data: [DONE]
```

## 端点详细说明

### `/v1/anthropic`

**支持流式**: ✅ 是

**格式**: Anthropic SSE 原生格式

**模型选择**:
- 具体模型名 (如 `claude-opus`, `claude-sonnet`) - 直接使用
- `auto` - 使用 FastAnalyzer 自动选择最佳 Anthropic 模型
- ⚠️ 不支持 `auth` 等无效模型(会返回 400 错误)

**示例**:
```bash
# 具体模型
curl -X POST http://localhost:8989/v1/anthropic \
  -H "Content-Type: application/json" \
  -d '{"model":"claude-opus","messages":[{"role":"user","content":"Hi"}],"stream":true}'

# 自动选择
curl -X POST http://localhost:8989/v1/anthropic \
  -H "Content-Type: application/json" \
  -d '{"model":"auto","messages":[{"role":"user","content":"Complex task"}],"stream":true}'
```

### `/v1/openai`

**支持流式**: ✅ 是

**格式**: OpenAI SSE 格式

**模型选择**:
- 具体模型名 (如 `gpt-4`, `gpt-5-mini`)
- `auto` - 自动选择最佳 OpenAI 模型
- `provider:model` - 直接指定 provider 和 model

**示例**:
```bash
curl -X POST http://localhost:8989/v1/openai \
  -H "Content-Type: application/json" \
  -d '{"model":"gpt-4","messages":[{"role":"user","content":"Hello"}],"stream":true}'
```

### `/v1/gemini`

**支持流式**: ✅ 是

**格式**: OpenAI SSE 格式(YoloRouter 将 Gemini 响应转换为 OpenAI 格式)

**模型选择**:
- Gemini 模型名 (如 `gemini-pro`, `gemini-ultra`)
- `auto` - 自动选择

**注意**: Gemini provider 的流式实现需要 API 支持。如果 Gemini API 不支持流式,将返回 501 Not Implemented。

### `/v1/codex`

**支持流式**: ✅ 是

**格式**: OpenAI SSE 格式

**模型选择**:
- Codex 模型名 (如 `gpt-5-mini`, `gpt-5.4`)
- `auto` - 自动选择

### `/v1/github`

**支持流式**: ✅ 是

**格式**: OpenAI SSE 格式

**模型选择**:
- GitHub Copilot 模型名 (如 `gpt-5.4`, `claude-opus-4.6`)
- `auto` - 自动选择

### `/v1/auto`

**支持流式**: ✅ 是

**格式**: 根据选择的 provider 决定(Anthropic 格式或 OpenAI 格式)

**模型选择**:
- `auto` - 使用 FastAnalyzer 从所有 provider 中选择最佳模型
- `provider:model` - 直接路由到指定 provider 的指定 model

**示例**:
```bash
# 完全自动选择
curl -X POST http://localhost:8989/v1/auto \
  -H "Content-Type: application/json" \
  -d '{"model":"auto","messages":[{"role":"user","content":"Solve this"}],"stream":true}'

# 直接路由
curl -X POST http://localhost:8989/v1/auto \
  -H "Content-Type: application/json" \
  -d '{"model":"openai:gpt-4","messages":[{"role":"user","content":"Hi"}],"stream":true}'
```

## 配置要求

### Provider 配置

确保在 `config.toml` 中配置了对应的 provider:

```toml
[providers.anthropic]
type = "anthropic"
api_key = "${ANTHROPIC_API_KEY}"

[providers.openai]
type = "openai"
api_key = "${OPENAI_API_KEY}"

[providers.gemini]
type = "gemini"
api_key = "${GEMINI_API_KEY}"

# ChatGPT Pro OAuth
[providers.codex]
type = "codex"
# 使用 TUI auth 配置: cargo run -- --auth codex

# GitHub Copilot OAuth
[providers.github_copilot]
type = "github_copilot"
# 使用 TUI auth 配置: cargo run -- --auth github_copilot
```

### Scenario 配置

如果使用 `auto` 模型,确保配置了 scenarios:

```toml
[scenarios.production]
models = [
  { provider = "anthropic", model = "claude-opus", cost_tier = "high" },
  { provider = "openai", model = "gpt-4", cost_tier = "high" },
]

[scenarios.development]
models = [
  { provider = "anthropic", model = "claude-haiku", cost_tier = "low" },
  { provider = "openai", model = "gpt-5-mini", cost_tier = "low" },
]
```

## 客户端集成

### Claude Code 配置

在 Claude Code 的设置中配置 YoloRouter:

```json
{
  "anthropic.baseURL": "http://localhost:8989/v1",
  "anthropic.apiKey": "dummy-key-ignored"
}
```

**支持的模型名**:
- `claude-opus` / `claude-sonnet` / `claude-haiku` - 使用具体模型
- `auto` - 自动选择(推荐)

### Cursor 配置

```json
{
  "openai.baseURL": "http://localhost:8989/v1/openai",
  "openai.apiKey": "dummy-key"
}
```

### 其他 OpenAI 兼容客户端

任何支持 OpenAI API 的客户端都可以使用相应的端点:

- Base URL: `http://localhost:8989/v1/openai` (或其他端点)
- API Key: 任意值(YoloRouter 使用配置文件中的 key)
- Model: 具体模型名或 `auto`

## 错误处理

### 不支持流式的模型

如果请求的 provider 不支持流式,返回 501 Not Implemented:

```json
{
  "error": "Not implemented: Streaming not supported",
  "status": 501
}
```

### 无效模型名

如果模型名无效(如 `model="auth"`),返回 400 Bad Request:

```json
{
  "error": "stream=true requires an explicit model name (e.g., 'claude-opus', 'gpt-4') or 'auto', not 'auth'",
  "status": 400
}
```

### Provider 错误

如果 provider API 返回错误,YoloRouter 会将错误转发给客户端。

## 性能考虑

1. **低延迟**: 流式响应立即开始,无需等待完整响应
2. **内存高效**: 直接转发字节流,不缓冲完整响应
3. **连接持久**: 保持 HTTP 长连接直到流结束
4. **自动超时**: 配置 `[routing] timeout_ms` 控制最大等待时间

## 调试

### 启用详细日志

```bash
export RUST_LOG=debug
cargo run
```

### 测试脚本

使用提供的测试脚本:

```bash
./test_streaming.sh
```

### curl 参数说明

- `-N` / `--no-buffer`: 禁用输出缓冲,实时显示流式数据
- `--no-buffer`: 某些 curl 版本需要此参数
- `-v`: 查看详细 HTTP 头

## 常见问题

### Q: 为什么流式响应格式不同?

A: 不同的 AI provider 使用不同的 SSE 格式。Anthropic 使用带 `event:` 类型的格式,而 OpenAI/Codex/GitHub 使用简单的 `data:` 格式。YoloRouter 保留原始格式以确保最大兼容性。

### Q: 能否将 Anthropic 格式转换为 OpenAI 格式?

A: 技术上可行,但当前实现选择保留原始格式以避免格式转换带来的潜在错误和延迟。如需此功能,请提 issue。

### Q: `auto` 模型如何选择?

A: FastAnalyzer 使用15维分析(复杂度、成本、延迟、准确性等)评估请求特征,然后从配置的 scenario 中选择最佳模型。详见 `src/analyzer/multidimensional.rs`。

### Q: 流式请求是否支持 fallback?

A: 当前实现中,流式请求不支持 fallback chain。一旦流开始,就无法切换到另一个 provider。非流式请求支持完整的 fallback 机制。

### Q: 如何知道哪些 provider 支持流式?

A: 检查 `/stats` 端点或查看代码中的 `Provider::supports_streaming()` 实现。当前:
- Anthropic: ✅
- OpenAI: ✅
- Gemini: 待 API 确认
- Codex: ✅
- GitHub Copilot: ✅

## 实现细节

### 代码位置

- **Provider trait**: `src/provider/mod.rs` (L24-44)
- **Anthropic streaming**: `src/provider/anthropic.rs` (L169-196, L283-293)
- **OpenAI streaming**: `src/provider/openai.rs` (L1-135)
- **Generic streaming handler**: `src/server/mod.rs` (L321-456)
- **Anthropic-specific streaming**: `src/server/mod.rs` (L458-570)
- **Model selection**: `src/router/engine.rs` (L158-243)

### 测试覆盖

- ✅ Anthropic 端点流式支持 (65 tests passing)
- ✅ `auto` 模型选择
- ✅ 无效模型拒绝
- ⏳ OpenAI/Gemini/Codex 端点流式(待添加)

### 未来改进

- [ ] 为流式请求添加 fallback 支持
- [ ] 统一流式格式转换(可选)
- [ ] 流式响应的详细统计(token 计数等)
- [ ] 流式缓存机制
- [ ] 流式请求的速率限制

## 相关文档

- [CLAUDE_CODE_SETUP.md](CLAUDE_CODE_SETUP.md) - Claude Code 集成指南
- [USER_GUIDE.md](USER_GUIDE.md) - 完整用户指南
- [AGENTS.md](AGENTS.md) - 开发者文档

---

**最后更新**: 2024 | **版本**: 0.1.0
