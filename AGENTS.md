# AGENTS.md — YoloRouter Development Context

## Project Essentials

**YoloRouter** is a Rust-based AI model routing proxy. Routes requests to multiple AI providers (Anthropic, OpenAI, Gemini, GitHub Copilot, Codex, OpenAI-compatible) based on scenario, cost, and capability. Supports daemon mode, TUI-driven auth, TOML config with env var expansion, fallback chains, and HTTP API.

- **Binary**: `yolo-router` → `target/release/yolo-router`
- **Rust edition**: 2021 | **Default port**: 8989 (bound to `127.0.0.1` only)
- **No Makefile/Justfile** — all commands are `cargo` invocations

## Developer Commands

```bash
# Build
cargo build --release             # Production
cargo check                       # Fastest syntax check

# Test
cargo test --lib                  # Unit tests only
cargo test                        # All tests (unit + integration)
cargo test --lib config::         # Single module
cargo test -- --nocapture         # Show println! output
# CI uses: cargo test --lib --release

# Lint / Format
cargo fmt                                                          # Auto-format
cargo fmt --check                                                  # CI format check
cargo clippy --all-targets --release -- -D warnings               # CI strict mode
cargo clippy --fix --lib -p yolo-router --allow-dirty --allow-staged  # Auto-fix

# Run daemon
YOLO_CONFIG=config.toml cargo run --release
cargo run -- --config /path/to/config.toml
cargo run -- --auth github        # GitHub Copilot OAuth device flow
cargo run -- --auth codex         # Codex/ChatGPT Pro OAuth device flow
cargo run -- --tui                # TUI config editor
```

## Module Structure

```
src/
├── main.rs               # Entry point; --auth, --config, --tui flags
├── lib.rs                # Public exports
├── models.rs             # ChatRequest/ChatResponse, Message types
├── error.rs              # YoloRouterError (thiserror)
├── config/
│   ├── mod.rs            # Config structs: Daemon/Provider/Scenario/Routing
│   └── parser.rs         # from_file(), env var expansion
├── provider/
│   ├── mod.rs            # Provider trait
│   ├── factory.rs        # create_provider() factory
│   ├── anthropic.rs
│   ├── openai.rs         # Also handles all OpenAI-compatible providers
│   ├── gemini.rs
│   ├── github_copilot.rs # Token from ~/.config/yolo-router/github_token
│   └── codex_oauth.rs    # Token from ~/.config/yolo-router/codex_oauth.json
├── router/
│   ├── mod.rs            # ProviderRegistry, Router
│   ├── engine.rs         # RoutingEngine — scenario selection; also handles provider:model bypass
│   └── fallback.rs       # FallbackChain
├── server/mod.rs         # Actix-web routes + AppState
├── analyzer/multidimensional.rs  # FastAnalyzer (15 dimensions)
├── tui/
│   ├── auth.rs           # Auth state machine
│   ├── github_auth.rs    # GitHub device flow
│   └── codex_auth.rs     # ChatGPT Pro device flow
└── utils/
    ├── stats.rs          # StatsCollector (last 1000 requests, thread-safe)
    └── mod.rs            # init_logger() — tracing setup
```

## HTTP API Routes

```
GET  /health                        # Status + providers/scenarios list
GET  /config                        # Current config as TOML text
GET  /stats                         # Request statistics

GET  /control/status                # Active overrides + providers/scenarios
POST /control/override              # Pin endpoint to a scenario
DELETE /control/override/{endpoint} # Clear override
POST /control/reload                # Reload config from disk (no restart needed)

POST /v1/auto                       # Auto-select via 15D FastAnalyzer
POST /v1/anthropic
POST /v1/anthropic/v1/messages      # Anthropic native path alias
POST /v1/openai
POST /v1/openai/chat/completions    # OpenAI native path alias
POST /v1/gemini
POST /v1/gemini/chat/completions
POST /v1/codex
POST /v1/codex/chat/completions
```

**Server quirk**: JSON body limit is 10MB. Entry point macro is `actix_web::main`, not `tokio::main`.

## Configuration

### Config Loading Order (highest priority first)
1. `--config <path>` CLI flag
2. `YOLO_CONFIG` env var
3. `config.toml` in CWD

### Key config.toml Fields

```toml
[daemon]
port = 8989
log_level = "info"        # or use RUST_LOG env var

[providers.github_copilot]
type = "github_copilot"   # Token auto-loaded from ~/.config/yolo-router/github_token

[providers.openrouter]
type = "openai"           # Any OpenAI-compatible service uses type = "openai"
base_url = "https://openrouter.ai/api/v1"
api_key = "${OPENROUTER_API_KEY}"   # Env var expansion

[providers.ollama]
type = "openai"
base_url = "http://127.0.0.1:11434/v1"
api_key = "${OLLAMA_API_KEY}"

[routing]
fallback_enabled = true
timeout_ms = 30000
retry_count = 2
confidence_threshold = 0.6   # Below this, fallback to default scenario

[[scenarios.coding.models]]
provider = "github_copilot"
model = "claude-sonnet-4.6"
cost_tier = "high"
```

