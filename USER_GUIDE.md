# YoloRouter 用户指南

YoloRouter 是一个智能 AI 模型路由代理，用 Rust 实现。它允许你配置多个 AI 服务商（Anthropic Claude、OpenAI、Google Gemini、GitHub Codex 等），并根据场景自动选择最优模型。支持故障转移、动态配置切换和多种 API 端点。

## 快速开始

### 1. 安装

```bash
git clone https://github.com/sternelee/YoloRouter.git
cd YoloRouter
cargo build --release
```

### 2. 配置

复制示例配置：

```bash
cp config.example.toml config.toml
```

编辑 `config.toml`，设置你的 API 密钥：

```toml
[daemon]
port = 8989
log_level = "info"

[providers.anthropic]
type = "anthropic"
api_key = "${ANTHROPIC_API_KEY}"  # 或直接填写密钥

[providers.openai]
type = "openai"
api_key = "${OPENAI_API_KEY}"

[routing]
fallback_enabled = true
timeout_ms = 30000
retry_count = 2
```

### 3. 设置环境变量

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-..."
```

### 4. 启动服务器

```bash
cargo run --release -- --config config.toml
```

服务器将在 `http://127.0.0.1:8989` 启动。

## 配置详解

### 顶级配置

```toml
[daemon]
port = 8989                    # 服务器监听端口
log_level = "info"            # 日志级别：debug, info, warn, error

[providers]
# 配置你的 AI 服务商

[scenarios]
# 定义路由场景

[routing]
# 路由配置
```

### 提供商配置

所有 `api_key`/`token` 字段支持 `${ENV_VAR}` 环境变量展开。

#### 内置 Provider

##### Anthropic Claude

```toml
[providers.anthropic]
type = "anthropic"
api_key = "${ANTHROPIC_API_KEY}"
```

获取 API 密钥：https://console.anthropic.com/account/keys

##### OpenAI

```toml
[providers.openai]
type = "openai"
api_key = "${OPENAI_API_KEY}"
```

获取 API 密钥：https://platform.openai.com/account/api-keys

##### Google Gemini

```toml
[providers.gemini]
type = "gemini"
api_key = "${GEMINI_API_KEY}"
```

获取 API 密钥：https://makersuite.google.com/app/apikey

##### GitHub Copilot（订阅免费）

```toml
[providers.github_copilot]
type = "github_copilot"
# token 通过 OAuth 自动加载，无需手动填写
# 也可强制指定: token = "${GITHUB_COPILOT_TOKEN}"
```

认证流程：`yolo-router --auth github`（TUI 引导完成设备流 OAuth）

##### ChatGPT Pro / Codex OAuth（订阅免费）

```toml
[providers.codex_oauth]
type = "codex_oauth"
# token 通过 OAuth 自动加载，存储在 ~/.config/yolo-router/codex_oauth.json
```

认证流程：`yolo-router --auth codex`（TUI 引导完成设备流 OAuth）

##### Azure OpenAI

```toml
[providers.azure]
type = "codex"
api_key = "${AZURE_OPENAI_API_KEY}"
[providers.azure.extra]
azure_endpoint = "https://your-resource.openai.azure.com"
api_version = "2024-02-01"
```

---

#### OpenAI 兼容的三方 Provider

任何支持 OpenAI `/v1/chat/completions` 接口的服务均可用 `type = "openai"` + `base_url` 接入：

##### OpenRouter（推荐，100+ 模型含大量免费）

```toml
[providers.openrouter]
type = "openai"
base_url = "https://openrouter.ai/api/v1"
api_key = "${OPENROUTER_API_KEY}"
```

注册：https://openrouter.ai — 免费额度无需信用卡

##### Groq（超快推理，有免费层）

```toml
[providers.groq]
type = "openai"
base_url = "https://api.groq.com/openai/v1"
api_key = "${GROQ_API_KEY}"
```

可用模型：`llama-3.3-70b-versatile`、`mixtral-8x7b-32768`、`gemma2-9b-it`

##### DeepSeek（高性价比，编程/推理强）

