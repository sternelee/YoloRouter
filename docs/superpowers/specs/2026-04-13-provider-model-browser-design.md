# Design: Provider Model Browser & Scenario Editor

**Date:** 2026-04-13  
**Status:** Awaiting approval  

## Summary

Add an interactive model browser to the Providers tab of the TUI. The user presses Enter on a provider to fetch its available models via live API, selects a model, assigns a cost tier, picks or creates a scenario, then the change is persisted to `config.toml` and immediately applied to the running daemon via a new `/control/reload` HTTP endpoint.

---

## Requirements

1. In the Providers tab, pressing Enter on a provider triggers a live model list fetch.
2. The user navigates the model list and presses Enter to select a model.
3. The user selects a cost tier: `low`, `medium`, or `high`.
4. The user picks an existing scenario or creates a new one by name.
5. The model entry (`{ provider, model, cost_tier }`) is appended to the scenario's model list.
6. The change is written to `config.toml`.
7. A `POST /control/reload` request is sent to the daemon so the change takes effect without restart.
8. Esc at any step returns to the previous step.

---

## Architecture

### 1. ProviderViewState (new enum in `src/tui/mod.rs`)

Controls what the right-hand detail pane of the Providers tab renders:

```rust
enum ProviderViewState {
    /// Default: shows provider config details
    ProviderDetail,

    /// Async model fetch in progress
    FetchingModels,

    /// Model list ready, user navigates
    ModelList {
        models: Vec<String>,
        selected: usize,
    },

    /// User picks cost tier
    CostTierPicker {
        model: String,
        selected: usize,   // 0=low, 1=medium, 2=high
    },

    /// User picks existing scenario or creates new
    ScenarioPicker {
        model: String,
        cost_tier: String,
        selected: usize,          // 0..n = existing scenarios, n+1 = "[+ New Scenario]"
        creating_new: bool,       // true = typing new name
        new_name_input: String,
    },

    /// Confirmation message (any key → ProviderDetail)
    Done { message: String },

    /// Error message (Esc → ProviderDetail)
    Error { message: String },
}
```

`TuiApp` gains a `provider_view: ProviderViewState` field (default: `ProviderDetail`).

### 2. Model Fetch Channel

Two new channels added to `TuiApp` and `TuiManager`:

| Channel | Type | Direction | Purpose |
|---|---|---|---|
| `model_req_tx` | `tokio::sync::mpsc::Sender<ModelFetchRequest>` | TUI → background task | Request model list |
| `model_res_rx` | `std::sync::mpsc::Receiver<ModelFetchResult>` | background task → TUI | Deliver results |

`ModelFetchRequest` carries `{ provider_name: String, config: ProviderConfig }`.  
`ModelFetchResult` is `Result<Vec<String>, String>` (model names or error message).

The existing `OverrideCommand` background task is extended to also handle reload commands. A second background tokio task handles model fetch requests.

### 3. Model Fetch Logic (`src/provider/models.rs`, new file)

```rust
pub async fn fetch_provider_models(cfg: &ProviderConfig) -> Result<Vec<String>, String>
```

Per provider type:

| `provider_type` | Method |
|---|---|
| `openai` or OpenAI-compatible (has `base_url`) | `GET {base_url}/models` with `Authorization: Bearer {api_key}` → parse `data[].id` |
| `anthropic` | No public `/models` endpoint → fall back to hardcoded list |
| `gemini` | `GET https://generativelanguage.googleapis.com/v1beta/models?key={api_key}` → parse `models[].name` |
| `github_copilot` / `codex_oauth` | Fall back to hardcoded `model_list()` |

All network errors are caught and returned as `Err(String)` (no panics).

### 4. Keyboard Flow (Providers Tab)

