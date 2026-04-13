# Provider Model Browser & Scenario Editor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an interactive model browser to the Providers TUI tab — press Enter on a provider to fetch models via live API, pick a model + cost tier + scenario, persist to config.toml, and hot-reload the running daemon.

**Architecture:** A new `ProviderViewState` enum drives the right-hand pane of the Providers tab through a 6-step state machine (ProviderDetail → FetchingModels → ModelList → CostTierPicker → ScenarioPicker → Done/Error). Model fetches run in a background tokio task via a channel pair. A new `POST /control/reload` daemon endpoint allows hot config reload without restart.

**Tech Stack:** Rust, ratatui 0.26, crossterm, actix-web 4, reqwest, tokio, serde/toml

---

## File Map

| File                     | Action | Responsibility                                                     |
| ------------------------ | ------ | ------------------------------------------------------------------ |
| `src/config/parser.rs`   | Modify | Add `add_model_to_scenario()`, `add_scenario()`                    |
| `src/provider/models.rs` | Create | `fetch_provider_models()` — live API per provider type             |
| `src/provider/mod.rs`    | Modify | `pub mod models;`                                                  |
| `src/server/mod.rs`      | Modify | Add `config_path` to `AppState`, `POST /control/reload` handler    |
| `src/main.rs`            | Modify | Pass `config_path` to `start_server()`                             |
| `src/tui/mod.rs`         | Modify | `ProviderViewState`, `ControlCommand`, channels, keyboard, drawing |

---

## Task 1: Config Mutation Methods

**Files:**

- Modify: `src/config/parser.rs`

- [ ] **Step 1: Write failing tests**

Add at the bottom of `src/config/parser.rs`:

```rust
#[cfg(test)]
mod mutation_tests {
    use super::*;

    fn base_config() -> Config {
        Config::from_string(r#"
[daemon]
port = 8989

[providers.openai]
type = "openai"
api_key = "sk-test"

[scenarios.coding]
models = [
    { provider = "openai", model = "gpt-4", cost_tier = "high" }
]

[routing]
fallback_enabled = true
"#).unwrap()
    }

    #[test]
    fn test_add_model_to_existing_scenario() {
        let mut cfg = base_config();
        cfg.add_model_to_scenario("coding", "openai", "gpt-4o", "medium").unwrap();
        let scenarios = cfg.scenarios();
        let models = &scenarios["coding"].models;
        assert_eq!(models.len(), 2);
        assert_eq!(models[1].model, "gpt-4o");
        assert_eq!(models[1].provider, "openai");
        assert_eq!(models[1].cost_tier.as_deref(), Some("medium"));
    }

    #[test]
    fn test_add_model_to_missing_scenario_errors() {
        let mut cfg = base_config();
        let result = cfg.add_model_to_scenario("nonexistent", "openai", "gpt-4o", "low");
        assert!(result.is_err());
    }

    #[test]
    fn test_add_scenario_creates_new() {
        let mut cfg = base_config();
        cfg.add_scenario("budget", "openai", "gpt-3.5-turbo", "low").unwrap();
        let scenarios = cfg.scenarios();
        assert!(scenarios.contains_key("budget"));
        assert_eq!(scenarios["budget"].models.len(), 1);
        assert_eq!(scenarios["budget"].models[0].model, "gpt-3.5-turbo");
        assert_eq!(scenarios["budget"].models[0].cost_tier.as_deref(), Some("low"));
    }

    #[test]
    fn test_add_scenario_duplicate_errors() {
        let mut cfg = base_config();
        let result = cfg.add_scenario("coding", "openai", "gpt-4o", "high");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_round_trips_after_mutation() {
        let mut cfg = base_config();
        cfg.add_model_to_scenario("coding", "openai", "gpt-4o", "medium").unwrap();
        let toml_str = cfg.to_string().unwrap();
        let reloaded = Config::from_string(&toml_str).unwrap();
        assert_eq!(reloaded.scenarios()["coding"].models.len(), 2);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd /Users/sternelee/www/github/YoloRouter
cargo test mutation_tests -- --nocapture 2>&1 | tail -20
```

Expected: compile error — `add_model_to_scenario` and `add_scenario` not found.

- [ ] **Step 3: Implement the methods**

In `src/config/parser.rs`, add after the `get_provider` method and before the `#[cfg(test)]` block:

