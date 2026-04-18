# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

YoloRouter is a Rust-based intelligent AI model routing proxy. It sits between AI clients (Claude Code, OpenAI SDKs, etc.) and multiple AI providers, routing requests based on cost, capability, scenario, and availability. Supports 10+ vendors including Anthropic, OpenAI, Gemini, GitHub Copilot, ChatGPT Pro (Codex), OpenRouter, Groq, DeepSeek, Ollama, and any OpenAI-compatible API.

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
```

## High-Level Architecture

### Provider Abstraction (`src/provider/`)

All AI backends implement the `Provider` trait (`src/provider/mod.rs`):

```rust
#[async_trait]
pub trait Provider: Send + Sync {
    async fn send_request(&self, request: &ChatRequest) -> Result<ChatResponse>;
    async fn start_streaming_request(&self, request: &ChatRequest) -> Result<Response>;
    fn supports_streaming(&self) -> bool;
    fn name(&self) -> &str;
    fn model_list(&self) -> Vec<String>;
}
```

`ProviderFactory::create_provider()` (`src/provider/factory.rs`) dispatches by `provider_type` string (`"anthropic"`, `"openai"`, `"github_copilot"`, `"codex_oauth"`, `"gemini"`, `"codex"`). The generic `openai` type with a custom `base_url` handles all OpenAI-compatible third-party services (OpenRouter, Groq, DeepSeek, Ollama, etc.).

**To add a new provider:** create a file in `src/provider/`, implement the `Provider` trait, and register it in `factory.rs`.

### Three-Tier Routing Engine (`src/router/engine.rs`)

The `RoutingEngine::route()` method makes decisions in this priority order:

1. **Explicit scenario** — from a TUI override or endpoint mapping (skips all analysis)
2. **Direct routing** — `provider:model` format (e.g., `"github_copilot:gpt-5.4"`) routes directly to that provider; bare model name routes directly if exactly one provider advertises it
3. **Auto-routing** — `FastAnalyzer` performs 15D analysis on the request, scores candidates against configured scenarios, and selects the best match

If no decision is reached, it returns a clear error telling the user to configure scenarios or use `provider:model`.

### Fallback Chain (`src/router/fallback.rs`)

When `fallback_enabled = true`, requests execute through an ordered chain:

- Models are ordered by `cost_tier` (preferred tier first)
- Each model's `fallback_to` field links to the next model in the chain
- `ProviderHealthTracker` (`src/router/health.rs`) tracks provider failures and enforces cooldown (default 3 hours, configurable via `cooldown_secs`)
- The chain skips providers in cooldown and retries each model up to `retry_count` times

### Streaming Architecture (`src/server/mod.rs`)

The server exposes protocol-adapter endpoints (`/v1/anthropic`, `/v1/openai`, `/v1/gemini`, `/v1/codex`, `/v1/github`, `/v1/auto`). The endpoint name determines the **request/response protocol format**, not the target provider. The routing engine decides which provider/model actually handles the request.

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
