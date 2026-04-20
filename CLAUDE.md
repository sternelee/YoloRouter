# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

YoloRouter is a Rust-based intelligent AI model routing proxy. It sits between AI clients (Claude Code, OpenAI SDKs, etc.) and multiple AI providers, routing requests based on cost, capability, scenario, and availability. Supports 10+ vendors including Anthropic, OpenAI, Gemini, GitHub Copilot, ChatGPT Pro (Codex), Cursor, OpenRouter, Groq, DeepSeek, Ollama, and any OpenAI-compatible API.

## Build, Test, and Quality Commands

```bash
# Quick check (fastest)
cargo check

# Build
cargo build
cargo build --release

# Test
cargo test --lib                    # Unit tests only
cargo test --test '*'              # Integration tests
cargo test --lib --release         # CI mode (release assertions)
cargo test -- --nocapture          # With stdout visible
cargo test <filter>                # Run specific test(s) by name/path

# Lint and format (CI enforces zero warnings)
cargo clippy --all-targets --release -- -D warnings
cargo fmt -- --check
cargo fmt                           # Apply formatting
```

## Running the Application

```bash
# Daemon mode
cargo run --release -- --config config.toml

# Or with env var
YOLO_CONFIG=config.toml cargo run --release

# TUI mode
cargo run --release -- --tui --config config.toml

# OAuth authentication
./target/release/yolo-router --auth github      # GitHub Copilot
./target/release/yolo-router --auth codex       # ChatGPT Pro / Codex
./target/release/yolo-router --auth cursor      # Cursor IDE (via cursor-agent login)
```

## High-Level Architecture

### Provider Abstraction (`src/provider/`)

All AI backends implement the `Provider` trait (`src/provider/mod.rs`):

```rust
pub type ByteStream = Pin<Box<dyn Stream<Item = std::io::Result<Bytes>> + Send>>;

#[async_trait]
pub trait Provider: Send + Sync {
    async fn send_request(&self, request: &ChatRequest) -> Result<ChatResponse>;
    async fn start_streaming_request(&self, request: &ChatRequest) -> Result<ByteStream>;
    fn supports_streaming(&self) -> bool;
    fn name(&self) -> &str;
    fn model_list(&self) -> Vec<String>;
}
```

`ProviderFactory::create_provider()` (`src/provider/factory.rs`) dispatches by `provider_type` string (`"anthropic"`, `"openai"`, `"github_copilot"` / `"github"`, `"codex_oauth"`, `"gemini"`, `"codex"`, `"cursor"`). The generic `openai` type with a custom `base_url` handles all OpenAI-compatible third-party services (OpenRouter, Groq, DeepSeek, Ollama, etc.).

**To add a new provider:** create a file in `src/provider/`, implement the `Provider` trait, and register it in `factory.rs`.

### Cursor Provider (`src/provider/cursor.rs`)

The `cursor` provider routes requests to Cursor IDE's AI models via the `cursor-agent` CLI subprocess. It requires the Cursor IDE to be installed and authenticated (`cursor-agent login`).

- **Subprocess-based**: Spawns `cursor-agent` with `--output-format stream-json`, reads stdout line-by-line, and converts stream-json events to OpenAI-compatible SSE chunks
- **No API key needed**: Relies on Cursor's own CLI authentication (token stored in `~/.cursor/cli-config.json` or `~/.config/cursor/cli-config.json`)
- **Authentication**: Run `./yolo-router --auth cursor` to launch `cursor-agent login`; the flow detects existing tokens and verifies login success
- **Configurable**: `agent_path` overrides the executable path (defaults to `cursor-agent` or `$CURSOR_AGENT_EXECUTABLE`); `timeout_ms` controls subprocess timeout (default 5 minutes)
- **Model list**: Includes all Cursor-supported models (`auto`, `composer-1.5`, Claude variants, GPT variants, Gemini variants, `grok`, `kimi-k2.5`, etc.)

Example TOML config:
```toml
[[providers]]
name = "cursor"
provider_type = "cursor"
extra = { agent_path = "/usr/local/bin/cursor-agent", timeout_ms = 300000 }
```

### Router Wrapper (`src/router/mod.rs`)