```rust
    /// Append a model entry to an existing scenario's model list.
    pub fn add_model_to_scenario(
        &mut self,
        scenario_name: &str,
        provider: &str,
        model: &str,
        cost_tier: &str,
    ) -> Result<()> {
        use crate::config::schema::ModelConfig;
        let scenarios = self.scenarios
            .get_or_insert_with(std::collections::HashMap::new);
        let scenario = scenarios.get_mut(scenario_name).ok_or_else(|| {
            YoloRouterError::ConfigError(format!("Scenario '{}' not found", scenario_name))
        })?;
        scenario.models.push(ModelConfig {
            provider: provider.to_string(),
            model: model.to_string(),
            cost_tier: Some(cost_tier.to_string()),
            capabilities: None,
            fallback_to: None,
        });
        Ok(())
    }

    /// Create a new scenario with one initial model entry.
    pub fn add_scenario(
        &mut self,
        scenario_name: &str,
        provider: &str,
        model: &str,
        cost_tier: &str,
    ) -> Result<()> {
        use crate::config::schema::{ModelConfig, ScenarioConfig};
        let scenarios = self.scenarios
            .get_or_insert_with(std::collections::HashMap::new);
        if scenarios.contains_key(scenario_name) {
            return Err(YoloRouterError::ConfigError(format!(
                "Scenario '{}' already exists", scenario_name
            )));
        }
        scenarios.insert(scenario_name.to_string(), ScenarioConfig {
            models: vec![ModelConfig {
                provider: provider.to_string(),
                model: model.to_string(),
                cost_tier: Some(cost_tier.to_string()),
                capabilities: None,
                fallback_to: None,
            }],
            default_tier: None,
            match_task_types: vec![],
            match_languages: vec![],
            priority: 0,
            is_default: false,
        });
        Ok(())
    }
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test mutation_tests -- --nocapture 2>&1 | tail -10
```

Expected: `5 passed`

- [ ] **Step 5: Commit**

```bash
git add src/config/parser.rs
git commit -m "feat(config): add add_model_to_scenario and add_scenario mutation methods"
```

---

## Task 2: Provider Model Fetch

**Files:**

- Create: `src/provider/models.rs`
- Modify: `src/provider/mod.rs`

- [ ] **Step 1: Write failing test**

Create `src/provider/models.rs` with just the test first:

```rust
use crate::config::schema::ProviderConfig;
use std::collections::HashMap;

/// Fetch available model IDs for a provider via its API.
/// Falls back to a hardcoded list for providers without a /models endpoint.
pub async fn fetch_provider_models(cfg: &ProviderConfig) -> Result<Vec<String>, String> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cfg(provider_type: &str, api_key: Option<&str>, base_url: Option<&str>) -> ProviderConfig {
        ProviderConfig {
            provider_type: provider_type.to_string(),
            api_key: api_key.map(str::to_string),
            auth_type: None,
            token: None,
            base_url: base_url.map(str::to_string),
            extra: HashMap::new(),
        }
    }

    #[test]
    fn test_anthropic_returns_hardcoded() {
        let cfg = make_cfg("anthropic", Some("sk-ant-test"), None);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let models = rt.block_on(fetch_provider_models(&cfg)).unwrap();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.contains("claude")));
    }

    #[test]
    fn test_github_copilot_returns_hardcoded() {
        let cfg = make_cfg("github_copilot", None, None);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let models = rt.block_on(fetch_provider_models(&cfg)).unwrap();
        assert!(!models.is_empty());
    }
}
```

- [ ] **Step 2: Add module to `src/provider/mod.rs`**

```rust
pub mod models;
```

Add this line after `pub mod factory;`.

- [ ] **Step 3: Run test to verify it fails**

```bash
cargo test provider::models::tests -- --nocapture 2>&1 | tail -10
```

Expected: panics with `not yet implemented` (todo!()).

- [ ] **Step 4: Implement `fetch_provider_models`**

Replace the `todo!()` body in `src/provider/models.rs`:

```rust
use crate::config::schema::ProviderConfig;
use std::collections::HashMap;

pub async fn fetch_provider_models(cfg: &ProviderConfig) -> Result<Vec<String>, String> {
    match cfg.provider_type.as_str() {
        "anthropic" => Ok(vec![
            "claude-opus-4-5".to_string(),
            "claude-sonnet-4-5".to_string(),
            "claude-haiku-4-5".to_string(),
            "claude-opus-4".to_string(),
            "claude-sonnet-4".to_string(),
            "claude-3-5-sonnet-20241022".to_string(),
            "claude-3-5-haiku-20241022".to_string(),
            "claude-3-opus-20240229".to_string(),
        ]),

        "github_copilot" => Ok(vec![
            "gpt-4o".to_string(),
            "gpt-4o-mini".to_string(),
            "gpt-4".to_string(),
            "o1-preview".to_string(),
            "o1-mini".to_string(),
            "claude-3.5-sonnet".to_string(),
            "gemini-1.5-pro".to_string(),
        ]),

        "codex_oauth" => Ok(vec![
            "gpt-4o".to_string(),
            "gpt-4o-mini".to_string(),
            "o1".to_string(),
            "o1-mini".to_string(),
            "o3-mini".to_string(),
        ]),

        "gemini" => {
            let api_key = cfg.api_key.as_deref().unwrap_or("");
            if api_key.is_empty() {
                return Ok(vec![
                    "gemini-2.0-flash".to_string(),
                    "gemini-1.5-pro".to_string(),
                    "gemini-1.5-flash".to_string(),
                ]);
            }
            fetch_gemini_models(api_key).await
        }

        // openai and all OpenAI-compatible providers (openrouter, groq, deepseek, etc.)
        _ => {
            let base_url = cfg.base_url.as_deref()
                .unwrap_or("https://api.openai.com/v1");
            let api_key = cfg.api_key.as_deref().unwrap_or("");
            if api_key.is_empty() {
                return Err("API key not configured for this provider".to_string());
            }
            fetch_openai_compatible_models(base_url, api_key).await
        }
    }
}

async fn fetch_openai_compatible_models(base_url: &str, api_key: &str) -> Result<Vec<String>, String> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("API error: HTTP {}", resp.status()));
    }

    let json: serde_json::Value = resp.json().await
        .map_err(|e| format!("Parse error: {}", e))?;

    let models: Vec<String> = json["data"]
        .as_array()
        .ok_or("Unexpected response format: missing 'data' array")?
        .iter()
        .filter_map(|m| m["id"].as_str().map(str::to_string))
        .collect();

    if models.is_empty() {
        return Err("No models returned by API".to_string());
    }

    let mut sorted = models;
    sorted.sort();
    Ok(sorted)
}

async fn fetch_gemini_models(api_key: &str) -> Result<Vec<String>, String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={}",
        api_key
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client.get(&url).send().await
        .map_err(|e| format!("Network error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Gemini API error: HTTP {}", resp.status()));
    }

    let json: serde_json::Value = resp.json().await
        .map_err(|e| format!("Parse error: {}", e))?;

    let models: Vec<String> = json["models"]
        .as_array()
        .ok_or("Unexpected response format: missing 'models' array")?
        .iter()
        .filter_map(|m| {
            m["name"].as_str().map(|n| {
                // Strip "models/" prefix: "models/gemini-1.5-pro" → "gemini-1.5-pro"
                n.trim_start_matches("models/").to_string()
            })
        })
        .filter(|name| name.starts_with("gemini"))
        .collect();

    let mut sorted = models;
    sorted.sort();
    Ok(sorted)
}
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cargo test provider::models::tests -- --nocapture 2>&1 | tail -10
```

Expected: `2 passed`

- [ ] **Step 6: Commit**

```bash
git add src/provider/models.rs src/provider/mod.rs
git commit -m "feat(provider): add fetch_provider_models with live API support"
```

---

## Task 3: Daemon Reload Endpoint

**Files:**

- Modify: `src/server/mod.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Add `config_path` to `AppState` and `start_server`**

In `src/server/mod.rs`, replace the `AppState` struct and `start_server` signature:

```rust
pub struct AppState {
    pub config: Arc<RwLock<crate::Config>>,
    pub router: Arc<Router>,
    pub stats: Arc<StatsCollector>,
    pub overrides: OverrideMap,
    pub config_path: String,
}

pub async fn start_server(port: u16, config: crate::Config, config_path: String) -> Result<()> {
    let routing_engine = RoutingEngine::new_with_config(config.clone())?;
    let router = Arc::new(Router::new(routing_engine));
    let stats = Arc::new(StatsCollector::new());
    let overrides: OverrideMap = Arc::new(RwLock::new(HashMap::new()));

    let app_state = web::Data::new(AppState {
        config: Arc::new(RwLock::new(config)),
        router,
        stats,
        overrides,
        config_path,
    });
```

- [ ] **Step 2: Add the reload route**

In `start_server`, add the route in the `App::new()` chain after `/control/override/{endpoint}`:

```rust
            .route("/control/reload", web::post().to(control_reload))
```

- [ ] **Step 3: Add the reload handler**

Add this function after `control_clear_override` in `src/server/mod.rs`:

```rust
async fn control_reload(state: web::Data<AppState>) -> Result<HttpResponse> {
    match crate::Config::from_file(&state.config_path) {
        Ok(new_config) => {
            *state.config.write().await = new_config;
            tracing::info!("Config hot-reloaded from {}", state.config_path);
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "status": "reloaded",
                "config_path": state.config_path,
            })))
        }
        Err(e) => {
            tracing::error!("Config reload failed: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e.to_string(),
            })))
        }
    }
}
```

- [ ] **Step 4: Update `main.rs` to pass `config_path`**

In `src/main.rs`, find the `start_server` call and add `config_path.clone()`:

```rust
    server::start_server(daemon_config.port, config, config_path.clone()).await?;
```

- [ ] **Step 5: Verify it compiles**

```bash
cargo check 2>&1 | grep -E "^error" | head -20
```

Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add src/server/mod.rs src/main.rs
git commit -m "feat(server): add POST /control/reload for hot config reload"
```

---

## Task 4: Refactor OverrideCommand → ControlCommand

**Files:**

- Modify: `src/tui/mod.rs`

- [ ] **Step 1: Replace `OverrideCommand` with `ControlCommand`**

In `src/tui/mod.rs`, replace:

```rust
// ─── Override command sent from TUI to background HTTP task ──────────────────

#[derive(Debug)]
pub struct OverrideCommand {
    pub endpoint: String,
    pub scenario: Option<String>, // None = reset to auto
}
```

with:

```rust
// ─── Commands sent from TUI to background HTTP task ──────────────────────────

#[derive(Debug)]
pub enum ControlCommand {
    Override { endpoint: String, scenario: Option<String> },
    Reload,
}
```