```
State: ProviderDetail
  Down/Up/j/k  → move provider selection (unchanged)
  Enter        → send ModelFetchRequest; transition to FetchingModels
  Esc          → no-op

State: FetchingModels
  (any key)    → no-op, waiting for result
  Esc          → cancel (back to ProviderDetail, result ignored when it arrives)

State: ModelList
  Down/Up/j/k  → move model selection
  Enter        → transition to CostTierPicker { model }
  Esc          → back to ProviderDetail

State: CostTierPicker
  Down/Up/j/k  → move tier selection (low / medium / high)
  Enter        → transition to ScenarioPicker { model, cost_tier }
  Esc          → back to ModelList

State: ScenarioPicker (browsing)
  Down/Up/j/k  → move scenario selection
  Enter on existing scenario → add_model(); write_config(); send Reload; Done/Error
  Enter on "[+ New Scenario]" → creating_new = true (switch to input mode)
  Esc          → back to CostTierPicker

State: ScenarioPicker (creating_new = true)
  Char(c)      → append to new_name_input
  Backspace    → pop from new_name_input
  Enter        → create scenario; add_model(); write_config(); send Reload; Done/Error
  Esc          → creating_new = false (back to browsing)

State: Done
  (any key)    → ProviderDetail; update app.scenario_names

State: Error
  (any key)    → ProviderDetail
```

### 5. Config Mutation

Two methods added to `Config` (in `src/config/mod.rs`):

```rust
/// Append a model entry to an existing scenario.
pub fn add_model_to_scenario(
    &mut self,
    scenario_name: &str,
    provider: &str,
    model: &str,
    cost_tier: &str,
) -> Result<()>

/// Create a new scenario containing one model entry.
pub fn add_scenario(
    &mut self,
    scenario_name: &str,
    provider: &str,
    model: &str,
    cost_tier: &str,
) -> Result<()>
```

Serialization: `config.to_string()` already exists and emits TOML. The result is written with `std::fs::write(config_path, content)`.

### 6. Daemon Reload Endpoint (`src/server/mod.rs`)

New route: `POST /control/reload`

```
Handler:
  1. Read config_path from AppState (stored at startup)
  2. Config::from_file(config_path)? 
  3. *state.config.write().await = new_config
  4. Return 200 { "status": "reloaded" }
  5. On error: 500 { "error": "..." }
```

`AppState` gains a `config_path: String` field.

### 7. Reload Command

`OverrideCommand` becomes a new `ControlCommand` enum:

```rust
enum ControlCommand {
    Override { endpoint: String, scenario: Option<String> },
    Reload,
}
```

After writing `config.toml`, the TUI sends `ControlCommand::Reload` via the existing `cmd_tx`. The background task POSTs to `/control/reload` and sends "✅ 已实时生效" or "⚠️ daemon 离线，重启后生效" back on `status_tx`.

---

## Files Changed

| File | Change |
|---|---|
| `src/tui/mod.rs` | Add `ProviderViewState`, `ModelFetchRequest/Result`, channels, keyboard handler updates, drawing updates |
| `src/provider/models.rs` | New: `fetch_provider_models()` |
| `src/provider/mod.rs` | Re-export `models` module |
| `src/config/mod.rs` | Add `add_model_to_scenario()`, `add_scenario()` |
| `src/server/mod.rs` | Add `POST /control/reload`, add `config_path` to `AppState` |
| `src/main.rs` | Pass `config_path` to `start_server()` |

---

## Error Handling

- Network errors during model fetch → `Error { message }` state, user can Esc and retry.
- Invalid scenario name (empty string) → show inline hint "Name cannot be empty".
- Config write failure → `Error { message }` state.
- Daemon offline during reload → `Done { "⚠️ 已写入 config.toml，daemon 离线，重启后生效" }` (not an error, degraded success).

---

## Out of Scope

- Removing models from scenarios via TUI (future).
- Reordering models within a scenario (future).
- Editing provider API keys via TUI (future).
- Streaming model lists as they arrive (all-or-nothing fetch).