**dotenv**: `.env` files in CWD are loaded automatically via the `dotenv` crate.

## Request Flow

1. `POST /v1/auto` → `FastAnalyzer` scores request on 15 dimensions
2. `RoutingEngine` selects scenario — **unless** `model` field is `"provider:model"` (e.g., `"github_copilot:claude-sonnet-4.6"`), which bypasses analysis entirely
3. `FallbackChain` tries models in priority order; retries on failure
4. Provider forwards to AI service; response serialized + stats recorded

**Direct routing bypass**: Set `"model": "github_copilot:claude-sonnet-4.6"` in the request. The colon-separated prefix is consumed in `router/engine.rs`; the stripped model name is forwarded to the provider.

## Critical Code Locations

| Task | File |
|------|------|
| Add provider | `src/provider/{name}.rs` + `factory.rs:create_provider()` + `config/mod.rs:ProviderConfig` |
| Add endpoint | `src/server/mod.rs:start_server()` + handler; handler gets `AppState` |
| Change config schema | `config/mod.rs` (structs) + `parser.rs` (if Serde can't auto-handle) + `config.example.toml` |
| Add test | Inline `#[cfg(test)] mod tests` in same file; integration tests in `tests/integration_tests.rs` |
| Debug logging | `info!`, `debug!`, `error!` macros (tracing) |

## Common Pitfalls

- **`system` field**: Claude Code sends `system` as a content-blocks array, not a string. Current code handles both. If you see `invalid type: sequence, expected a string`, rebuild from latest.
- **`provider:model` bypass**: Direct routing check in `engine.rs` must run BEFORE scenario analysis. If you add code before the prefix check, auto-routing can hijack the request.
- **GitHub Copilot `expires_at`**: Comes as an integer Unix timestamp but typed as `Option<String>`. Uses `deserialize_optional_int_as_string` custom deserializer in `github_copilot.rs`.
- **Auth persistence**: GitHub token → `~/.config/yolo-router/github_token`; Codex → `~/.config/yolo-router/codex_oauth.json`. Directory must exist.
- **`reqwest` is 0.11**: Not 0.12 — the API surface differs; do not upgrade without checking breaking changes.
- **`ratatui` + `crossterm` version lock**: `ratatui 0.26` requires `crossterm 0.27`. Keep them paired.
- **Duplicate scenario models**: TUI rejects duplicate `(provider, model, cost_tier)` entries. Use different `cost_tier` or different scenario to add the same model twice.
- **Clippy in CI**: `-- -D warnings` treats all warnings as errors. Run `cargo clippy --all-targets --release -- -D warnings` locally before pushing.
- **`cargo audit`/`cargo-tarpaulin`** are not installed by default — install before local use; CI installs them in the workflow.

## Key Dependencies

| Crate | Version | Note |
|-------|---------|------|
| `actix-web` | 4.4 | HTTP server |
| `tokio` | 1.35 | Async runtime (features=["full"]) |
| `reqwest` | 0.11 | HTTP client — **not 0.12** |
| `ratatui` | 0.26 | TUI — must pair with crossterm 0.27 |
| `async-trait` | 0.1 | Required for `Provider` trait async methods |
| `oauth2` | 4.4 | Device flow for Codex/GitHub Copilot |
| `dotenv` | 0.15 | Auto-loads `.env` in CWD |

## CI Overview

Four workflows in `.github/workflows/`:
- **`ci.yml`**: `test` + `fmt` + `clippy` + `security` + `coverage` (ubuntu/macos/windows matrix)
- **`build.yml`**: Cross-platform artifact builds for linux-amd64, darwin-amd64, darwin-arm64, windows-amd64
- **`release.yml`**: Triggered by `v*` tags → builds all platforms + creates GitHub Release
- **`validate.yml`**: Validates workflow YAML files on changes to `.github/workflows/`

CI env: `CARGO_TERM_COLOR=always`, `RUST_BACKTRACE=1`. Uses `dtolnay/rust-toolchain@stable` (no pinned version).

## Documentation

- `USER_GUIDE.md` — Full user docs, config examples, API endpoints
- `CLAUDE_CODE_SETUP.md` — Claude Code integration + troubleshooting
- `.github/copilot-instructions.md` — Architecture & phases overview
- `config.example.toml` — Canonical config reference

---
**Rust edition**: 2021 | **MSRV**: 1.70+