- [ ] **Step 2: Update `TuiApp` field**

In `TuiApp` struct, change:

```rust
    cmd_tx: tokio::sync::mpsc::Sender<OverrideCommand>,
```

to:

```rust
    cmd_tx: tokio::sync::mpsc::Sender<ControlCommand>,
```

Update `TuiApp::new` signature and body accordingly (same field name, new type).

- [ ] **Step 3: Update `TuiManager::run` background task**

Replace the background task that handles `OverrideCommand`:

```rust
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            while let Some(cmd) = cmd_rx.recv().await {
                let result = match cmd {
                    ControlCommand::Override { endpoint, scenario } => {
                        let url = format!("http://127.0.0.1:{}/control/override", port);
                        let body = serde_json::json!({
                            "endpoint": endpoint,
                            "scenario": scenario,
                        });
                        match client
                            .post(&url)
                            .json(&body)
                            .timeout(Duration::from_secs(2))
                            .send()
                            .await
                        {
                            Ok(resp) if resp.status().is_success() => {
                                let label = scenario.as_deref().unwrap_or("auto");
                                format!("✅ {} → {}", endpoint, label)
                            }
                            Ok(resp) => format!("❌ Override failed: HTTP {}", resp.status()),
                            Err(e) => {
                                if e.is_timeout() || e.is_connect() {
                                    "⚠️  Daemon offline — start daemon first (yolo-router)".to_string()
                                } else {
                                    format!("❌ {}", e)
                                }
                            }
                        }
                    }
                    ControlCommand::Reload => {
                        let url = format!("http://127.0.0.1:{}/control/reload", port);
                        match client
                            .post(&url)
                            .timeout(Duration::from_secs(3))
                            .send()
                            .await
                        {
                            Ok(resp) if resp.status().is_success() => {
                                "✅ 已实时生效".to_string()
                            }
                            Ok(_) | Err(_) => {
                                "⚠️  已写入 config.toml，daemon 离线，重启后生效".to_string()
                            }
                        }
                    }
                };
                let _ = status_tx.send(result);
            }
        });
```

- [ ] **Step 4: Update call sites**

In `handle_tab_key` (Scenarios branch), update `OverrideCommand { ... }` to `ControlCommand::Override { ... }`:

```rust
                    let cmd = ControlCommand::Override {
                        endpoint: "global".to_string(),
                        scenario: Some(name.clone()),
                    };
```

And for the `'a'` reset:

```rust
                    let cmd = ControlCommand::Override {
                        endpoint: "global".to_string(),
                        scenario: None,
                    };
```

- [ ] **Step 5: Verify compilation**

```bash
cargo check 2>&1 | grep "^error" | head -20
```

Expected: no errors.

- [ ] **Step 6: Run tests**

```bash
cargo test --lib 2>&1 | tail -5
```

Expected: 30 passed (same as before).

- [ ] **Step 7: Commit**

```bash
git add src/tui/mod.rs
git commit -m "refactor(tui): OverrideCommand → ControlCommand enum with Override + Reload variants"
```

---

## Task 5: TUI — ProviderViewState + Model Fetch Channel

**Files:**

- Modify: `src/tui/mod.rs`

- [ ] **Step 1: Add the types**

Add after the `ControlCommand` enum in `src/tui/mod.rs`:

```rust
// ─── Model fetch channel types ────────────────────────────────────────────────

#[derive(Debug)]
pub struct ModelFetchRequest {
    pub provider_name: String,
    pub config: crate::config::schema::ProviderConfig,
}

// ─── Provider tab view state ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ProviderViewState {
    ProviderDetail,
    FetchingModels,
    ModelList {
        models: Vec<String>,
        selected: usize,
    },
    CostTierPicker {
        model: String,
        selected: usize,
    },
    ScenarioPicker {
        model: String,
        cost_tier: String,
        selected: usize,
        creating_new: bool,
        new_name_input: String,
    },
    Done { message: String },
    Error { message: String },
}
```

- [ ] **Step 2: Add fields to `TuiApp`**

In the `TuiApp` struct, add:

```rust
    provider_view: ProviderViewState,
    model_req_tx: tokio::sync::mpsc::Sender<ModelFetchRequest>,
    model_res_rx: std::sync::mpsc::Receiver<Result<Vec<String>, String>>,
```

In `TuiApp::new`, add the new fields (channels are passed in):

```rust
    fn new(
        config: Config,
        config_path: String,
        cmd_tx: tokio::sync::mpsc::Sender<ControlCommand>,
        status_rx: std::sync::mpsc::Receiver<String>,
        model_req_tx: tokio::sync::mpsc::Sender<ModelFetchRequest>,
        model_res_rx: std::sync::mpsc::Receiver<Result<Vec<String>, String>>,
    ) -> Self {
        // ... existing fields ...
        Self {
            // existing fields unchanged
            provider_view: ProviderViewState::ProviderDetail,
            model_req_tx,
            model_res_rx,
            // ...
        }
    }
```