```toml
[providers.deepseek]
type = "openai"
base_url = "https://api.deepseek.com/v1"
api_key = "${DEEPSEEK_API_KEY}"
```

可用模型：`deepseek-chat`、`deepseek-coder`、`deepseek-reasoner`

##### Mistral AI

```toml
[providers.mistral]
type = "openai"
base_url = "https://api.mistral.ai/v1"
api_key = "${MISTRAL_API_KEY}"
```

可用模型：`mistral-large-latest`、`mistral-small-latest`、`open-mistral-7b`

##### Together.ai（开源模型托管）

```toml
[providers.together]
type = "openai"
base_url = "https://api.together.xyz/v1"
api_key = "${TOGETHER_API_KEY}"
```

##### Perplexity（联网搜索增强）

```toml
[providers.perplexity]
type = "openai"
base_url = "https://api.perplexity.ai"
api_key = "${PERPLEXITY_API_KEY}"
```

可用模型：`sonar`、`sonar-pro`（含实时联网）

##### 硅基流动 SiliconFlow（国内，免费额度）

```toml
[providers.siliconflow]
type = "openai"
base_url = "https://api.siliconflow.cn/v1"
api_key = "${SILICONFLOW_API_KEY}"
```

可用模型：`Qwen/Qwen2.5-72B-Instruct`、`deepseek-ai/DeepSeek-V3`（部分免费）

##### 月之暗面 Kimi（长上下文，中文优化）

```toml
[providers.kimi]
type = "openai"
base_url = "https://api.moonshot.cn/v1"
api_key = "${MOONSHOT_API_KEY}"
```

可用模型：`moonshot-v1-8k`、`moonshot-v1-32k`、`moonshot-v1-128k`

##### 智谱 GLM

```toml
[providers.zhipu]
type = "openai"
base_url = "https://open.bigmodel.cn/api/paas/v4"
api_key = "${ZHIPU_API_KEY}"
```

可用模型：`glm-4-flash`（免费）、`glm-4-plus`

##### 本地 Ollama（完全离线）

```toml
[providers.ollama]
type = "openai"
base_url = "http://localhost:11434/v1"
api_key = "ollama"   # Ollama 不校验，随意填
```

先安装 Ollama：https://ollama.ai，然后 `ollama pull qwen2.5:7b`

##### 本地 LM Studio

```toml
[providers.lmstudio]
type = "openai"
base_url = "http://localhost:1234/v1"
api_key = "lm-studio"
```

---

#### Provider Type 速查表

| 接口类型                   | `type` 值        | 必填字段                           |
| -------------------------- | ---------------- | ---------------------------------- |
| Anthropic Messages API     | `anthropic`      | `api_key`                          |
| OpenAI / 任何 OAI 兼容     | `openai`         | `api_key` + `base_url`（官方可省） |
| Google Gemini              | `gemini`         | `api_key`                          |
| GitHub Copilot（Pro 订阅） | `github_copilot` | 无（OAuth 后自动加载）             |
| ChatGPT Pro（Pro 订阅）    | `codex_oauth`    | 无（OAuth 后自动加载）             |
| Azure OpenAI               | `codex`          | `api_key` + `extra.azure_endpoint` |
| 其他任意兼容 API           | 任意名称         | `api_key` + `base_url`             |

### 场景定义

场景允许你为不同的任务类型配置不同的模型链：

```toml
[scenarios.production_coding]
# 高质量代码生成：优先使用昂贵但强大的模型，失败时转移
models = [
  { provider = "anthropic", model = "claude-opus", cost_tier = "high" },
  { provider = "openai", model = "gpt-4", cost_tier = "high" },
  { provider = "anthropic", model = "claude-sonnet", cost_tier = "medium" },
  { provider = "openai", model = "gpt-3.5-turbo", cost_tier = "low" }
]
default_tier = "high"

[scenarios.quick_task]
# 快速任务：使用便宜的模型
models = [
  { provider = "openai", model = "gpt-3.5-turbo", cost_tier = "low" },
  { provider = "gemini", model = "gemini-pro", cost_tier = "low" }
]
default_tier = "low"
```

### 路由配置

