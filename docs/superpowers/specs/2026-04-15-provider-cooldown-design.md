# Provider Cooldown Design

**Date**: 2026-04-15  
**Status**: Approved  
**Scope**: `src/router/health.rs` (new), `src/router/mod.rs`, `src/router/engine.rs`, `src/router/fallback.rs`, `src/config/mod.rs`

## Problem

当某个 provider 请求失败（429 限流、网络错误、5xx 等），`FallbackChain` 会切换到下一个 provider 完成本次请求。但下次新请求进来，路由引擎对上次的失败毫无记忆，仍然优先尝试已知失败的 provider，造成不必要的延迟和失败重试。

## Goal

任何 provider 失败后，在可配置的冷却时间内，所有请求跳过该 provider。冷却期结束后自动恢复正常尝试。

## Non-Goals

- 不实现 Half-Open 探测（circuit breaker 完整模式）
- 不在 HTTP API 中暴露冷却状态（仅日志）
- 不区分错误类型（所有失败一律触发冷却）

## Design

### New File: `src/router/health.rs`

```rust
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct ProviderHealthTracker {
    entries: Mutex<HashMap<String, Instant>>,  // provider_name → failed_at
}

impl ProviderHealthTracker {
    pub fn new() -> Self {
        Self { entries: Mutex::new(HashMap::new()) }
    }

    /// Returns true if provider is currently in cooldown.
    pub fn is_cooling_down(&self, provider: &str, cooldown: Duration) -> bool {
        let entries = self.entries.lock().unwrap();
        if let Some(&failed_at) = entries.get(provider) {
            failed_at.elapsed() < cooldown
        } else {
            false
        }
    }

    /// Remaining cooldown duration for display in logs.
    pub fn remaining(&self, provider: &str, cooldown: Duration) -> Option<Duration> {
        let entries = self.entries.lock().unwrap();
        entries.get(provider).and_then(|&failed_at| {
            let elapsed = failed_at.elapsed();
            if elapsed < cooldown { Some(cooldown - elapsed) } else { None }
        })
    }

    /// Record a failure. Resets the cooldown timer.
    pub fn record_failure(&self, provider: &str) {
        let mut entries = self.entries.lock().unwrap();
        entries.insert(provider.to_string(), Instant::now());
    }

    /// Record a success. Clears the cooldown entry.
    pub fn record_success(&self, provider: &str) {
        let mut entries = self.entries.lock().unwrap();
        entries.remove(provider);
    }
}
```

### Config Changes: `src/config/mod.rs`

In `RoutingConfig`:

```rust
pub struct RoutingConfig {
    pub fallback_enabled: bool,
    pub timeout_ms: u64,
    pub retry_count: u32,
    pub confidence_threshold: f64,
    pub cooldown_enabled: bool,   // default: true
    pub cooldown_secs: u64,       // default: 60
}
```

`config.example.toml` 新增：

```toml
[routing]
cooldown_enabled = true
cooldown_secs = 60
```

两字段有默认值，不配置不影响现有 config。

### `Router` struct (`src/router/mod.rs`)

```rust
pub struct Router {
    engine: RwLock<RoutingEngine>,
    tracker: Arc<ProviderHealthTracker>,   // ← new, NOT reset on reload
}
```

`Router::new()` 初始化 tracker；`reload()` 仅重建 engine，tracker 不变。

`Router::route()` 将 `&self.tracker` 传递给 `engine.route()`。

### `RoutingEngine::route()` (`src/router/engine.rs`)

签名变更：

```rust
pub async fn route(
    &self,
    request: &ChatRequest,
    scenario: Option<&str>,
    tracker: &ProviderHealthTracker,   // ← new
) -> Result<ChatResponse>
```

将 tracker 向下传给 `route_via_scenario()`，再传给 `FallbackChain::execute()`。

### `FallbackChain::execute()` (`src/router/fallback.rs`)

```rust
pub async fn execute(
    &self,
    request: &ChatRequest,
    registry: &ProviderRegistry,
    max_retries: u32,
    tracker: &ProviderHealthTracker,   // ← new
    cooldown: Duration,                // ← new
) -> Result<ChatResponse>
```

循环逻辑变为：

```rust
for model_config in &self.scenario.models {
    if tracker.is_cooling_down(&model_config.provider, cooldown) {
        let remaining = tracker.remaining(&model_config.provider, cooldown)
            .unwrap_or_default();
        tracing::warn!(
            provider = model_config.provider,
            remaining_secs = remaining.as_secs(),
            "Provider is cooling down, skipping"
        );
        continue;  // ← skip entirely, no retry loop
    }

    for attempt in 0..=max_retries {
        match provider.send_request(&req).await {
            Ok(response) => {
                tracker.record_success(&model_config.provider);
                return Ok(response);
            }
            Err(e) => {
                tracker.record_failure(&model_config.provider);
                tracing::warn!(
                    provider = model_config.provider,
                    cooldown_secs = cooldown.as_secs(),
                    "Provider failed, entering cooldown: {}", e
                );
                last_error = Some(e.to_string());
                break;  // 失败即进入冷却，不再对同一 provider 重试
            }
        }
    }
}
```

注意：
- provider 失败后立刻 `break`（跳出 attempt 循环），记录冷却，不再重试同一 provider
- `retry_count` 在冷却模式下语义变更为「同一请求内对已冷却 provider 的跳过不计入重试次数」——实际上只会对单个 provider 尝试一次，然后进入冷却
- `cooldown_enabled = false` 时传入 `Duration::ZERO`；FallbackChain 用 `if cooldown > Duration::ZERO` 决定是否调用 tracker，行为与改造前完全一致

## Data Flow

```
Request arrives
    ↓
Router::route(req, scenario)
    ↓ passes &self.tracker
RoutingEngine::route(req, scenario, tracker)
    ↓
route_via_scenario(req, scenario_name, config, timeout, tracker)
    ↓
FallbackChain::execute(req, registry, retries, tracker, cooldown_duration)
    ↓
  for each provider in scenario.models:
    is_cooling_down? → yes → log + skip
                    → no  → try request
                             success → record_success, return
                             fail    → record_failure (starts/resets timer), next provider
    ↓
  all providers skipped/failed → AllProvidersFailed error
```

## Cooldown Behavior

| Situation | Result |
|-----------|--------|
| Provider succeeds | timer cleared |
| Provider fails | timer set to now; provider skipped for `cooldown_secs` |
| Provider fails again during cooldown | timer reset (extends cooldown) |
| All providers cooling down | `AllProvidersFailed` returned to client |
| `cooldown_enabled = false` | tracker calls are no-ops; behavior identical to before |
| Config reload | Engine rebuilt, tracker unchanged, cooldown state preserved |

## Tests

- `health.rs` unit tests: is_cooling_down, record_failure, record_success, expiry
- `fallback.rs` unit tests: skips cooling provider, tries next, all cooling → error
- `router/mod.rs` integration test: tracker survives reload

## Log Examples

```
WARN provider="github_copilot" remaining_secs=54 Provider is cooling down, skipping
WARN provider="openai" cooldown_secs=60 Provider failed, entering cooldown: 429 rate limited
INFO provider="anthropic" Successfully routed to provider (fallback index: 1)
```
