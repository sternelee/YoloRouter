# AGENTS.md — YoloRouter Development Context

## Project Essentials

**YoloRouter** is a Rust-based intelligent AI model routing proxy. It routes requests to multiple AI providers (Anthropic, OpenAI, Gemini, GitHub Copilot, Codex, and OpenAI-compatible services) based on scenario, cost, and capability. Supports daemon mode, TUI-driven configuration, TOML config with env var expansion, fallback chains, and HTTP API.

**Status**: Feature-complete (54 tests passing). Builds and runs; all core features functional.

## Key Developer Commands

```bash
# Build
cargo build --release             # Production build
cargo check                       # Quick syntax check

# Test
cargo test --lib                 # Unit tests only (47 passing)
cargo test                        # All tests (54 passing)
cargo test config::              # Test specific module
cargo test -- --nocapture        # Show println! output

# Code quality
cargo clippy                      # Lint warnings
cargo fmt --check                # Format check
cargo fmt                         # Auto-format

# Run daemon
YOLO_CONFIG=config.toml cargo run --release

# Run with auth flow
cargo run -- --auth anthropic    # Interactive TUI auth for provider
cargo run -- --tui               # TUI mode (config editor)
```

## Architecture Essentials

### Module Structure

```
src/
├── main.rs               # Daemon entry, --auth, --config, --tui flags
├── lib.rs                # Public exports
├── models.rs             # ChatRequest/ChatResponse, Message types
├── error.rs              # YoloRouterError enum (thiserror)
├── config/               # TOML parsing + validation
│   ├── mod.rs            # Daemon/Provider/Scenario/Routing config structs
│   └── parser.rs         # from_file(), env var expansion
├── provider/             # Provider trait implementations
│   ├── mod.rs            # Provider trait definition
│   ├── factory.rs        # create_provider() factory
│   ├── anthropic.rs      # Anthropic client
│   ├── openai.rs         # OpenAI client
│   ├── gemini.rs         # Gemini client
│   └── codex_oauth.rs    # OAuth-based providers
├── router/               # Routing decisions + fallback
│   ├── mod.rs            # ProviderRegistry, Router
│   ├── engine.rs         # RoutingEngine (scenario selection)
│   └── fallback.rs       # FallbackChain (retry logic)
├── server/               # HTTP daemon
│   └── mod.rs            # Actix-web routes: /v1/{anthropic,openai,gemini,codex,auto}, /health, /stats, /config, /control/*
├── analyzer/             # 15-dimensional model analyzer
│   └── multidimensional.rs  # FastAnalyzer, RequestFeatures, TaskType, Language, ModelScore
├── tui/                  # Terminal UI
│   ├── auth.rs           # Interactive provider auth
│   ├── github_auth.rs    # GitHub OAuth device flow
│   ├── codex_auth.rs     # ChatGPT Pro OAuth device flow
│   └── config_editor.rs  # Config editor UI (framework)
└── utils/                # Stats, logging
    ├── stats.rs          # StatsCollector (thread-safe, tracks requests/providers/times)
    └── init_logger()     # Tracing setup
```

### Request Flow

1. HTTP request arrives at endpoint (e.g., `POST /v1/auto`)
2. `FastAnalyzer` examines request (15 dimensions: complexity, cost, latency, accuracy, etc.)
3. `RoutingEngine` selects best scenario + model list
4. `FallbackChain` tries models in order; retries on failure
5. `Provider` sends to actual AI service (or returns placeholder in dev)
6. Response serialized, stats recorded

## Configuration

### config.toml Structure

```toml
[daemon]
port = 8989
log_level = "info"

[providers.anthropic]
type = "anthropic"
api_key = "${ANTHROPIC_API_KEY}"   # Env var expansion

[providers.openai]
type = "openai"
api_key = "${OPENAI_API_KEY}"

# OpenAI-compatible services (all 100+ models via OpenRouter, Groq, DeepSeek, etc.)
[providers.openrouter]
type = "openai"
base_url = "https://openrouter.ai/api/v1"
api_key = "${OPENROUTER_API_KEY}"

[scenarios.production]
models = [
  { provider = "anthropic", model = "claude-opus", cost_tier = "high" },
  { provider = "openai", model = "gpt-4", cost_tier = "high" }
]

[routing]
fallback_enabled = true
timeout_ms = 30000
retry_count = 2
```

