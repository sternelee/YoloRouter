# Copilot Instructions for YoloRouter

## Project Overview

YoloRouter is a **Rust-based intelligent AI model routing proxy**. It enables users to configure multiple AI providers (Anthropic, OpenAI, Google Gemini, GitHub, Codex, etc.) and intelligently routes requests to the best model based on cost, capability, and scenario. Supports daemon mode, TUI interfaces, TOML configuration, fallback chains, and multiple API endpoints.

## Build & Test Commands

### Build
```bash
cargo build                # Debug build
cargo build --release      # Release with optimizations
cargo check               # Quick syntax check (no compilation)
```

### Run
```bash
cargo run                  # Run in daemon mode
YOLO_CONFIG=config.toml cargo run  # Run with custom config
```

### Testing
```bash
cargo test --lib          # Unit tests only
cargo test --lib config   # Test specific module
cargo test -- --nocapture # Show output during tests
```

### Code Quality
```bash
cargo clippy              # Check warnings
cargo clippy --fix        # Auto-fix issues
cargo fmt                 # Format code
cargo fmt --check         # Check formatting only
```

## Architecture

**Phase 1 (Complete)**: Project setup, config system, provider abstraction
- `src/config/` — TOML parsing, schema, validation, env var expansion
- `src/provider/` — Provider trait, implementations (Anthropic, OpenAI, Gemini, Generic), factory
- `src/error.rs` — Unified error types
- `src/models.rs` — Chat request/response types

**Phase 2-3 (In Progress)**: TUI auth, daemon/server, HTTP routing, scenario-based routing, fallback

**Phase 4-6 (Planned)**: Dynamic config switching, monitoring, skills, documentation, testing

## Module Structure

```
src/
├── lib.rs              # Main library exports
├── main.rs             # Daemon entry point
├── error.rs            # Error types
├── models.rs           # Chat message/request/response types
├── config/             # TOML configuration system
│   ├── mod.rs          # Schema definitions
│   └── parser.rs       # Config parsing with env expansion
├── provider/           # AI provider implementations
│   ├── mod.rs          # Provider trait
│   ├── factory.rs      # Create providers from config
│   ├── anthropic.rs    # Anthropic Claude client
│   ├── openai.rs       # OpenAI GPT client
│   ├── gemini.rs       # Google Gemini client
│   └── generic.rs      # Generic provider template
├── router/             # Request routing logic
│   ├── mod.rs
│   └── engine.rs       # Routing engine with fallback support
├── server/             # HTTP API server
│   └── mod.rs          # Actix-web routes (/v1/anthropic, /v1/openai, etc)
├── tui/                # TUI interface
│   └── mod.rs
└── utils/              # Utilities
    └── mod.rs          # Logger initialization
```

## Configuration

**config.toml structure**:
- `[daemon]` — Port, log level
- `[providers.*]` — Provider configs with API keys (support `${ENV_VAR}` expansion)
- `[scenarios.*]` — Named scenarios with model chains and fallback rules
- `[routing]` — Global fallback, timeout, retry settings

See `config.example.toml` for complete example.

## Key Conventions

1. **Provider Pattern**: All providers implement `Provider` trait with `send_request()`, can be swapped at runtime
2. **Config Validation**: Config includes `.validate()` to check provider references exist
3. **Error Handling**: Use `YoloRouterError` enum for all errors
4. **Async/Await**: Use `async-trait` for trait methods; tokio runtime
5. **Testing**: Unit tests in same file as code under `#[cfg(test)] mod tests`
6. **Environment Variables**: API keys can use `${VAR}` syntax in TOML

## Common Tasks

- **Add new provider**: Create `src/provider/newprovider.rs`, impl `Provider` trait, add to factory
- **Add scenario**: Edit `config.toml`, add `[scenarios.name]` with model chain
- **Test provider**: `cargo test --lib provider::factory`
- **Run server**: `cargo run` (uses YOLO_CONFIG env var or config.toml)

## Dependencies Overview

- **actix-web** — Async HTTP server
- **tokio** — Async runtime
- **serde/toml** — Config serialization
- **reqwest** — HTTP client for provider calls
- **async-trait** — Async trait support
- **tracing** — Logging framework

## Next Steps (Phase 2)

1. Implement `daemon-mode` — Complete server startup, health checks
2. Implement `http-endpoints` — Route /v1/* paths to specific providers
3. Implement `scenario-routing` — Match request to scenario and select model
4. Implement `fallback-logic` — Retry chain on provider failure
