# YoloRouter 🚀

> Intelligent AI Model Routing Proxy - A high-performance, flexible multi-vendor AI model routing system built in Rust

[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/Tests-54%2F54-brightgreen.svg)]()
[![Build](https://img.shields.io/badge/Build-Passing-brightgreen.svg)]()
[![Analyzer](https://img.shields.io/badge/Analyzer-%3C1ms-brightgreen.svg)]()

## Overview

YoloRouter is a powerful AI model routing proxy that allows you to:

- 🔀 **Intelligently route** requests across multiple AI vendors
- 🛡️ Ensure **high availability** through **fallback chains**
- ⚙️ Manage model selection easily with **flexible TOML configuration** (no code changes needed)
- 📊 **Monitor in real-time** request statistics and performance metrics
- 💰 **Optimize costs** through scenario-based model configuration
- 🎯 **Auto-detect scenarios** and intelligently select the best model

### Supported AI Vendors

**Native Support (Built-in Authentication):**

- **Anthropic Claude** — claude-opus, claude-sonnet, claude-haiku
- **OpenAI** — gpt-4o, gpt-4, gpt-3.5-turbo, etc.
- **Google Gemini** — gemini-2.0-flash, gemini-pro, etc.
- **GitHub Copilot** — OAuth device flow auth, free with Copilot Pro subscription
- **ChatGPT Pro (Codex OAuth)** — OAuth device flow auth, free with ChatGPT Pro subscription
- **Azure OpenAI** — Enterprise deployment support

**OpenAI Compatible (All services supporting `/v1/chat/completions`):**

- **OpenRouter** — Unified access to 100+ models, many free
- **Groq** — Ultra-fast inference (LLaMA, Mixtral)
- **DeepSeek** — Cost-effective programming/reasoning models
- **Mistral AI** — European open-source models
- **Together.ai** — Open-source model hosting
- **Perplexity** — Web-search-enhanced models
- **SiliconFlow** — Domestic access, free credits
- **Kimi (Moon Dark Side)** — Long-context Chinese models
- **Zhipu GLM** — Chinese large language models
- **Ollama** — Local model inference (fully offline)
- **LM Studio** — Local model GUI
- **Any OpenAI-compatible API** — Generic `openai` type + `base_url`

## Quick Start

### 1️⃣ Installation

#### Method 1: Automated Installation Script (Recommended)

```bash
# Clone the repository
git clone https://github.com/sternelee/YoloRouter.git
cd YoloRouter

# One-click install (supports macOS, Linux, Windows)
bash install.sh
```

> **New user?** See [QUICK_INSTALL.txt](QUICK_INSTALL.txt) for a 5-minute quick start guide

#### Method 2: Manual Build

```bash
# Clone the repository
git clone https://github.com/sternelee/YoloRouter.git
cd YoloRouter

# Build
cargo build --release

# Run
./target/release/yolo-router --config config.toml
```

### 3️⃣ Configure and Set Up Environment Variables

#### Automatic Configuration (Recommended)

Run the interactive configuration wizard:

```bash
bash yolo-setup.sh
```

This script will:
- Help you edit the `config.toml` file
- Set up necessary environment variables
- Run health checks
- Test API connections

#### Manual Configuration

```bash
# Copy example configuration
cp config.example.toml config.toml

# Edit config.toml and add your API keys
nano config.toml
```

### 4️⃣ Set Environment Variables

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-..."
```

See [INSTALL.md](INSTALL.md) for complete environment variable setup instructions.

### 5️⃣ Start the Server

```bash
# Using cargo
cargo run --release -- --config config.toml

# Or run the binary directly
./target/release/yolo-router --config config.toml
```

The server will start at `http://127.0.0.1:8989`.

### 6️⃣ Verify Installation

```bash
# Check health status
curl http://127.0.0.1:8989/health

# View statistics
curl http://127.0.0.1:8989/stats
```

## Tools and Scripts

### 📦 Installation Scripts

| Script | Function | Use Case |
|--------|----------|----------|
| **install.sh** | One-click installation (multi-platform) | First-time install, auto-check dependencies and compile |
| **uninstall.sh** | Safe uninstall | Completely remove YoloRouter (preserves config) |
| **yolo-setup.sh** | Interactive configuration wizard | Initial setup, reconfigure environment variables, test connections |

### Usage Examples

```bash
# Install
bash install.sh

# Configure
bash yolo-setup.sh

# Uninstall
bash uninstall.sh
```

See [INSTALL.md](INSTALL.md) for complete instructions.

## Sending Requests

Basic configuration example:

```toml
[daemon]
port = 8989
log_level = "info"

[providers.anthropic]
type = "anthropic"
api_key = "${ANTHROPIC_API_KEY}"

[providers.openai]
type = "openai"
api_key = "${OPENAI_API_KEY}"

[scenarios.production]
models = [
  { provider = "anthropic", model = "claude-opus", cost_tier = "high" },
  { provider = "openai", model = "gpt-4", cost_tier = "high" }
]

[routing]
fallback_enabled = true
timeout_ms = 30000
```

### API Request Examples

```bash
curl -X POST http://127.0.0.1:8989/v1/anthropic \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-opus",
    "messages": [{"role": "user", "content": "Hello!"}],
    "max_tokens": 100
  }'
```

## Core Features

### 🔀 Intelligent Routing

Define scenarios to select different models for different tasks:

```toml
[scenarios.high_quality_coding]
models = [
  { provider = "anthropic", model = "claude-opus", cost_tier = "high" },
  { provider = "openai", model = "gpt-4", cost_tier = "high" },
  { provider = "anthropic", model = "claude-sonnet", cost_tier = "medium" }
]

[scenarios.quick_task]
models = [
  { provider = "openai", model = "gpt-3.5-turbo", cost_tier = "low" }
]
```

### 🛡️ Fallback Failover

Automatically switch to the next model when a request fails:

```
Request → claude-opus (failed)
       → gpt-4 (failed)
       → claude-sonnet (success) ✅
```

Configuration:

```toml
[routing]
fallback_enabled = true    # Enable failover
retry_count = 2            # Retry 2 times per model
timeout_ms = 30000         # 30 second timeout
```

### ⚙️ Flexible Configuration

- Environment variable support: `${VARIABLE_NAME}`
- Dynamic validation: Automatic configuration integrity checking
- Hot query: Read new config without restart

### 🧠 15-Dimensional Intelligent Analysis

YoloRouter includes FastAnalyzer that analyzes requests in **< 1ms** across 15 dimensions to automatically select the optimal model:

1. **Request Complexity** - Token count and structural complexity
2. **Cost Importance** - User budget constraints
3. **Latency Requirements** - SLA urgency
4. **Accuracy Needs** - Output quality importance
5. **Throughput Requirements** - QPS limits
6. **Cost Budget** - Monthly budget remaining
7. **Model Availability** - Service health
8. **Cache Hit Rate** - Historical cache hit ratio
9. **Geographic Constraints** - Location compliance
10. **Privacy Level** - Data sensitivity
11. **Feature Requirements** - Special capabilities (vision, tools)
12. **Reliability** - SLA and failover requirements
13. **Reasoning Ability** - Complex reasoning task needs
14. **Programming Ability** - Code generation needs
15. **General Knowledge** - Knowledge-intensive task needs

**Advantage**: Compared to hardcoded routing, dynamic model selection can save **40% cost** while improving response quality.

### 📊 Monitoring and Statistics

```bash
# View request statistics
curl http://127.0.0.1:8989/stats

# Example response
{
  "total_requests": 150,
  "total_successes": 145,
  "total_errors": 5,
  "average_response_time_ms": 1250.5,
  "providers_called": {
    "anthropic": 80,
    "openai": 55,
    "gemini": 15
  }
}
```

### API Endpoints

### Protocol Adapter Endpoints

These endpoints accept native request formats from different AI clients, with routing decisions handled uniformly by the routing engine:

| Endpoint | Format | Applicable Clients |
|----------|--------|-------------------|
| `POST /v1/anthropic` | Anthropic Messages API | Claude Code, Cursor |
| `POST /v1/anthropic/v1/messages` | Same (full path) | Same |
| `POST /v1/openai` | OpenAI Chat Completions | OpenAI SDK, ChatGPT clients |
| `POST /v1/openai/chat/completions` | Same (full path) | Same |
| `POST /v1/codex` | OpenAI format | Codex CLI |
| `POST /v1/codex/chat/completions` | Same (full path) | Same |
| `POST /v1/gemini` | OpenAI-compatible format | Gemini clients |
| `POST /v1/auto` | OpenAI format | Generic, 15D auto-routing |

> **Note**: The endpoint name determines the **protocol format**, not the target provider. Which provider/model is actually used depends on the routing engine (scenario matching or TUI override).

### Management Endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /health` | Health check |
| `GET /config` | View current configuration |
| `GET /stats` | View statistics |
| `GET /control/status` | Current routing override status |
| `POST /control/override` | Set routing override (see below) |
| `DELETE /control/override/{ep}` | Clear override, restore auto-routing |

**Setting routing override:**

```bash
# Route all requests to coding scenario
curl -X POST http://127.0.0.1:8989/control/override \
  -H "Content-Type: application/json" \
  -d '{"endpoint":"global","scenario":"coding"}'

# Only route anthropic endpoint to reasoning scenario
curl -X POST http://127.0.0.1:8989/control/override \
  -H "Content-Type: application/json" \
  -d '{"endpoint":"anthropic","scenario":"reasoning"}'

# Restore auto-routing
curl -X DELETE http://127.0.0.1:8989/control/override/global
```

### Request Format

All endpoints accept the same JSON format:

```json
{
  "model": "claude-opus",
  "messages": [
    {
      "role": "user",
      "content": "Your prompt"
    }
  ],
  "max_tokens": 1000,
  "temperature": 0.7,
  "top_p": null
}
```

### Response Format

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

## Usage Examples

### Python

```python
import requests

response = requests.post(
    "http://127.0.0.1:8989/v1/auto",
    json={
        "model": "claude-opus",
        "messages": [{"role": "user", "content": "Hello!"}],
        "max_tokens": 100
    }
)

print(response.json())
```

### JavaScript

```javascript
const response = await fetch("http://127.0.0.1:8989/v1/openai", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({
    model: "gpt-4",
    messages: [{ role: "user", content: "Hello!" }],
    max_tokens: 100,
  }),
});

console.log(await response.json());
```

### cURL

```bash
curl -X POST http://127.0.0.1:8989/v1/auto \
  -H "Content-Type: application/json" \
  -d '{"model":"claude-opus","messages":[{"role":"user","content":"Say hello!"}],"max_tokens":100}'
```

## Project Structure

```
YoloRouter/
├── src/
│   ├── lib.rs                 # Library root
│   ├── main.rs                # Application entry
│   ├── error.rs               # Error handling
│   ├── models.rs              # Data structures
│   ├── config/                # Configuration system
│   ├── provider/              # Provider implementations
│   ├── router/                # Routing engine
│   ├── server/                # HTTP server
│   ├── tui/                   # TUI authentication
│   └── utils/                 # Utility functions
├── tests/                     # Integration tests
├── config.example.toml        # Configuration example
├── Cargo.toml                 # Project manifest
├── USER_GUIDE.md              # User guide
├── PROJECT_SUMMARY.md         # Project summary
└── README.md                  # This file
```

## Documentation

### 👤 User Documentation

- **[00-START-HERE.md](00-START-HERE.md)** - Documentation navigation guide (recommendations by role)
- **[QUICK_INSTALL.txt](QUICK_INSTALL.txt)** - 5-minute quick start guide
- **[INSTALL.md](INSTALL.md)** - Detailed installation guide (macOS, Linux, Windows)
- **[USER_GUIDE.md](USER_GUIDE.md)** - Complete user guide (configuration, API usage, troubleshooting)
- **[README_cn.md](README_cn.md)** - Chinese version of README

### 🔧 Developer and Operations Documentation

- **[RELEASE_GUIDE.md](RELEASE_GUIDE.md)** - Release process guide (with automation workflow)
- **[CI_CD_GUIDE.md](CI_CD_GUIDE.md)** - CI/CD quick reference
- **[PROJECT_SUMMARY.md](PROJECT_SUMMARY.md)** - Project architecture summary and technology choices
- **[.github/copilot-instructions.md](.github/copilot-instructions.md)** - Developer guide
- **[.github/copilot-skill-yoloprouter.md](.github/copilot-skill-yoloprouter.md)** - Copilot Skill collaboration guide

### 📋 Configuration Reference

- **[config.example.toml](config.example.toml)** - Configuration examples (all vendors and scenarios)

### 🔄 GitHub Actions Workflows

| Workflow | File | Function |
|----------|------|----------|
| **Release** | `.github/workflows/release.yml` | Multi-platform builds, release creation, docs deployment |
| **Continuous Integration** | `.github/workflows/ci.yml` | Code quality, tests, security scanning |
| **Development Build** | `.github/workflows/build.yml` | Cross-platform builds for dev branches |
| **Validation** | `.github/workflows/validate.yml` | Workflow file syntax validation |

## Configuration Guide

### Provider Configuration

All providers support `${ENV_VAR}` environment variable expansion.

#### Built-in Providers

```toml
# Anthropic
[providers.anthropic]
type = "anthropic"
api_key = "${ANTHROPIC_API_KEY}"

# OpenAI
[providers.openai]
type = "openai"
api_key = "${OPENAI_API_KEY}"

# Google Gemini
[providers.gemini]
type = "gemini"
api_key = "${GEMINI_API_KEY}"

# GitHub Copilot (token auto-loaded after OAuth)
# First run: yolo-router --auth github
[providers.github_copilot]
type = "github_copilot"

# ChatGPT Pro / Codex OAuth (token auto-loaded after OAuth)
# First run: yolo-router --auth codex
[providers.codex_oauth]
type = "codex_oauth"

# Azure OpenAI
[providers.azure]
type = "codex"
api_key = "${AZURE_OPENAI_API_KEY}"
[providers.azure.extra]
azure_endpoint = "https://your-resource.openai.azure.com"
api_version = "2024-02-01"
```

#### OpenAI-Compatible Third-Party Providers

Any service supporting OpenAI's `/v1/chat/completions` interface can be configured with `type = "openai"` + `base_url`:

```toml
# OpenRouter (100+ models, many free)
[providers.openrouter]
type = "openai"
base_url = "https://openrouter.ai/api/v1"
api_key = "${OPENROUTER_API_KEY}"

# Groq (ultra-fast inference)
[providers.groq]
type = "openai"
base_url = "https://api.groq.com/openai/v1"
api_key = "${GROQ_API_KEY}"

# DeepSeek (cost-effective programming/reasoning)
[providers.deepseek]
type = "openai"
base_url = "https://api.deepseek.com/v1"
api_key = "${DEEPSEEK_API_KEY}"

# Mistral AI
[providers.mistral]
type = "openai"
base_url = "https://api.mistral.ai/v1"
api_key = "${MISTRAL_API_KEY}"

# Together.ai
[providers.together]
type = "openai"
base_url = "https://api.together.xyz/v1"
api_key = "${TOGETHER_API_KEY}"

# Perplexity (web search)
[providers.perplexity]
type = "openai"
base_url = "https://api.perplexity.ai"
api_key = "${PERPLEXITY_API_KEY}"

# SiliconFlow (domestic, free credits)
[providers.siliconflow]
type = "openai"
base_url = "https://api.siliconflow.cn/v1"
api_key = "${SILICONFLOW_API_KEY}"

# Kimi (long-context Chinese)
[providers.kimi]
type = "openai"
base_url = "https://api.moonshot.cn/v1"
api_key = "${MOONSHOT_API_KEY}"

# Zhipu GLM
[providers.zhipu]
type = "openai"
base_url = "https://open.bigmodel.cn/api/paas/v4"
api_key = "${ZHIPU_API_KEY}"

# Local Ollama (fully offline)
[providers.ollama]
type = "openai"
base_url = "http://localhost:11434/v1"
api_key = "ollama"

# Local LM Studio
[providers.lmstudio]
type = "openai"
base_url = "http://localhost:1234/v1"
api_key = "lm-studio"
```

#### Provider Type Quick Reference

| Interface Type | `type` Value | Required Fields |
|----------------|--------------|-----------------|
| Anthropic Messages API | `anthropic` | `api_key` |
| OpenAI / Any Compatible | `openai` | `api_key` + `base_url` (required for non-official) |
| Google Gemini | `gemini` | `api_key` |
| GitHub Copilot (subscription) | `github_copilot` | None (auto-loaded after OAuth) |
| ChatGPT Pro (subscription) | `codex_oauth` | None (auto-loaded after OAuth) |
| Azure OpenAI | `codex` | `api_key` + extra.azure_endpoint |
| Any other compatible | any name | `api_key` + `base_url` |

### Scenario Definition

```toml
[scenarios.production_code]
models = [
  { provider = "github_copilot", model = "claude-sonnet-4-6", cost_tier = "low" },
  { provider = "codex_oauth", model = "gpt-5.4", cost_tier = "low" },
  { provider = "anthropic", model = "claude-opus-4-5", cost_tier = "high" }
]
default_tier = "low"
match_task_types = ["coding"]
priority = 100

[scenarios.budget_mode]
models = [
  { provider = "openrouter", model = "meta-llama/llama-3.1-8b-instruct:free", cost_tier = "low" },
  { provider = "groq", model = "llama-3.3-70b-versatile", cost_tier = "low" },
  { provider = "ollama", model = "qwen2.5:7b", cost_tier = "low" }
]
default_tier = "low"
is_default = true
```

### Routing Configuration

```toml
[routing]
fallback_enabled = true        # Enable failover
timeout_ms = 30000             # Request timeout
retry_count = 2                # Retry count per model
confidence_threshold = 0.6     # Minimum confidence for auto-routing
```

## Best Practices

### 1. Environment Variable Management

Always use environment variables to store sensitive information:

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-..."
```

Do not store actual keys in `config.toml`.

### 2. Cost Optimization

Create different scenarios for different tasks:

```toml
[scenarios.important_task]
models = [{ provider = "anthropic", model = "claude-opus" }]

[scenarios.general_task]
models = [{ provider = "openai", model = "gpt-3.5-turbo" }]
```

### 3. Failover Chains

Configure multiple models to ensure high availability:

```toml
[scenarios.critical]
models = [
  { provider = "anthropic", model = "claude-opus" },
  { provider = "openai", model = "gpt-4" },
  { provider = "anthropic", model = "claude-sonnet" }
]
```

### 4. Monitoring

Regularly check the `/stats` endpoint:

```bash
watch -n 5 'curl -s http://127.0.0.1:8989/stats | jq .'
```

## Troubleshooting

### Problem: Connection Refused

```
error: Connection refused (os error 111)
```

**Solution**: Ensure the server is running

```bash
cargo run --release -- --config config.toml
```

### Problem: Authentication Failed

```json
{ "error": "Unauthorized" }
```

**Solution**: Check API keys

```bash
echo $ANTHROPIC_API_KEY  # Verify environment variable
```

### Problem: Timeout

```json
{ "error": "Request timeout" }
```

**Solution**: Increase `timeout_ms` or check network

```toml
[routing]
timeout_ms = 60000  # Increase to 60 seconds
```

## System Requirements

- **Rust**: 1.70 or higher
- **Cargo**: Latest version
- **Memory**: Minimum 256 MB
- **Network**: Internet connection

## Building and Testing

### Compilation

```bash
# Debug build
cargo build

# Release build
cargo build --release
```

### Running Tests

```bash
# All tests
cargo test

# Run specific test
cargo test config::parser

# Tests with output
cargo test -- --nocapture
```

### Code Quality Checks

```bash
# Clippy checks
cargo clippy

# Format check
cargo fmt --check

# Full check
cargo check
```

## Performance

- **Startup time**: < 1 second
- **Request latency**: 1-3 seconds (depends on provider)
- **Concurrent requests**: Full Actix-web concurrency support
- **Memory usage**: 30-50 MB

## Technology Stack

| Technology | Purpose |
|-----------|---------|
| **Tokio** | Async runtime |
| **Actix-web** | Web framework |
| **Serde + TOML** | Configuration serialization |
| **async-trait** | Async traits |
| **Ratatui** | TUI framework |
| **Tracing** | Logging |

## Contributing

Contributions are welcome! Please follow these steps:

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit your changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

## Roadmap

- [x] Multi-vendor support
- [x] Failover mechanism
- [x] TOML configuration system
- [x] HTTP API
- [x] Monitoring and statistics
- [x] TUI authentication
- [x] **Cross-platform installation scripts** (macOS, Linux, Windows)
- [x] **GitHub Actions CI/CD workflows** (automated multi-platform builds, releases)
- [x] **Complete installation documentation** (quick start, detailed guides, troubleshooting)
- [ ] Hot config reload
- [ ] Database persistence
- [ ] Prometheus metrics
- [ ] Kubernetes deployment
- [ ] More provider integrations

## FAQ

### Q: Can I use it in production?

**A**: Yes! The project is thoroughly tested (54 tests passing) with complete error handling, monitoring, and automated release processes. See [RELEASE_GUIDE.md](RELEASE_GUIDE.md) for release details.

### Q: How simple is installation?

**A**: Very simple! Just run `bash install.sh` for one-click installation. Supports macOS, Linux, and Windows. See [QUICK_INSTALL.txt](QUICK_INSTALL.txt).

### Q: How do I configure?

**A**: Two ways:
1. **Recommended**: Run `bash yolo-setup.sh` for interactive setup
2. **Manual**: Edit the `config.toml` file

See [INSTALL.md](INSTALL.md) for complete instructions.

### Q: How do I uninstall?

**A**: Run `bash uninstall.sh` and follow the prompts. Config files can be kept or deleted.

### Q: How do I add a new provider?

**A**: See the development guide in [PROJECT_SUMMARY.md](PROJECT_SUMMARY.md). Quick steps:

1. Create a new file in `src/provider/`
2. Implement the `Provider` trait
3. Register in `factory.rs`

### Q: Do you support multi-language prompts?

**A**: Yes, YoloRouter supports prompts in any language. Support depends on the underlying AI provider.

### Q: How do I set up a proxy/VPN?

**A**: Use environment variables (with reqwest):

```bash
export HTTP_PROXY=http://proxy.example.com:8989
export HTTPS_PROXY=http://proxy.example.com:8989
```

### Q: Can I use multiple configuration files simultaneously?

**A**: Not in the current version, but you can run multiple instances via scripts.

## Contact

- **GitHub**: [sternelee/YoloRouter](https://github.com/sternelee/YoloRouter)
- **Issue Reports**: Submit a GitHub Issue
- **Discussions**: GitHub Discussions

## Acknowledgments

Thanks to all the developers of the dependencies, especially:

- [Tokio](https://tokio.rs/) - Async runtime
- [Actix-web](https://actix.rs/) - Web framework
- [Serde](https://serde.rs/) - Serialization framework

## Related Projects

- [cc-switch](https://github.com) - AI model switching tool
- [ClawRouter](https://github.com) - Another routing solution

---

<div align="center">

**Made with ❤️ by [sternelee](https://github.com/sternelee)**

If you find this helpful, please give it a ⭐!

</div>