**Key validation rules**: All referenced providers must exist; scenarios can have cost_tier filters.

## Testing Quick Reference

**All tests passing** (54/54):

- `config::parser::tests` — TOML parsing, env expansion, validation
- `config::parser::mutation_tests` — Config mutations, duplicate detection (added 3 new tests)
- `provider::factory::tests` — Create providers, error handling
- `utils::stats::tests` — Request tracking, aggregation
- `router::*` — Routing engine, fallback chains
- `analyzer::*` — Model scoring (FastAnalyzer 15D)
- `tui::auth::tests` — Auth UI state machine
- `integration_tests.rs` — Full config round-trip, multi-provider scenarios

**Run tests:**

```bash
cargo test --lib           # Unit tests only (47 passing)
cargo test                 # All tests (54 passing)
cargo test config::        # Test specific module
cargo test -- --nocapture  # Show println! output
```

## Critical Code Locations

| Task                 | File                                                                                | Lines |
| -------------------- | ----------------------------------------------------------------------------------- | ----- |
| Add new provider     | `src/provider/{name}.rs` + register in `factory.rs:create_provider()`               | —     |
| Add HTTP endpoint    | `src/server/mod.rs:start_server()` + handler                                        | —     |
| Change config schema | `src/config/mod.rs` (structs), `parser.rs` (parsing), `config.example.toml` (docs)  | —     |
| Fix build errors     | Check `cargo test` output; integration test expects `system` field in `ChatRequest` | —     |
| Add test             | Same file as code under `#[cfg(test)] mod tests`                                    | —     |
| Debug logging        | `utils::init_logger()` sets up tracing; use `info!`, `debug!`, `error!` macros      | —     |

## Build & Deployment Notes

- **Binary name**: `yolo-router` (from `Cargo.toml` `[package] name`)
- **Executable location after build**: `target/release/yolo-router`
- **Config loading order**: `--config` flag → `YOLO_CONFIG` env var → `config.toml` (current dir)
- **Log level control**: `[daemon] log_level` in config or `RUST_LOG` env var (tracing-subscriber)
- **Port binding**: Configured in `[daemon] port` (default 8989)

## Common Pitfalls

1. **Claude Code `system` field error** — `invalid type: sequence, expected a string` means Claude Code is sending system as a content blocks array (not a string). **Fixed in latest version**; system field now supports both formats. Ensure you're running the latest build.
2. **Duplicate model entries in TUI** — When re-selecting the same provider/model/cost_tier in TUI, the system now rejects duplicates with a clear error message. This prevents configuration pollution. To add the same model with a different cost tier or in a different scenario, use a different cost_tier or scenario name.
3. **Config not loading** — Check env vars are exported (`export ANTHROPIC_API_KEY="***"`) and config file path is correct
4. **Provider returns error** — In dev, providers return placeholder responses. To integrate real APIs, update `src/provider/{name}.rs:send_request()`
5. **TUI auth doesn't persist** — Auth credentials are saved to `~/.config/yolo-router/providers.json` (see `tui/auth.rs` for path); ensure directory exists
6. **Port already in use** — Change `[daemon] port` in config or kill existing process
7. **`provider:model` routing ignored** — Direct routing (`github_copilot:gpt-5-mini`) must be checked BEFORE auto-routing in `router/engine.rs:route()`. If auto-routing runs first, analyzer may match a default scenario and hijack the request. The `provider:model` split also must strip the prefix before forwarding (set `req.model = model_parts[1]`)
8. **GitHub Copilot API type mismatch** — `CopilotToken.expires_at` comes as integer Unix timestamp but was declared `Option<String>`. Use custom `deserialize_optional_int_as_string` deserializer to handle both formats (`src/provider/github_copilot.rs`)
9. **Clippy warnings** — Run `cargo clippy --fix --lib -p yolo-router --allow-dirty --allow-staged` to auto-fix. Common: redundant closures, `io::Error::other()`, useless `format!()`