```toml
[routing]
fallback_enabled = true       # 启用故障转移
timeout_ms = 30000           # 请求超时（毫秒）
retry_count = 2               # 失败重试次数
```

## API 端点

### 协议适配端点（Protocol Adapters）

端点名决定**请求/响应格式**，不决定使用哪个 provider。实际 provider 由路由引擎选择。

| 端点                               | 适配格式                | 适用客户端          |
| ---------------------------------- | ----------------------- | ------------------- |
| `POST /v1/anthropic`               | Anthropic Messages API  | Claude Code, Cursor |
| `POST /v1/anthropic/v1/messages`   | 同上（完整路径）        | 同上                |
| `POST /v1/openai`                  | OpenAI Chat Completions | OpenAI SDK          |
| `POST /v1/openai/chat/completions` | 同上（完整路径）        | 同上                |
| `POST /v1/codex`                   | OpenAI 格式             | Codex CLI           |
| `POST /v1/codex/chat/completions`  | 同上（完整路径）        | 同上                |
| `POST /v1/gemini`                  | OpenAI 兼容格式         | Gemini 客户端       |
| `POST /v1/auto`                    | OpenAI 格式             | 通用，15 维自动路由 |

### 管理端点

```
GET  /health                         健康检查
GET  /config                         查看当前配置
GET  /stats                          请求统计
GET  /control/status                 当前路由覆盖状态
POST /control/override               设置路由覆盖
DELETE /control/override/{endpoint}  清除覆盖
```

**路由覆盖示例：**

```bash
# 全局固定到 coding 场景
curl -X POST http://127.0.0.1:8989/control/override \
  -H "Content-Type: application/json" \
  -d '{"endpoint":"global","scenario":"coding"}'

# 只将 anthropic 端点固定到 reasoning 场景
curl -X POST http://127.0.0.1:8989/control/override \
  -H "Content-Type: application/json" \
  -d '{"endpoint":"anthropic","scenario":"reasoning"}'

# 恢复自动路由
curl -X DELETE http://127.0.0.1:8989/control/override/global
```

也可在 TUI 的 Scenarios 标签页按 `Enter` 固定场景，按 `a` 恢复自动。

## 请求格式

所有端点都接受 JSON 格式的请求：

```json
{
  "model": "claude-opus",
  "messages": [
    {
      "role": "user",
      "content": "Hello, how can you help?"
    }
  ],
  "max_tokens": 1000,
  "temperature": 0.7,
  "top_p": null
}
```

**字段说明：**

- `model` (必需) - 模型名称
- `messages` (必需) - 消息列表，每个包含 `role` 和 `content`
- `max_tokens` (可选) - 最大输出 token 数
- `temperature` (可选) - 温度 (0-1)，越高越随机
- `top_p` (可选) - 核采样概率

## 响应格式

成功响应：

```json
{
  "message": {
    "role": "assistant",
    "content": "Response text..."
  },
  "usage": {
    "input_tokens": 10,
    "output_tokens": 20,
    "total_tokens": 30
  }
}
```

错误响应：

```json
{
  "error": "Error message",
  "status": 503
}
```

## 使用示例

### 使用 curl

```bash
curl -X POST http://127.0.0.1:8989/v1/anthropic \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-opus",
    "messages": [{"role": "user", "content": "Say hello!"}],
    "max_tokens": 100
  }'
```

### 使用 Python

```python
import requests

response = requests.post(
    "http://127.0.0.1:8989/v1/auto",
    json={
        "model": "claude-opus",
        "messages": [{"role": "user", "content": "Say hello!"}],
        "max_tokens": 100
    }
)

print(response.json())
```

### 使用 JavaScript

```javascript
const response = await fetch("http://127.0.0.1:8989/v1/openai", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({
    model: "gpt-4",
    messages: [{ role: "user", content: "Say hello!" }],
    max_tokens: 100,
  }),
});

console.log(await response.json());
```

## 故障转移和重试

当启用 `fallback_enabled = true` 时，YoloRouter 会：