- [ ] **Step 3: Add model fetch background task in `TuiManager::run`**

Add after the existing background task spawn, before `spawn_blocking`:

```rust
        // Background task: fetch model lists from provider APIs
        let (model_req_tx, mut model_req_rx) =
            tokio::sync::mpsc::channel::<ModelFetchRequest>(4);
        let (model_res_tx, model_res_rx) =
            std::sync::mpsc::channel::<Result<Vec<String>, String>>();

        tokio::spawn(async move {
            while let Some(req) = model_req_rx.recv().await {
                let result = crate::provider::models::fetch_provider_models(&req.config).await;
                let _ = model_res_tx.send(result);
            }
        });
```

- [ ] **Step 4: Pass new channels into `run_tui` and `TuiApp::new`**

In the `spawn_blocking` closure, pass the new channels:

```rust
        tokio::task::spawn_blocking(move || {
            if let Err(e) = run_tui(config, config_path, cmd_tx, status_rx, model_req_tx, model_res_rx) {
                eprintln!("TUI error: {e}");
            }
        })
```

Update `run_tui` signature:

```rust
fn run_tui(
    config: Config,
    config_path: String,
    cmd_tx: tokio::sync::mpsc::Sender<ControlCommand>,
    status_rx: std::sync::mpsc::Receiver<String>,
    model_req_tx: tokio::sync::mpsc::Sender<ModelFetchRequest>,
    model_res_rx: std::sync::mpsc::Receiver<Result<Vec<String>, String>>,
) -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = TuiApp::new(config, config_path, cmd_tx, status_rx, model_req_tx, model_res_rx);
    let result = event_loop(&mut terminal, &mut app);
    restore_terminal(&mut terminal);
    result
}
```

- [ ] **Step 5: Poll model results in `event_loop`**

In `event_loop`, inside the status drain loop at the top, add model result polling:

```rust
        // Drain model fetch results
        while let Ok(result) = app.model_res_rx.try_recv() {
            match &app.provider_view {
                ProviderViewState::FetchingModels => {
                    app.provider_view = match result {
                        Ok(models) if models.is_empty() => ProviderViewState::Error {
                            message: "No models returned by this provider".to_string(),
                        },
                        Ok(models) => ProviderViewState::ModelList { models, selected: 0 },
                        Err(e) => ProviderViewState::Error { message: e },
                    };
                }
                // Result arrived after user cancelled (Esc) — discard
                _ => {}
            }
        }
```

- [ ] **Step 6: Verify compilation**

```bash
cargo check 2>&1 | grep "^error" | head -20
```

Expected: no errors.

- [ ] **Step 7: Commit**

```bash
git add src/tui/mod.rs
git commit -m "feat(tui): add ProviderViewState enum and model fetch channels"
```

---

## Task 6: TUI — Providers Tab Keyboard Handling

**Files:**

- Modify: `src/tui/mod.rs`

- [ ] **Step 1: Replace the Providers branch in `handle_tab_key`**

Replace the entire `ActiveTab::Providers => { ... }` block:

```rust
        ActiveTab::Providers => {
            let providers: Vec<_> = app.config.providers().keys().cloned().collect();
            match &app.provider_view.clone() {
                ProviderViewState::ProviderDetail => match key {
                    KeyCode::Down | KeyCode::Char('j') => {
                        let i = app.provider_list_state.selected().unwrap_or(0);
                        let next = if providers.is_empty() { 0 } else { (i + 1) % providers.len() };
                        app.provider_list_state.select(Some(next));
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        let i = app.provider_list_state.selected().unwrap_or(0);
                        let prev = if i == 0 { providers.len().saturating_sub(1) } else { i - 1 };
                        app.provider_list_state.select(Some(prev));
                    }
                    KeyCode::Enter => {
                        if let Some(idx) = app.provider_list_state.selected() {
                            if let Some(name) = providers.get(idx).cloned() {
                                if let Some(cfg) = app.config.providers().get(&name).cloned() {
                                    let _ = app.model_req_tx.try_send(ModelFetchRequest {
                                        provider_name: name,
                                        config: cfg,
                                    });
                                    app.provider_view = ProviderViewState::FetchingModels;
                                }
                            }
                        }
                    }
                    _ => {}
                },

                ProviderViewState::FetchingModels => {
                    if key == KeyCode::Esc {
                        app.provider_view = ProviderViewState::ProviderDetail;
                    }
                }

                ProviderViewState::ModelList { models, selected } => {
                    let models = models.clone();
                    let selected = *selected;
                    match key {
                        KeyCode::Down | KeyCode::Char('j') => {
                            let next = if models.is_empty() { 0 } else { (selected + 1) % models.len() };
                            app.provider_view = ProviderViewState::ModelList { models, selected: next };
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            let prev = if selected == 0 { models.len().saturating_sub(1) } else { selected - 1 };
                            app.provider_view = ProviderViewState::ModelList { models, selected: prev };
                        }
                        KeyCode::Enter => {
                            if let Some(model) = models.get(selected).cloned() {
                                app.provider_view = ProviderViewState::CostTierPicker {
                                    model,
                                    selected: 1, // default: medium
                                };
                            }
                        }
                        KeyCode::Esc => {
                            app.provider_view = ProviderViewState::ProviderDetail;
                        }
                        _ => {}
                    }
                }

                ProviderViewState::CostTierPicker { model, selected } => {
                    let model = model.clone();
                    let selected = *selected;
                    match key {
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.provider_view = ProviderViewState::CostTierPicker {
                                model, selected: (selected + 1) % 3,
                            };
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.provider_view = ProviderViewState::CostTierPicker {
                                model, selected: if selected == 0 { 2 } else { selected - 1 },
                            };
                        }
                        KeyCode::Enter => {
                            let cost_tier = ["low", "medium", "high"][selected].to_string();
                            let mut scenario_names: Vec<String> =
                                app.config.scenarios().keys().cloned().collect();
                            scenario_names.sort();
                            app.provider_view = ProviderViewState::ScenarioPicker {
                                model,
                                cost_tier,
                                selected: 0,
                                creating_new: false,
                                new_name_input: String::new(),
                            };
                        }
                        KeyCode::Esc => {
                            // Go back to the model list; need to re-fetch or store it.
                            // Simplest: return to ProviderDetail and user re-enters.
                            app.provider_view = ProviderViewState::ProviderDetail;
                        }
                        _ => {}
                    }
                }

                ProviderViewState::ScenarioPicker {
                    model, cost_tier, selected, creating_new, new_name_input,
                } => {
                    let model = model.clone();
                    let cost_tier = cost_tier.clone();
                    let selected = *selected;
                    let creating_new = *creating_new;
                    let new_name_input = new_name_input.clone();

                    let mut scenario_names: Vec<String> =
                        app.config.scenarios().keys().cloned().collect();
                    scenario_names.sort();
                    // last item is "[+ New Scenario]"
                    let total = scenario_names.len() + 1;
                    let new_scenario_idx = scenario_names.len();

                    if creating_new {
                        match key {
                            KeyCode::Char(c) => {
                                let mut input = new_name_input;
                                input.push(c);
                                app.provider_view = ProviderViewState::ScenarioPicker {
                                    model, cost_tier, selected, creating_new: true,
                                    new_name_input: input,
                                };
                            }
                            KeyCode::Backspace => {
                                let mut input = new_name_input;
                                input.pop();
                                app.provider_view = ProviderViewState::ScenarioPicker {
                                    model, cost_tier, selected, creating_new: true,
                                    new_name_input: input,
                                };
                            }
                            KeyCode::Enter => {
                                if new_name_input.trim().is_empty() {
                                    app.status_message = "⚠️  Scenario name cannot be empty".to_string();
                                } else {
                                    let provider_name = providers
                                        .get(app.provider_list_state.selected().unwrap_or(0))
                                        .cloned()
                                        .unwrap_or_default();
                                    commit_model_to_scenario(
                                        app,
                                        &provider_name,
                                        &model,
                                        &cost_tier,
                                        new_name_input.trim(),
                                        true,
                                    );
                                }
                            }
                            KeyCode::Esc => {
                                app.provider_view = ProviderViewState::ScenarioPicker {
                                    model, cost_tier, selected,
                                    creating_new: false,
                                    new_name_input: String::new(),
                                };
                            }
                            _ => {}
                        }
                    } else {
                        match key {
                            KeyCode::Down | KeyCode::Char('j') => {
                                app.provider_view = ProviderViewState::ScenarioPicker {
                                    model, cost_tier,
                                    selected: (selected + 1) % total,
                                    creating_new: false, new_name_input,
                                };
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                app.provider_view = ProviderViewState::ScenarioPicker {
                                    model, cost_tier,
                                    selected: if selected == 0 { total - 1 } else { selected - 1 },
                                    creating_new: false, new_name_input,
                                };
                            }
                            KeyCode::Enter => {
                                let provider_name = providers
                                    .get(app.provider_list_state.selected().unwrap_or(0))
                                    .cloned()
                                    .unwrap_or_default();
                                if selected == new_scenario_idx {
                                    app.provider_view = ProviderViewState::ScenarioPicker {
                                        model, cost_tier, selected,
                                        creating_new: true,
                                        new_name_input: String::new(),
                                    };
                                } else if let Some(scenario_name) = scenario_names.get(selected) {
                                    commit_model_to_scenario(
                                        app,
                                        &provider_name,
                                        &model,
                                        &cost_tier,
                                        scenario_name,
                                        false,
                                    );
                                }
                            }
                            KeyCode::Esc => {
                                app.provider_view = ProviderViewState::CostTierPicker {
                                    model,
                                    selected: 1,
                                };
                            }
                            _ => {}
                        }
                    }
                }

                ProviderViewState::Done { .. } | ProviderViewState::Error { .. } => {
                    // any key returns to ProviderDetail
                    app.provider_view = ProviderViewState::ProviderDetail;
                    // refresh scenario list after a successful add
                    let mut names: Vec<String> = app.config.scenarios().keys().cloned().collect();
                    names.sort();
                    app.scenario_names = names;
                }
            }
        }
```