`Router` wraps `RoutingEngine` with a shared `ProviderHealthTracker`. This is the interface the server uses:
- `Router::route()` delegates to `RoutingEngine::route()` with health tracking
- `Router::reload()` rebuilds the engine from a new `Config` but **preserves cooldown state**
- `Router::provider()` looks up a provider by name from the registry

### Three-Tier Routing Engine (`src/router/engine.rs`)

The `RoutingEngine::route()` method makes decisions in this priority order:

1. **Explicit scenario** — from a TUI override or endpoint mapping (skips all analysis)
2. **Direct routing** — `provider:model` format (e.g., `"github_copilot:gpt-5.4"`) routes directly to that provider; bare model name routes directly if exactly one provider advertises it
3. **Auto-routing** — `FastAnalyzer` performs 15D analysis on the request, scores candidates against configured scenarios, and selects the best match

If no decision is reached, it returns a clear error telling the user to configure scenarios or use `provider:model`.

### Fallback Chain (`src/router/fallback.rs`)

When `fallback_enabled = true`, requests execute through an ordered chain:

- Models are ordered by `cost_tier` (preferred `default_tier` first, then remaining tiers)
- Each model's `fallback_to` field links to the next model in the chain by reference (`provider:model` or bare model name if unambiguous)
- Ambiguous bare model names (same name across multiple providers) are handled safely: short-form lookup keys are omitted so `fallback_to` does not silently route to the wrong provider
- `ProviderHealthTracker` (`src/router/health.rs`) tracks provider failures and enforces cooldown (default 3 hours, configurable via `cooldown_secs`)
- The chain skips providers in cooldown and retries each model up to `retry_count` times
- When cooldown is enabled, each provider is attempted exactly once per chain traversal (any failure triggers cooldown immediately); when cooldown is disabled (`cooldown_secs = 0`), retries are honoured

### Streaming Architecture (`src/server/mod.rs`)

The server exposes protocol-adapter endpoints. The endpoint name determines the **request/response protocol format**, not the target provider. The routing engine decides which provider/model actually handles the request.

| Endpoint | Format |
|----------|--------|
| `POST /v1/anthropic` | Anthropic Messages API |
| `POST /v1/anthropic/v1/messages` | Anthropic (full path) |
| `POST /v1/openai` | OpenAI Chat Completions |
| `POST /v1/openai/chat/completions` | OpenAI (full path) |
| `POST /v1/gemini` | OpenAI-compatible |
| `POST /v1/gemini/chat/completions` | OpenAI-compatible (full path) |
| `POST /v1/codex` | OpenAI format |
| `POST /v1/codex/chat/completions` | OpenAI format (full path) |
| `POST /v1/github` | OpenAI format |
| `POST /v1/auto` | OpenAI format; 15D auto-routing |

- `proxy_generic_stream()` handles OpenAI-format SSE for all generic endpoints
- `proxy_anthropic_stream()` handles Anthropic-native SSE format (preserves `data:` prefixes, `event:` lines, etc.)
- Auto-resolution of `"model": "auto"` works with streaming
- Zero-copy byte stream forwarding via `reqwest` -> `actix-web`

### Config System (`src/config/`)

- TOML with `${ENV_VAR}` expansion at load time
- `Config::from_file()` and `Config::from_string()` for parsing
- `Config::validate()` checks that all provider references in scenarios exist
- `Config::to_string()` supports round-trip serialization
- Hot reloadable via `POST /control/reload`

### Error Handling (`src/error.rs`)

Single `YoloRouterError` enum covers all error domains. It implements `actix_web::ResponseError` to map variants to appropriate HTTP status codes (e.g., `Unauthorized` -> 401, `RoutingError` -> 400, provider timeouts -> 504).

### Analyzer (`src/analyzer/multidimensional.rs`)

`FastAnalyzer` analyzes requests across 15 dimensions (complexity, cost, latency, accuracy, reasoning, programming, language, etc.) in < 1ms. It scores model candidates and `match_scenario_by_model_scores()` matches against scenario configs using `match_task_types`, `match_languages`, `priority`, and `is_default`.

### Key Conventions

- Unit tests live in `#[cfg(test)] mod tests` blocks in the same file as the code under test
- `async-trait` is used for async trait methods; the runtime is `tokio` via `#[actix_web::main]`
- API keys must use `${VAR}` syntax in TOML — never hardcode keys
- OAuth tokens are stored in `~/.config/yolo-router/` (GitHub plain text, Codex as JSON)
