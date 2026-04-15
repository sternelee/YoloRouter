# 流式请求故障排查指南

## 常见错误

### 1. `stream error: request parse error: invalid Header provided`

**错误日志示例**:
```
2026-04-15T08:26:37.236595Z ERROR actix_http::h1::dispatcher: stream error: request parse error: invalid Header provided
```

**可能原因**:

1. **客户端发送了格式不正确的 HTTP 头**
   - 某些 HTTP 客户端可能发送不符合 RFC 规范的头
   - 特殊字符或换行符在头值中

2. **请求体过大或格式错误**
   - 超过服务器限制的请求体大小
   - JSON 格式错误(缺少引号、逗号等)

3. **连接中断**
   - 客户端在发送请求过程中断开
   - 网络问题导致的不完整请求

4. **HTTP/2 降级问题**
   - 客户端尝试 HTTP/2 但服务器只支持 HTTP/1.1

**解决方案**:

#### A. 启用详细日志

```bash
# 启动 YoloRouter 时设置详细日志级别
export RUST_LOG=debug
cargo run --release

# 或者使用 trace 级别查看更多细节
export RUST_LOG=trace
cargo run --release
```

#### B. 检查客户端配置

**Claude Code**:
确保配置正确:
```json
{
  "ANTHROPIC_BASE_URL": "http://127.0.0.1:8989/v1/anthropic",
  "ANTHROPIC_MODEL": "auto",
  "ANTHROPIC_AUTH_TOKEN": "dummy"
}
```

**cURL**:
使用 `-v` 查看详细的头信息:
```bash
curl -v -X POST http://localhost:8989/v1/anthropic \
  -H "Content-Type: application/json" \
  -d '{
    "model": "auto",
    "messages": [{"role": "user", "content": "Hello"}],
    "stream": true
  }' -N
```

#### C. 验证请求格式

确保请求 JSON 格式正确:
```bash
# 使用 jq 验证 JSON
echo '{
  "model": "auto",
  "messages": [{"role": "user", "content": "Hello"}],
  "stream": true
}' | jq .
```

#### D. 测试非流式请求

先测试非流式请求,确认服务器和客户端基本通信正常:
```bash
curl -X POST http://localhost:8989/v1/anthropic \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-opus",
    "messages": [{"role": "user", "content": "Hello"}],
    "stream": false,
    "max_tokens": 100
  }'
```

#### E. 检查防火墙和代理

某些防火墙或代理可能会干扰流式连接:
```bash
# 直接测试本地连接
curl http://127.0.0.1:8989/health

# 检查端口是否被占用
lsof -i :8989

# 检查服务器是否在监听
netstat -an | grep 8989
```

#### F. 更新 YoloRouter

确保使用最新版本:
```bash
cd /path/to/YoloRouter
git pull
cargo build --release
```

### 2. `Provider does not support streaming`

**错误响应**:
```json
{
  "error": {
    "message": "Provider 'gemini' does not support streaming",
    "type": "invalid_request_error"
  }
}
```

**解决方案**:

当前支持流式的 providers:
- ✅ Anthropic
- ✅ OpenAI
- ✅ Codex (ChatGPT Pro)
- ✅ GitHub Copilot
- ⏳ Gemini (待实现)

如果需要使用不支持流式的 provider,设置 `stream: false` 或省略此字段。

### 3. `Failed to auto-select model for streaming`

**错误响应**:
```json
{
  "error": {
    "message": "Failed to auto-select model for streaming: No providers available",
    "type": "api_error"
  }
}
```

**可能原因**:
- 配置文件中没有可用的 provider
- 所有 provider 的 API key 无效
- Scenario 配置错误

**解决方案**:

检查配置文件:
```bash
# 查看当前配置
curl http://localhost:8989/config

# 检查 scenarios 配置
cat config.toml | grep -A 5 "\[scenarios"

# 验证 provider 配置
cat config.toml | grep -A 3 "\[providers"
```

确保至少有一个有效的 provider:
```toml
[providers.anthropic]
type = "anthropic"
api_key = "${ANTHROPIC_API_KEY}"

[scenarios.production]
models = [
  { provider = "anthropic", model = "claude-opus", cost_tier = "high" }
]
```

## 调试技巧

### 查看实时日志

```bash
# 在一个终端启动服务器
export RUST_LOG=debug
cargo run

# 在另一个终端发送测试请求
./test_streaming.sh
```

### 使用 tcpdump 抓包

```bash
# 抓取本地回环接口的流量
sudo tcpdump -i lo0 -A 'port 8989'

# 发送测试请求,观察 HTTP 流量
```

### 检查服务器状态

```bash
# 健康检查
curl http://localhost:8989/health

# 统计信息
curl http://localhost:8989/stats

# 配置信息
curl http://localhost:8989/config
```

### 模拟客户端请求

使用 Python 测试流式请求:
```python
import requests
import json

url = "http://localhost:8989/v1/anthropic"
data = {
    "model": "auto",
    "messages": [{"role": "user", "content": "Hello"}],
    "stream": True,
    "max_tokens": 100
}

response = requests.post(url, json=data, stream=True)
for line in response.iter_lines():
    if line:
        print(line.decode('utf-8'))
```

## 性能问题

### 流式响应缓慢

**可能原因**:
- Provider API 延迟
- 网络问题
- 服务器资源不足

**解决方案**:
```bash
# 检查系统资源
top
df -h

# 检查网络延迟
ping api.anthropic.com
ping api.openai.com

# 增加超时时间
# 编辑 config.toml:
[routing]
timeout_ms = 60000  # 增加到 60 秒
```

### 连接超时

**配置调整**:
```toml
[daemon]
port = 8989
log_level = "info"
keep_alive = 75  # 增加 keep-alive 时间(秒)

[routing]
timeout_ms = 30000
retry_count = 2
```

## 已知问题

### Issue #1: HTTP header parsing error

**状态**: 已修复 (v0.1.0+)

**问题**: 使用 `header::CONNECTION` 常量可能导致某些环境下的头解析错误

**修复**: 改用字符串 `"Connection"` 设置响应头

**受影响版本**: < 0.1.0

**修复方法**: 更新到最新版本

### Issue #2: 流式请求不支持 fallback

**状态**: 已知限制

**问题**: 流式请求一旦开始,无法切换到备用 provider

**原因**: HTTP 响应头已发送,无法更改

**解决方案**: 使用非流式请求配合 fallback chain,或确保主 provider 稳定可用

## 报告问题

如果遇到无法解决的问题,请提供以下信息:

1. **YoloRouter 版本**:
   ```bash
   cargo run -- --version
   # 或查看 Cargo.toml
   ```

2. **完整错误日志**:
   ```bash
   export RUST_LOG=debug
   cargo run 2>&1 | tee yolo-router.log
   ```

3. **配置文件** (删除敏感信息):
   ```bash
   cat config.toml | sed 's/api_key = .*/api_key = "***"/'
   ```

4. **测试请求**:
   ```bash
   curl -v -X POST ... 2>&1 | tee request.log
   ```

5. **系统信息**:
   ```bash
   uname -a
   rustc --version
   cargo --version
   ```

---

**最后更新**: 2024  
**文档版本**: 1.0
