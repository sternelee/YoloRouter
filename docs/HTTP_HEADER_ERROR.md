# HTTP Header 错误诊断指南

## 问题: `invalid Header provided`

当您看到这个错误时:
```
ERROR actix_http::h1::dispatcher: stream error: request parse error: invalid Header provided
```

这意味着 **客户端发送的 HTTP 请求**有问题，而不是服务器的响应。

## 已实施的修复

### 1. 增加 JSON payload 大小限制

**问题**: 默认 payload 限制可能太小(通常是 256KB)
**修复**: 增加到 10MB

```rust
.limit(10 * 1024 * 1024) // 10MB
```

**文件**: `src/server/mod.rs`

### 2. 更好的错误处理

添加了详细的 JSON 解析错误日志，这样您可以看到具体是什么问题。

## 诊断步骤

### Step 1: 启用详细日志

```bash
export RUST_LOG=debug,actix_web=debug,actix_http=debug
cargo run --release
```

这会显示:
- 所有传入的请求
- JSON 解析错误的详细信息
- HTTP header 解析问题

### Step 2: 使用 cURL 测试

**基本测试(非流式)**:
```bash
curl -v -X POST http://127.0.0.1:8989/v1/anthropic \
  -H "Content-Type: application/json" \
  -H "Accept: application/json" \
  -d '{
    "model": "claude-opus",
    "messages": [{"role": "user", "content": "Hello"}],
    "max_tokens": 100
  }'
```

**流式测试**:
```bash
curl -v -X POST http://127.0.0.1:8989/v1/anthropic \
  -H "Content-Type: application/json" \
  -H "Accept: text/event-stream" \
  -d '{
    "model": "auto",
    "messages": [{"role": "user", "content": "Hello"}],
    "stream": true,
    "max_tokens": 100
  }' -N --no-buffer
```

如果 cURL 工作正常，问题可能在于 Claude Code 的请求格式。

### Step 3: 检查 Claude Code 配置

确保配置格式正确:

```json
{
  "ANTHROPIC_BASE_URL": "http://127.0.0.1:8989/v1/anthropic",
  "ANTHROPIC_MODEL": "auto",
  "ANTHROPIC_AUTH_TOKEN": "dummy-key"
}
```

**常见问题**:
- ✅ 使用 `http://127.0.0.1` 而不是 `http://localhost` (某些系统 IPv6 问题)
- ✅ 端口号正确 (8989)
- ✅ 路径是 `/v1/anthropic` 而不是 `/v1`
- ✅ 不要在 URL 末尾加斜杠

### Step 4: 检查是否有其他进程占用端口

```bash
# macOS/Linux
lsof -i :8989

# 如果有旧进程，杀掉它
kill -9 <PID>
```

### Step 5: 使用 tcpdump 抓包(高级)

```bash
# 在一个终端
sudo tcpdump -i lo0 -A -s 0 'port 8989' | tee http-traffic.log

# 在另一个终端发送请求
# 检查 http-traffic.log 查看实际的 HTTP 数据
```

查找可疑的 header，比如:
- 非 ASCII 字符
- 格式错误的 header 值
- 缺少的 Content-Length
- 错误的 Transfer-Encoding

## 可能的原因

### 1. Claude Code 发送了特殊 header

某些客户端可能发送非标准的 HTTP header。

**检查方法**: 查看 debug 日志中的 "Incoming request" 消息。

### 2. 请求体太大

**已修复**: 增加到 10MB 限制。

### 3. 连接被代理或防火墙干扰

**解决方案**: 
- 禁用 VPN
- 禁用防火墙
- 使用 127.0.0.1 而不是 localhost

### 4. HTTP/2 vs HTTP/1.1 问题

**解决方案**: YoloRouter 只支持 HTTP/1.1，确保客户端不尝试 HTTP/2。

在 Claude Code 中，这通常不是问题，因为它使用标准的 HTTP/1.1。

## 如果问题持续

### Option 1: 使用 Python 脚本测试

创建 `test_client.py`:
```python
import requests
import json

url = "http://127.0.0.1:8989/v1/anthropic"
headers = {
    "Content-Type": "application/json",
}
data = {
    "model": "auto",
    "messages": [{"role": "user", "content": "Hello, world!"}],
    "stream": False,
    "max_tokens": 100
}

response = requests.post(url, headers=headers, json=data)
print("Status:", response.status_code)
print("Response:", response.json())
```

运行:
```bash
python3 test_client.py
```

### Option 2: 降级到非流式模式

在 Claude Code 中，临时禁用流式:
- 这不是官方设置
- 但可以验证是否是流式特定的问题

### Option 3: 检查 Claude Code 版本

确保您使用的是最新版本的 Claude Code。

### Option 4: 直接查看 actix-web 日志

错误发生在 actix-http 的 H1 dispatcher 中，这意味着在解析 HTTP/1.1 请求时失败。

最可能的原因:
1. Header 行太长
2. Header 值包含非法字符
3. Header 格式不符合 RFC 7230

## 临时解决方案

如果问题只在 Claude Code 中出现，您可以:

1. **使用非流式模式** (如果 Claude Code 支持配置)
2. **使用其他客户端**测试 YoloRouter
3. **报告给 Claude Code 团队**，可能是他们发送了不规范的 header

## 成功的测试

如果使用 cURL 测试成功:
```bash
curl -X POST http://127.0.0.1:8989/v1/anthropic \
  -H "Content-Type: application/json" \
  -d '{"model":"auto","messages":[{"role":"user","content":"test"}],"stream":true}' \
  -N
```

那么 YoloRouter 工作正常，问题在于 Claude Code 的请求格式。

## 收集诊断信息

如果需要进一步帮助，请提供:

1. **完整日志** (使用 `RUST_LOG=debug,actix_web=debug`):
   ```bash
   export RUST_LOG=debug,actix_web=debug
   cargo run --release 2>&1 | tee yolo-debug.log
   ```

2. **cURL 测试结果**

3. **Claude Code 版本和配置**

4. **tcpdump 抓包** (如果可能):
   ```bash
   sudo tcpdump -i lo0 -s 0 -A 'port 8989' -w capture.pcap
   # 然后发送请求
   # 分析: tcpdump -A -r capture.pcap
   ```

---

**更新时间**: 2024-04-15
**状态**: 已实施 payload 限制增加和错误处理改进