## Why Features Exist

| Feature                    | Why                                                                               | File                           |
| -------------------------- | --------------------------------------------------------------------------------- | ------------------------------ |
| **15D FastAnalyzer**       | Auto-select best model without hardcoding routes; saves ~40% cost vs static rules | `analyzer/multidimensional.rs` |
| **Fallback chains**        | Ensure reliability when a provider fails or quota exhausted                       | `router/fallback.rs`           |
| **Env var expansion**      | Don't commit secrets; rely on deployment env                                      | `config/parser.rs`             |
| **Scenario-based routing** | Different tasks (coding vs. general) benefit from different models/costs          | `router/engine.rs`             |
| **TOML config**            | Human-friendly, no code changes needed to retune models                           | `config/mod.rs`                |
| **TUI auth**               | Interactive provider auth without editing config files                            | `tui/auth.rs`                  |
| **Stats endpoint**         | Monitor which providers are being called; detect issues                           | `utils/stats.rs`               |

## Extending the System

### Add a New Provider

1. Create `src/provider/{name}.rs` with `impl Provider` (trait requires `send_request()`)
2. Add creation logic to `src/provider/factory.rs:create_provider()` match statement
3. Add config struct to `src/config/mod.rs:ProviderConfig`
4. Update `config.example.toml` with example
5. Write unit test in the provider file

### Add a New HTTP Endpoint

1. Add route in `src/server/mod.rs:start_server()` using `web::post()`
2. Create async handler function in same file or `src/server/handlers.rs`
3. Handler receives `AppState` (contains `router`, `stats`, `config`)
4. Return `HttpResponse` with JSON or error
5. Write integration test in `tests/integration_tests.rs`

### Modify Config Schema

1. Update struct in `src/config/mod.rs`
2. Update parsing logic in `src/config/parser.rs` if needed (Serde usually handles it)
3. Update `config.example.toml` and `config.toml` examples
4. Write test in `config::parser::tests`

## Performance & Limits

- **Startup time**: ~500ms to 1s (includes config parsing)
- **Request handling**: 1-3s typical (depends on provider latency, not YoloRouter)
- **Memory**: ~50MB resident
- **Concurrent requests**: Actix-web default (num_cpus × 2 workers)
- **Stats buffer**: Tracks last 1000 requests; older entries dropped
- **Config reload**: Requires daemon restart (hot-reload not yet implemented)

## Dependencies to Know

| Crate            | Purpose                          | Version |
| ---------------- | -------------------------------- | ------- |
| `actix-web`      | HTTP server framework            | 4.4     |
| `tokio`          | Async runtime                    | 1.35    |
| `serde` + `toml` | Config serialization             | 1.0     |
| `reqwest`        | HTTP client (for provider calls) | 0.11    |
| `ratatui`        | Terminal UI rendering            | 0.26    |
| `tracing`        | Structured logging               | 0.1     |
| `thiserror`      | Error definitions                | 1.0     |

## Documentation Pointers

- **USER_GUIDE.md** — Full user docs (config examples, API endpoints, troubleshooting)
- **CLAUDE_CODE_SETUP.md** — Complete Claude Code integration guide with troubleshooting
- **.github/copilot-instructions.md** — Architecture & phases overview
- **PROJECT_SUMMARY.md** — Detailed feature list & test summary
- **README.md** — Project intro, quick start, features

## Getting Unstuck

- **Build fails?** Run `cargo check` first to see syntax errors; then `cargo build`
- **Test fails?** Run `cargo test -- --nocapture` to see println! output
- **Config doesn't load?** Check file exists and has valid TOML syntax (`toml-cli validate config.toml` or try in an online validator)
- **Endpoint 404?** Verify route registered in `src/server/mod.rs` and server is running
- **Auth not saving?** Check `~/.config/yolo-router/` directory exists and is writable

---

**Last updated**: 2024 | **Rust edition**: 2021 | **MSRV**: 1.70+