- [ ] **Step 2: Add `commit_model_to_scenario` helper**

Add before `handle_tab_key`:

```rust
/// Write model to config + send Reload command. Transitions provider_view to Done/Error.
fn commit_model_to_scenario(
    app: &mut TuiApp,
    provider_name: &str,
    model: &str,
    cost_tier: &str,
    scenario_name: &str,
    is_new: bool,
) {
    let result = if is_new {
        app.config.add_scenario(scenario_name, provider_name, model, cost_tier)
    } else {
        app.config.add_model_to_scenario(scenario_name, provider_name, model, cost_tier)
    };

    match result {
        Err(e) => {
            app.provider_view = ProviderViewState::Error { message: e.to_string() };
            return;
        }
        Ok(_) => {}
    }

    if let Err(e) = app.config.save_to_file(&app.config_path) {
        app.provider_view = ProviderViewState::Error {
            message: format!("Failed to write config.toml: {}", e),
        };
        return;
    }

    let _ = app.cmd_tx.try_send(ControlCommand::Reload);

    let action = if is_new { "创建并加入场景" } else { "加入场景" };
    app.provider_view = ProviderViewState::Done {
        message: format!("✅ {}/{} {} '{}'，正在通知 daemon…", provider_name, model, action, scenario_name),
    };
}
```

- [ ] **Step 3: Verify compilation**

```bash
cargo check 2>&1 | grep "^error" | head -20
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add src/tui/mod.rs
git commit -m "feat(tui): implement Providers tab keyboard state machine"
```

---

## Task 7: TUI — Providers Tab Drawing

**Files:**

- Modify: `src/tui/mod.rs`

- [ ] **Step 1: Replace `draw_providers`**

Replace the entire `draw_providers` function:

```rust
fn draw_providers(f: &mut ratatui::Frame, app: &mut TuiApp, area: ratatui::layout::Rect) {
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    // ── Left pane: provider list (unchanged) ──────────────────────────────────
    let providers = app.config.providers();
    let items: Vec<ListItem> = providers
        .keys()
        .map(|name| ListItem::new(name.as_str()))
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Providers "))
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, panes[0], &mut app.provider_list_state);

    // ── Right pane: driven by ProviderViewState ───────────────────────────────
    match &app.provider_view.clone() {
        ProviderViewState::ProviderDetail => {
            let provider_names: Vec<String> = providers.keys().cloned().collect();
            let detail_text = if let Some(idx) = app.provider_list_state.selected() {
                if let Some(name) = provider_names.get(idx) {
                    if let Some(cfg) = providers.get(name) {
                        vec![
                            Line::from(vec![
                                Span::styled("Name:      ", Style::default().fg(Color::Cyan)),
                                Span::raw(name.as_str()),
                            ]),
                            Line::from(vec![
                                Span::styled("Type:      ", Style::default().fg(Color::Cyan)),
                                Span::raw(&cfg.provider_type),
                            ]),
                            Line::from(vec![
                                Span::styled("API Key:   ", Style::default().fg(Color::Cyan)),
                                Span::raw(cfg.api_key.as_deref().map(|k| {
                                    if k.len() > 8 { format!("{}...{}", &k[..4], &k[k.len()-4..]) }
                                    else { "****".to_string() }
                                }).unwrap_or_else(|| "not set".to_string())),
                            ]),
                            Line::from(vec![
                                Span::styled("Auth Type: ", Style::default().fg(Color::Cyan)),
                                Span::raw(cfg.auth_type.as_deref().unwrap_or("api_key")),
                            ]),
                            Line::from(vec![
                                Span::styled("Base URL:  ", Style::default().fg(Color::Cyan)),
                                Span::raw(cfg.base_url.as_deref().unwrap_or("(default)")),
                            ]),
                            Line::from(""),
                            Line::from(Span::styled(
                                "  Enter — fetch model list",
                                Style::default().fg(Color::DarkGray),
                            )),
                        ]
                    } else { vec![Line::from("Select a provider")] }
                } else { vec![Line::from("Select a provider")] }
            } else { vec![Line::from("No providers configured")] };

            let detail = Paragraph::new(detail_text)
                .block(Block::default().borders(Borders::ALL).title(" Provider Details "))
                .alignment(Alignment::Left);
            f.render_widget(detail, panes[1]);
        }

        ProviderViewState::FetchingModels => {
            let para = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  ⠋ Fetching model list…",
                    Style::default().fg(Color::Yellow),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  Esc — cancel",
                    Style::default().fg(Color::DarkGray),
                )),
            ])
            .block(Block::default().borders(Borders::ALL).title(" Models "));
            f.render_widget(para, panes[1]);
        }

        ProviderViewState::ModelList { models, selected } => {
            let items: Vec<ListItem> = models
                .iter()
                .map(|m| ListItem::new(m.as_str()))
                .collect();
            let mut state = ListState::default();
            state.select(Some(*selected));
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL)
                    .title(format!(" Models ({}) — Enter=select  Esc=back ", models.len())))
                .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
                .highlight_symbol("▶ ");
            f.render_stateful_widget(list, panes[1], &mut state);
        }

        ProviderViewState::CostTierPicker { model, selected } => {
            let tiers = [
                ("low",    "适合快速、便宜的任务"),
                ("medium", "平衡性价比"),
                ("high",   "最强能力，成本最高"),
            ];
            let mut lines = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("Model: ", Style::default().fg(Color::Cyan)),
                    Span::styled(model.as_str(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(""),
                Line::from(Span::styled("选择 cost tier：", Style::default().fg(Color::Cyan))),
                Line::from(""),
            ];
            for (i, (tier, desc)) in tiers.iter().enumerate() {
                let is_sel = i == *selected;
                let prefix = if is_sel { "▶ " } else { "  " };
                let style = if is_sel {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                lines.push(Line::from(Span::styled(
                    format!("{}[{}]  {}", prefix, tier, desc),
                    style,
                )));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Enter=confirm  Esc=back",
                Style::default().fg(Color::DarkGray),
            )));
            let para = Paragraph::new(lines)
                .block(Block::default().borders(Borders::ALL).title(" Cost Tier "));
            f.render_widget(para, panes[1]);
        }

        ProviderViewState::ScenarioPicker {
            model, cost_tier, selected, creating_new, new_name_input,
        } => {
            let mut scenario_names: Vec<String> =
                app.config.scenarios().keys().cloned().collect();
            scenario_names.sort();
            let new_scenario_idx = scenario_names.len();

            let mut lines = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("Model: ", Style::default().fg(Color::Cyan)),
                    Span::styled(
                        format!("{} [{}]", model, cost_tier),
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(""),
                Line::from(Span::styled("选择或新建场景：", Style::default().fg(Color::Cyan))),
                Line::from(""),
            ];

            for (i, name) in scenario_names.iter().enumerate() {
                let is_sel = i == *selected && !creating_new;
                let prefix = if is_sel { "▶ " } else { "  " };
                let style = if is_sel {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                lines.push(Line::from(Span::styled(format!("{}{}", prefix, name), style)));
            }

            // "[+ New Scenario]" item
            if *creating_new {
                lines.push(Line::from(Span::styled(
                    format!("▶ [+ New Scenario]: {}_", new_name_input),
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                )));
            } else {
                let is_sel = *selected == new_scenario_idx;
                let prefix = if is_sel { "▶ " } else { "  " };
                let style = if is_sel {
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Green)
                };
                lines.push(Line::from(Span::styled(
                    format!("{}[+ New Scenario]", prefix),
                    style,
                )));
            }

            lines.push(Line::from(""));
            if *creating_new {
                lines.push(Line::from(Span::styled(
                    "  输入场景名称，Enter=确认  Esc=取消",
                    Style::default().fg(Color::DarkGray),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    "  Enter=选择  Esc=back",
                    Style::default().fg(Color::DarkGray),
                )));
            }

            let para = Paragraph::new(lines)
                .block(Block::default().borders(Borders::ALL).title(" Select Scenario "));
            f.render_widget(para, panes[1]);
        }

        ProviderViewState::Done { message } => {
            let para = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    message.as_str(),
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  任意键返回",
                    Style::default().fg(Color::DarkGray),
                )),
            ])
            .block(Block::default().borders(Borders::ALL).title(" Done "));
            f.render_widget(para, panes[1]);
        }

        ProviderViewState::Error { message } => {
            let para = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("  ✗ {}", message),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  任意键返回",
                    Style::default().fg(Color::DarkGray),
                )),
            ])
            .block(Block::default().borders(Borders::ALL).title(" Error "));
            f.render_widget(para, panes[1]);
        }
    }
}
```

- [ ] **Step 2: Update Help tab text**

In `draw_help`, add under the keyboard shortcuts section:

```rust
        Line::from("  Enter            — [Providers] Fetch model list for selected provider"),
        Line::from("  Enter            — [Models]    Select model → cost tier → scenario"),
        Line::from("  Esc              — [Providers] Go back one step"),
```

- [ ] **Step 3: Full compile + test**

```bash
cargo test 2>&1 | tail -10
```

Expected: 37 passed.

- [ ] **Step 4: Commit**

```bash
git add src/tui/mod.rs
git commit -m "feat(tui): draw Providers tab model browser and scenario picker UI"
```

---

## Task 8: Final Integration Test

- [ ] **Step 1: Build release binary**

```bash
cargo build --release 2>&1 | tail -5
```

Expected: `Finished release [optimized]`.

- [ ] **Step 2: Smoke-test model fetch (offline providers)**

```bash
cargo test provider::models::tests -- --nocapture
```

Expected: 2 passed (anthropic + github_copilot hardcoded lists).

- [ ] **Step 3: Verify all tests still pass**

```bash
cargo test 2>&1 | tail -5
```

Expected: 37 passed.

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "feat: provider model browser — Enter on provider → fetch models → add to scenario + hot reload"
```