1. 尝试场景中的第一个模型
2. 如果失败，转移到下一个模型
3. 最多重试 `retry_count` 次
4. 如果所有模型都失败，返回错误

例如，对于场景配置：

```toml
[scenarios.coding]
models = [
  { provider = "anthropic", model = "claude-opus" },
  { provider = "openai", model = "gpt-4" },
  { provider = "gemini", model = "gemini-pro" }
]
```

如果 claude-opus 超时，系统会自动尝试 gpt-4，然后是 gemini-pro。

## 监控和调试

### 健康检查

```bash
curl http://127.0.0.1:8989/health
```

响应示例：

```json
{
  "status": "healthy",
  "service": "yolo-router",
  "version": "0.1.0",
  "providers": ["anthropic", "openai", "gemini"],
  "scenarios": ["production_coding", "quick_task"]
}
```

### 查看配置

```bash
curl http://127.0.0.1:8989/config
```

### 请求统计

```bash
curl http://127.0.0.1:8989/stats
```

响应示例：

```json
{
  "total_requests": 150,
  "total_successes": 145,
  "total_errors": 5,
  "average_response_time_ms": 1200.5,
  "providers_called": {
    "anthropic": 80,
    "openai": 55,
    "gemini": 15
  }
}
```

## 最佳实践

### 1. 环境变量管理

使用环境变量存储敏感信息：

```toml
[providers.anthropic]
api_key = "${ANTHROPIC_API_KEY}"
```

不要将实际的 API 密钥提交到版本控制系统。

### 2. 成本优化

为不同的任务创建不同的场景：

```toml
[scenarios.complex_analysis]
models = [
  { provider = "anthropic", model = "claude-opus", cost_tier = "high" }
]

[scenarios.simple_task]
models = [
  { provider = "openai", model = "gpt-3.5-turbo", cost_tier = "low" }
]
```

### 3. 可靠性

配置故障转移链确保服务持续可用：

```toml
[routing]
fallback_enabled = true
retry_count = 3
timeout_ms = 60000
```

### 4. 监控

定期检查 `/stats` 端点以了解：

- 哪些提供商被使用最频繁
- 平均响应时间
- 错误率

## 故障排除

### 问题：认证错误

**症状**：请求返回 401/403 错误

**解决方案**：

1. 验证 API 密钥正确
2. 检查环境变量设置
3. 确认提供商的认证方式（OAuth vs API Key）

### 问题：超时

**症状**：请求超过 30 秒无响应

**解决方案**：

1. 增加 `timeout_ms` 值
2. 检查网络连接
3. 验证提供商服务状态

### 问题：故障转移不工作

**症状**：第一个模型失败，没有尝试备用模型

**解决方案**：

1. 检查 `fallback_enabled = true`
2. 验证场景配置有多个模型
3. 检查所有引用的提供商都已配置

### 问题：找不到模型

**症状**：返回 "provider not found" 错误

**解决方案**：

1. 检查提供商名称拼写
2. 验证提供商已在 `[providers]` 部分配置
3. 确认 API 密钥有效

## 高级配置

### 自定义提供商

YoloRouter 支持通用提供商类型，可用于集成自定义 API：

```toml
[providers.custom]
type = "generic"
base_url = "https://api.example.com"
api_key = "${CUSTOM_API_KEY}"
```

### 多区域部署

为不同地域配置提供商：

```toml
[scenarios.us_region]
models = [
  { provider = "openai_us", model = "gpt-4" },
  { provider = "anthropic_us", model = "claude-opus" }
]

[scenarios.eu_region]
models = [
  { provider = "openai_eu", model = "gpt-4" },
  { provider = "anthropic_eu", model = "claude-opus" }
]
```

## 更新和维护

### 检查日志

日志级别配置：

```toml
[daemon]
log_level = "debug"  # 更详细的日志
```

### 备份配置

定期备份你的 `config.toml` 文件：

```bash
cp config.toml config.toml.backup
```

## 支持和反馈

- 开源仓库：https://github.com/sternelee/YoloRouter
- 问题报告：提交 GitHub Issue
- 讨论：参与 GitHub Discussions

## 许可证

MIT License - 详见项目仓库
