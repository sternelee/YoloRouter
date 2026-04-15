pub mod auth;
pub mod codex_auth;
pub mod config_editor;
pub mod github_auth;

pub use auth::AuthFlow;

use crate::config::{schema::ProviderConfig, Config};
use crate::provider::codex_oauth::CodexQuotaInfo;
use crate::provider::models::{
    codex_quota_rows, codex_quota_ttl_ms, fetch_provider_quota, is_codex_oauth_provider,
    now_epoch_ms, should_refresh_quota,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs},
    Terminal,
};
use std::collections::HashMap;
use std::io;
use std::sync::mpsc;
use std::time::Duration;

// ─── Tab Index ───────────────────────────────────────────────────────────────

const TAB_NAMES: &[&str] = &["Status", "Providers", "Scenarios", "Auth", "Help"];

#[derive(Debug, Clone, PartialEq)]
enum ActiveTab {
    Status = 0,
    Providers = 1,
    Scenarios = 2,
    Auth = 3,
    Help = 4,
}

impl ActiveTab {
    fn next(&self) -> Self {
        match self {
            ActiveTab::Status => ActiveTab::Providers,
            ActiveTab::Providers => ActiveTab::Scenarios,
            ActiveTab::Scenarios => ActiveTab::Auth,
            ActiveTab::Auth => ActiveTab::Help,
            ActiveTab::Help => ActiveTab::Status,
        }
    }
    fn prev(&self) -> Self {
        match self {
            ActiveTab::Status => ActiveTab::Help,
            ActiveTab::Providers => ActiveTab::Status,
            ActiveTab::Scenarios => ActiveTab::Providers,
            ActiveTab::Auth => ActiveTab::Scenarios,
            ActiveTab::Help => ActiveTab::Auth,
        }
    }
    fn index(&self) -> usize {
        self.clone() as usize
    }
}

// ─── Commands sent from TUI to background HTTP task ──────────────────────────

#[derive(Debug)]
pub enum ControlCommand {
    Override {
        endpoint: String,
        scenario: Option<String>,
    },
    Reload,
}

// ─── Model fetch channel types ────────────────────────────────────────────────

pub struct ModelFetchRequest {
    pub provider_name: String,
    pub config: ProviderConfig,
}

pub struct QuotaFetchRequest {
    pub provider_name: String,
    pub config: ProviderConfig,
}

#[derive(Debug, Clone)]
enum ProviderQuotaState {
    Loading,
    Ready(CodexQuotaInfo),
    Error(String),
}

const CODEX_QUOTA_REFRESH_KEY: char = 'r';
const CODEX_QUOTA_LOADING_MESSAGE: &str = "Loading Codex quota…";
const CODEX_QUOTA_EMPTY_MESSAGE: &str = "No quota windows returned";
const CODEX_QUOTA_SECTION_TITLE: &str = "Quota / Usage";
const CODEX_QUOTA_HELP_LINE: &str = "  Enter — fetch model list";
const CODEX_QUOTA_REFRESH_LINE: &str = "  r — refresh quota";
const PROVIDER_DETAIL_SELECT_MESSAGE: &str = "Select a provider";
const PROVIDER_DETAIL_EMPTY_MESSAGE: &str = "No providers configured";
const QUOTA_ERROR_PREFIX: &str = "Quota error: ";
const QUOTA_UPDATED_PREFIX: &str = "Updated ";
const QUOTA_UPDATED_JUST_NOW: &str = "just now";
const QUOTA_UPDATED_MIN_SUFFIX: &str = "m ago";
const QUOTA_ROW_SEPARATOR: &str = " • ";
const DEFAULT_STATUS_MESSAGE: &str =
    "Ready. Tab/Shift+Tab: switch tabs  |  Scenarios: Enter=pin, a=auto  |  q=quit";
const MODEL_LIST_EMPTY_MESSAGE: &str = "No models returned by this provider";
const PROVIDER_DETAILS_TITLE: &str = " Provider Details ";
const MODELS_TITLE: &str = " Models ";
const CONTROL_CHANNEL_CAPACITY: usize = 16;
const MODEL_FETCH_CHANNEL_CAPACITY: usize = 4;
const QUOTA_FETCH_CHANNEL_CAPACITY: usize = 4;
const UI_POLL_INTERVAL_MS: u64 = 200;
const CONTROL_TIMEOUT_OVERRIDE_SECS: u64 = 2;
const CONTROL_TIMEOUT_RELOAD_SECS: u64 = 3;

// ─── Provider tab view state ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ProviderViewState {
    ProviderDetail,
    FetchingModels,
    ModelList {
        models: Vec<String>,
        selected: usize,
        search_query: String,
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
    Done {
        message: String,
    },
    Error {
        message: String,
    },
}

// ─── TuiApp State ────────────────────────────────────────────────────────────

struct TuiApp {
    tab: ActiveTab,
    config: Config,
    config_path: String,
    provider_list_state: ListState,
    scenario_list_state: ListState,
    /// Sorted, stable scenario names for index-stable selection
    scenario_names: Vec<String>,
    status_message: String,
    /// Active override label for display (updated via HTTP result channel)
    active_override: Option<String>,
    /// Send override commands to the async HTTP task
    cmd_tx: tokio::sync::mpsc::Sender<ControlCommand>,
    /// Receive results/status messages from the async HTTP task
    status_rx: mpsc::Receiver<String>,
    /// Current view state for the Providers tab
    provider_view: ProviderViewState,
    /// Send model fetch requests to the background async task
    model_req_tx: tokio::sync::mpsc::Sender<ModelFetchRequest>,
    /// Receive model fetch results from the background async task
    model_res_rx: std::sync::mpsc::Receiver<Result<Vec<String>, String>>,
    /// Send quota fetch requests to the background async task
    quota_req_tx: tokio::sync::mpsc::Sender<QuotaFetchRequest>,
    /// Receive quota fetch results from the background async task
    quota_res_rx: std::sync::mpsc::Receiver<(String, Result<CodexQuotaInfo, String>)>,
    /// Cached quota state keyed by provider name
    quota_state_by_provider: HashMap<String, ProviderQuotaState>,
}

fn quota_ttl_ms() -> i64 {
    let ttl = codex_quota_ttl_ms();
    if ttl > 0 {
        ttl
    } else {
        5 * 60 * 1000
    }
}

fn quota_last_queried_at(state: Option<&ProviderQuotaState>) -> Option<i64> {
    match state {
        Some(ProviderQuotaState::Ready(quota)) => Some(quota.queried_at_ms),
        _ => None,
    }
}

fn format_quota_updated(queried_at_ms: i64) -> String {
    let age_minutes = now_epoch_ms().saturating_sub(queried_at_ms) / 60_000;
    if age_minutes <= 0 {
        format!("{}{}", QUOTA_UPDATED_PREFIX, QUOTA_UPDATED_JUST_NOW)
    } else {
        format!(
            "{}{}{}",
            QUOTA_UPDATED_PREFIX, age_minutes, QUOTA_UPDATED_MIN_SUFFIX
        )
    }
}

fn maybe_fetch_selected_provider_quota(app: &mut TuiApp, force: bool) {
    let provider_names = sorted_provider_names(&app.config);
    let Some(idx) = app.provider_list_state.selected() else {
        return;
    };
    let Some(provider_name) = provider_names.get(idx).cloned() else {
        return;
    };
    let Ok(cfg) = app.config.get_provider(&provider_name) else {
        return;
    };
    if !is_codex_oauth_provider(&cfg) {
        return;
    }

    let should_fetch = if force {
        true
    } else {
        match app.quota_state_by_provider.get(&provider_name) {
            Some(ProviderQuotaState::Loading) => false,
            state => {
                should_refresh_quota(quota_last_queried_at(state), now_epoch_ms(), quota_ttl_ms())
            }
        }
    };

    if should_fetch {
        app.quota_state_by_provider
            .insert(provider_name.clone(), ProviderQuotaState::Loading);
        let _ = app.quota_req_tx.try_send(QuotaFetchRequest {
            provider_name,
            config: cfg,
        });
    }
}

fn quota_detail_lines(app: &TuiApp, provider_name: &str) -> Vec<Line<'static>> {
    match app.quota_state_by_provider.get(provider_name) {
        Some(ProviderQuotaState::Loading) => {
            vec![Line::from(format!("  {}", CODEX_QUOTA_LOADING_MESSAGE))]
        }
        Some(ProviderQuotaState::Error(message)) => {
            vec![Line::from(format!("  {}{}", QUOTA_ERROR_PREFIX, message))]
        }
        Some(ProviderQuotaState::Ready(quota)) => {
            let mut lines: Vec<Line<'static>> = codex_quota_rows(quota, now_epoch_ms())
                .into_iter()
                .map(|(window, usage, reset)| {
                    Line::from(format!(
                        "  {}{}{}{}reset {}",
                        window, QUOTA_ROW_SEPARATOR, usage, QUOTA_ROW_SEPARATOR, reset
                    ))
                })
                .collect();
            if lines.is_empty() {
                lines.push(Line::from(format!("  {}", CODEX_QUOTA_EMPTY_MESSAGE)));
            }
            lines.push(Line::from(format!(
                "  {}",
                format_quota_updated(quota.queried_at_ms)
            )));
            lines
        }
        None => vec![Line::from(format!("  {}", CODEX_QUOTA_LOADING_MESSAGE))],
    }
}

fn provider_detail_lines(
    app: &TuiApp,
    provider_name: &str,
    cfg: &ProviderConfig,
) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Name:      ", Style::default().fg(Color::Cyan)),
            Span::raw(provider_name.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Type:      ", Style::default().fg(Color::Cyan)),
            Span::raw(cfg.provider_type.clone()),
        ]),
        Line::from(vec![
            Span::styled("API Key:   ", Style::default().fg(Color::Cyan)),
            Span::raw(
                cfg.api_key
                    .as_deref()
                    .map(|k| {
                        if k.len() > 8 {
                            format!("{}...{}", &k[..4], &k[k.len() - 4..])
                        } else {
                            "****".to_string()
                        }
                    })
                    .unwrap_or_else(|| "not set".to_string()),
            ),
        ]),
        Line::from(vec![
            Span::styled("Auth Type: ", Style::default().fg(Color::Cyan)),
            Span::raw(cfg.auth_type.as_deref().unwrap_or("api_key").to_string()),
        ]),
        Line::from(vec![
            Span::styled("Base URL:  ", Style::default().fg(Color::Cyan)),
            Span::raw(cfg.base_url.as_deref().unwrap_or("(default)").to_string()),
        ]),
        Line::from(Span::styled(
            CODEX_QUOTA_HELP_LINE,
            Style::default().fg(Color::DarkGray),
        )),
    ];

    if is_codex_oauth_provider(cfg) {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  {}", CODEX_QUOTA_SECTION_TITLE),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        lines.extend(quota_detail_lines(app, provider_name));
        lines.push(Line::from(Span::styled(
            CODEX_QUOTA_REFRESH_LINE,
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines
}

fn sorted_provider_names(config: &Config) -> Vec<String> {
    let mut provider_names: Vec<String> = config.providers().keys().cloned().collect();
    provider_names.sort();
    provider_names
}

fn selected_provider_name(app: &TuiApp) -> Option<String> {
    sorted_provider_names(&app.config)
        .get(app.provider_list_state.selected().unwrap_or(0))
        .cloned()
}

fn refresh_provider_names_after_selection_change(app: &mut TuiApp) {
    maybe_fetch_selected_provider_quota(app, false);
}

fn apply_quota_result(
    app: &mut TuiApp,
    provider_name: String,
    result: Result<CodexQuotaInfo, String>,
) {
    let state = match result {
        Ok(quota) => ProviderQuotaState::Ready(quota),
        Err(error) => ProviderQuotaState::Error(error),
    };
    let status_message = match &state {
        ProviderQuotaState::Ready(quota) => Some(format_quota_updated(quota.queried_at_ms)),
        ProviderQuotaState::Error(message) => Some(format!("{}{}", QUOTA_ERROR_PREFIX, message)),
        ProviderQuotaState::Loading => None,
    };
    app.quota_state_by_provider.insert(provider_name, state);
    if let Some(message) = status_message {
        app.status_message = message;
    }
}

fn queue_model_fetch_for_selected_provider(app: &mut TuiApp, provider_name: String) {
    if let Ok(cfg) = app.config.get_provider(&provider_name) {
        let _ = app.model_req_tx.try_send(ModelFetchRequest {
            provider_name,
            config: cfg,
        });
        app.provider_view = ProviderViewState::FetchingModels;
    } else {
        app.provider_view = ProviderViewState::Error {
            message: format!("Failed to load provider '{}' config", provider_name),
        };
    }
}

fn maybe_force_refresh_selected_provider_quota(app: &mut TuiApp, key: KeyCode) {
    if matches!(key, KeyCode::Char(c) if c == CODEX_QUOTA_REFRESH_KEY) {
        maybe_fetch_selected_provider_quota(app, true);
    }
}

fn provider_detail_text(app: &TuiApp) -> Vec<Line<'static>> {
    let providers = app.config.providers();
    if let Some(name) = selected_provider_name(app) {
        if let Some(cfg) = providers.get(&name) {
            provider_detail_lines(app, &name, cfg)
        } else {
            vec![Line::from(PROVIDER_DETAIL_SELECT_MESSAGE)]
        }
    } else if providers.is_empty() {
        vec![Line::from(PROVIDER_DETAIL_EMPTY_MESSAGE)]
    } else {
        vec![Line::from(PROVIDER_DETAIL_SELECT_MESSAGE)]
    }
}

fn model_loading_paragraph() -> Paragraph<'static> {
    Paragraph::new(vec![
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
    .block(Block::default().borders(Borders::ALL).title(MODELS_TITLE))
}

fn provider_list_items(app: &TuiApp) -> Vec<ListItem<'static>> {
    sorted_provider_names(&app.config)
        .into_iter()
        .map(ListItem::new)
        .collect()
}

fn update_provider_selection(app: &mut TuiApp, index: usize) {
    app.provider_list_state.select(Some(index));
    refresh_provider_names_after_selection_change(app);
}

fn move_provider_selection(app: &mut TuiApp, delta: isize) {
    let provider_names = sorted_provider_names(&app.config);
    if provider_names.is_empty() {
        app.provider_list_state.select(Some(0));
        return;
    }

    let current = app.provider_list_state.selected().unwrap_or(0) as isize;
    let next = (current + delta).rem_euclid(provider_names.len() as isize) as usize;
    update_provider_selection(app, next);
}

// ─── Provider tab view state ──────────────────────────────────────────────────

impl TuiApp {
    fn new(
        config: Config,
        config_path: String,
        cmd_tx: tokio::sync::mpsc::Sender<ControlCommand>,
        status_rx: mpsc::Receiver<String>,
        model_req_tx: tokio::sync::mpsc::Sender<ModelFetchRequest>,
        model_res_rx: std::sync::mpsc::Receiver<Result<Vec<String>, String>>,
        quota_req_tx: tokio::sync::mpsc::Sender<QuotaFetchRequest>,
        quota_res_rx: std::sync::mpsc::Receiver<(String, Result<CodexQuotaInfo, String>)>,
    ) -> Self {
        let mut provider_list_state = ListState::default();
        provider_list_state.select(Some(0));
        let mut scenario_list_state = ListState::default();
        scenario_list_state.select(Some(0));

        let mut scenario_names: Vec<String> = config.scenarios().keys().cloned().collect();
        scenario_names.sort();

        let mut app = Self {
            tab: ActiveTab::Status,
            config,
            config_path,
            provider_list_state,
            scenario_list_state,
            scenario_names,
            status_message: DEFAULT_STATUS_MESSAGE.to_string(),
            active_override: None,
            cmd_tx,
            status_rx,
            provider_view: ProviderViewState::ProviderDetail,
            model_req_tx,
            model_res_rx,
            quota_req_tx,
            quota_res_rx,
            quota_state_by_provider: HashMap::new(),
        };
        refresh_provider_names_after_selection_change(&mut app);
        app
    }
}

// ─── TuiManager ──────────────────────────────────────────────────────────────

pub struct TuiManager;

impl Default for TuiManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiManager {
    pub fn new() -> Self {
        Self
    }

    pub async fn run(&self, config: Config, config_path: String) {
        let port = config.daemon().port;
        let (cmd_tx, mut cmd_rx) =
            tokio::sync::mpsc::channel::<ControlCommand>(CONTROL_CHANNEL_CAPACITY);
        let (status_tx, status_rx) = mpsc::channel::<String>();

        // Background async task: receives control commands, sends HTTP to daemon
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
                            .timeout(Duration::from_secs(CONTROL_TIMEOUT_OVERRIDE_SECS))
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
                                    "⚠️  Daemon offline — start daemon first (yolo-router)"
                                        .to_string()
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
                            .timeout(Duration::from_secs(CONTROL_TIMEOUT_RELOAD_SECS))
                            .send()
                            .await
                        {
                            Ok(resp) if resp.status().is_success() => "✅ 已实时生效".to_string(),
                            Ok(resp) => {
                                let status = resp.status();
                                let body = resp.text().await.unwrap_or_default();
                                if status.is_client_error() || status.is_server_error() {
                                    format!("❌ 热重载失败: HTTP {} {}", status, body)
                                } else {
                                    format!("⚠️  热重载状态异常: HTTP {}", status)
                                }
                            }
                            Err(_) => "⚠️  已写入 config.toml，daemon 离线，重启后生效".to_string(),
                        }
                    }
                };
                let _ = status_tx.send(result);
            }
        });

        // Background task: fetch model lists from provider APIs
        let (model_req_tx, mut model_req_rx) =
            tokio::sync::mpsc::channel::<ModelFetchRequest>(MODEL_FETCH_CHANNEL_CAPACITY);
        let (model_res_tx, model_res_rx) =
            std::sync::mpsc::channel::<Result<Vec<String>, String>>();

        tokio::spawn(async move {
            while let Some(req) = model_req_rx.recv().await {
                let _provider_name = req.provider_name;
                let result = crate::provider::models::fetch_provider_models(&req.config).await;
                let _ = model_res_tx.send(result);
            }
        });

        let (quota_req_tx, mut quota_req_rx) =
            tokio::sync::mpsc::channel::<QuotaFetchRequest>(QUOTA_FETCH_CHANNEL_CAPACITY);
        let (quota_res_tx, quota_res_rx) =
            std::sync::mpsc::channel::<(String, Result<CodexQuotaInfo, String>)>();

        tokio::spawn(async move {
            while let Some(req) = quota_req_rx.recv().await {
                let provider_name = req.provider_name;
                let result = fetch_provider_quota(&req.config).await;
                let _ = quota_res_tx.send((provider_name, result));
            }
        });

        // Run blocking TUI in a dedicated OS thread
        tokio::task::spawn_blocking(move || {
            if let Err(e) = run_tui(
                config,
                config_path,
                cmd_tx,
                status_rx,
                model_req_tx,
                model_res_rx,
                quota_req_tx,
                quota_res_rx,
            ) {
                eprintln!("TUI error: {e}");
            }
        })
        .await
        .ok();
    }
}

// ─── Terminal setup/teardown ──────────────────────────────────────────────────

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) {
    let _ = disable_raw_mode();
    let _ = execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    );
    let _ = terminal.show_cursor();
}

// ─── Main event loop ─────────────────────────────────────────────────────────

fn run_tui(
    config: Config,
    config_path: String,
    cmd_tx: tokio::sync::mpsc::Sender<ControlCommand>,
    status_rx: mpsc::Receiver<String>,
    model_req_tx: tokio::sync::mpsc::Sender<ModelFetchRequest>,
    model_res_rx: std::sync::mpsc::Receiver<Result<Vec<String>, String>>,
    quota_req_tx: tokio::sync::mpsc::Sender<QuotaFetchRequest>,
    quota_res_rx: std::sync::mpsc::Receiver<(String, Result<CodexQuotaInfo, String>)>,
) -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = TuiApp::new(
        config,
        config_path,
        cmd_tx,
        status_rx,
        model_req_tx,
        model_res_rx,
        quota_req_tx,
        quota_res_rx,
    );
    let result = event_loop(&mut terminal, &mut app);
    restore_terminal(&mut terminal);
    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut TuiApp,
) -> io::Result<()> {
    loop {
        // Drain any HTTP result messages before redraw
        while let Ok(msg) = app.status_rx.try_recv() {
            // Parse override label from success message (e.g. "✅ global → coding")
            if msg.starts_with("✅") {
                if let Some(arrow) = msg.find('→') {
                    app.active_override = Some(msg[arrow + 2..].trim().to_string());
                }
            }
            app.status_message = msg;
        }

        // Drain model fetch results
        while let Ok(result) = app.model_res_rx.try_recv() {
            if matches!(&app.provider_view, ProviderViewState::FetchingModels) {
                app.provider_view = match result {
                    Ok(models) if models.is_empty() => ProviderViewState::Error {
                        message: MODEL_LIST_EMPTY_MESSAGE.to_string(),
                    },
                    Ok(models) => ProviderViewState::ModelList {
                        models,
                        selected: 0,
                        search_query: String::new(),
                    },
                    Err(e) => ProviderViewState::Error { message: e },
                };
            }
        }

        while let Ok((provider_name, result)) = app.quota_res_rx.try_recv() {
            apply_quota_result(app, provider_name, result);
        }

        terminal.draw(|f| draw_ui(f, app))?;

        // Non-blocking poll so the UI stays responsive and status_rx is checked regularly
        if event::poll(Duration::from_millis(UI_POLL_INTERVAL_MS))? {
            if let Event::Key(key) = event::read()? {
                match (key.code, key.modifiers) {
                    (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => break,
                    (KeyCode::Tab, _) => app.tab = app.tab.next(),
                    (KeyCode::BackTab, _) => app.tab = app.tab.prev(),
                    (KeyCode::Char('1'), _) => app.tab = ActiveTab::Status,
                    (KeyCode::Char('2'), _) => app.tab = ActiveTab::Providers,
                    (KeyCode::Char('3'), _) => app.tab = ActiveTab::Scenarios,
                    (KeyCode::Char('4'), _) => app.tab = ActiveTab::Auth,
                    (KeyCode::Char('5'), _) => app.tab = ActiveTab::Help,
                    _ => handle_tab_key(app, key.code),
                }
            }
        }
    }
    Ok(())
}

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
        app.config
            .add_scenario(scenario_name, provider_name, model, cost_tier)
    } else {
        app.config
            .add_model_to_scenario(scenario_name, provider_name, model, cost_tier)
    };

    if let Err(e) = result {
        app.provider_view = ProviderViewState::Error {
            message: e.to_string(),
        };
        return;
    }

    if let Err(e) = app.config.save_to_file(&app.config_path) {
        app.provider_view = ProviderViewState::Error {
            message: format!("Failed to write config.toml: {}", e),
        };
        return;
    }

    let _ = app.cmd_tx.try_send(ControlCommand::Reload);

    let action = if is_new {
        "创建并加入场景"
    } else {
        "加入场景"
    };
    app.provider_view = ProviderViewState::Done {
        message: format!(
            "✅ {}/{} {} '{}'，正在通知 daemon…",
            provider_name, model, action, scenario_name
        ),
    };
}

fn handle_tab_key(app: &mut TuiApp, key: KeyCode) {
    match app.tab {
        ActiveTab::Providers => {
            let providers = sorted_provider_names(&app.config);
            match app.provider_view.clone() {
                ProviderViewState::ProviderDetail => match key {
                    KeyCode::Down | KeyCode::Char('j') => move_provider_selection(app, 1),
                    KeyCode::Up | KeyCode::Char('k') => move_provider_selection(app, -1),
                    KeyCode::Enter => {
                        if let Some(name) = selected_provider_name(app) {
                            queue_model_fetch_for_selected_provider(app, name);
                        }
                    }
                    KeyCode::Char(c) if c == CODEX_QUOTA_REFRESH_KEY => {
                        maybe_force_refresh_selected_provider_quota(app, key);
                    }
                    _ => {}
                },

                ProviderViewState::FetchingModels => {
                    if key == KeyCode::Esc {
                        app.provider_view = ProviderViewState::ProviderDetail;
                    }
                }

                ProviderViewState::ModelList {
                    models,
                    selected,
                    search_query,
                } => {
                    // 计算搜索过滤后的模型列表
                    let filtered_models: Vec<&String> = if search_query.is_empty() {
                        models.iter().collect()
                    } else {
                        models
                            .iter()
                            .filter(|m| m.to_lowercase().contains(&search_query.to_lowercase()))
                            .collect()
                    };

                    match key {
                        KeyCode::Down => {
                            let next = if filtered_models.is_empty() {
                                0
                            } else {
                                (selected + 1) % filtered_models.len()
                            };
                            app.provider_view = ProviderViewState::ModelList {
                                models,
                                selected: next,
                                search_query,
                            };
                        }
                        KeyCode::Up => {
                            let prev = if selected == 0 {
                                filtered_models.len().saturating_sub(1)
                            } else {
                                selected - 1
                            };
                            app.provider_view = ProviderViewState::ModelList {
                                models,
                                selected: prev,
                                search_query,
                            };
                        }
                        KeyCode::Char('j') => {
                            let next = if filtered_models.is_empty() {
                                0
                            } else {
                                (selected + 1) % filtered_models.len()
                            };
                            app.provider_view = ProviderViewState::ModelList {
                                models,
                                selected: next,
                                search_query,
                            };
                        }
                        KeyCode::Char('k') => {
                            let prev = if selected == 0 {
                                filtered_models.len().saturating_sub(1)
                            } else {
                                selected - 1
                            };
                            app.provider_view = ProviderViewState::ModelList {
                                models,
                                selected: prev,
                                search_query,
                            };
                        }
                        KeyCode::Char(c) => {
                            // 输入搜索字符
                            let mut query = search_query;
                            query.push(c);
                            app.provider_view = ProviderViewState::ModelList {
                                models,
                                selected: 0,
                                search_query: query,
                            };
                        }
                        KeyCode::Backspace => {
                            // 删除搜索字符
                            let mut query = search_query;
                            query.pop();
                            app.provider_view = ProviderViewState::ModelList {
                                models,
                                selected: 0,
                                search_query: query,
                            };
                        }
                        KeyCode::Enter => {
                            if let Some(model) = filtered_models.get(selected) {
                                app.provider_view = ProviderViewState::CostTierPicker {
                                    model: model.to_string(),
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

                ProviderViewState::CostTierPicker { model, selected } => match key {
                    KeyCode::Down | KeyCode::Char('j') => {
                        app.provider_view = ProviderViewState::CostTierPicker {
                            model,
                            selected: (selected + 1) % 3,
                        };
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        app.provider_view = ProviderViewState::CostTierPicker {
                            model,
                            selected: if selected == 0 { 2 } else { selected - 1 },
                        };
                    }
                    KeyCode::Enter => {
                        let cost_tier = ["low", "medium", "high"][selected].to_string();
                        app.provider_view = ProviderViewState::ScenarioPicker {
                            model,
                            cost_tier,
                            selected: 0,
                            creating_new: false,
                            new_name_input: String::new(),
                        };
                    }
                    KeyCode::Esc => {
                        app.provider_view = ProviderViewState::ProviderDetail;
                    }
                    _ => {}
                },

                ProviderViewState::ScenarioPicker {
                    model,
                    cost_tier,
                    selected,
                    creating_new,
                    new_name_input,
                } => {
                    let mut scenario_names: Vec<String> =
                        app.config.scenarios().keys().cloned().collect();
                    scenario_names.sort();
                    let new_scenario_idx = scenario_names.len();
                    let total = scenario_names.len() + 1;

                    if creating_new {
                        match key {
                            KeyCode::Char(c) => {
                                let mut input = new_name_input;
                                input.push(c);
                                app.provider_view = ProviderViewState::ScenarioPicker {
                                    model,
                                    cost_tier,
                                    selected,
                                    creating_new: true,
                                    new_name_input: input,
                                };
                            }
                            KeyCode::Backspace => {
                                let mut input = new_name_input;
                                input.pop();
                                app.provider_view = ProviderViewState::ScenarioPicker {
                                    model,
                                    cost_tier,
                                    selected,
                                    creating_new: true,
                                    new_name_input: input,
                                };
                            }
                            KeyCode::Enter => {
                                if new_name_input.trim().is_empty() {
                                    app.status_message =
                                        "⚠️  Scenario name cannot be empty".to_string();
                                } else {
                                    let provider_name = providers
                                        .get(app.provider_list_state.selected().unwrap_or(0))
                                        .cloned()
                                        .unwrap_or_default();
                                    let name = new_name_input.trim().to_string();
                                    commit_model_to_scenario(
                                        app,
                                        &provider_name,
                                        &model,
                                        &cost_tier,
                                        &name,
                                        true,
                                    );
                                }
                            }
                            KeyCode::Esc => {
                                app.provider_view = ProviderViewState::ScenarioPicker {
                                    model,
                                    cost_tier,
                                    selected,
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
                                    model,
                                    cost_tier,
                                    selected: (selected + 1) % total,
                                    creating_new: false,
                                    new_name_input,
                                };
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                app.provider_view = ProviderViewState::ScenarioPicker {
                                    model,
                                    cost_tier,
                                    selected: if selected == 0 {
                                        total - 1
                                    } else {
                                        selected - 1
                                    },
                                    creating_new: false,
                                    new_name_input,
                                };
                            }
                            KeyCode::Enter => {
                                let provider_name = providers
                                    .get(app.provider_list_state.selected().unwrap_or(0))
                                    .cloned()
                                    .unwrap_or_default();
                                if selected == new_scenario_idx {
                                    app.provider_view = ProviderViewState::ScenarioPicker {
                                        model,
                                        cost_tier,
                                        selected,
                                        creating_new: true,
                                        new_name_input: String::new(),
                                    };
                                } else if let Some(scenario_name) = scenario_names.get(selected) {
                                    let scenario_name = scenario_name.clone();
                                    commit_model_to_scenario(
                                        app,
                                        &provider_name,
                                        &model,
                                        &cost_tier,
                                        &scenario_name,
                                        false,
                                    );
                                }
                            }
                            KeyCode::Esc => {
                                app.provider_view =
                                    ProviderViewState::CostTierPicker { model, selected: 1 };
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
        ActiveTab::Scenarios => {
            let count = app.scenario_names.len();
            match key {
                KeyCode::Down | KeyCode::Char('j') => {
                    let i = app.scenario_list_state.selected().unwrap_or(0);
                    let next = if count == 0 { 0 } else { (i + 1) % count };
                    app.scenario_list_state.select(Some(next));
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    let i = app.scenario_list_state.selected().unwrap_or(0);
                    let prev = if i == 0 {
                        count.saturating_sub(1)
                    } else {
                        i - 1
                    };
                    app.scenario_list_state.select(Some(prev));
                }
                KeyCode::Enter => {
                    if let Some(i) = app.scenario_list_state.selected() {
                        if let Some(name) = app.scenario_names.get(i).cloned() {
                            let cmd = ControlCommand::Override {
                                endpoint: "global".to_string(),
                                scenario: Some(name.clone()),
                            };
                            match app.cmd_tx.try_send(cmd) {
                                Ok(_) => {
                                    app.status_message =
                                        format!("Sending override: global → {}…", name);
                                }
                                Err(_) => {
                                    app.status_message =
                                        "⚠️  Request queue full, try again".to_string();
                                }
                            }
                        }
                    }
                }
                KeyCode::Char('a') => {
                    let cmd = ControlCommand::Override {
                        endpoint: "global".to_string(),
                        scenario: None,
                    };
                    match app.cmd_tx.try_send(cmd) {
                        Ok(_) => {
                            app.status_message = "Sending reset: global → auto…".to_string();
                            app.active_override = Some("auto".to_string());
                        }
                        Err(_) => {
                            app.status_message = "⚠️  Request queue full, try again".to_string();
                        }
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

// ─── UI Rendering ─────────────────────────────────────────────────────────────

fn draw_ui(f: &mut ratatui::Frame, app: &mut TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tabs bar
            Constraint::Min(0),    // content
            Constraint::Length(1), // status bar
        ])
        .split(f.size());

    // Tab bar
    let titles: Vec<Line> = TAB_NAMES
        .iter()
        .enumerate()
        .map(|(i, t)| {
            Line::from(Span::styled(
                format!(" {}: {} ", i + 1, t),
                if i == app.tab.index() {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                },
            ))
        })
        .collect();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" YoloRouter TUI "),
        )
        .select(app.tab.index())
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, chunks[0]);

    // Content
    match app.tab {
        ActiveTab::Status => draw_status(f, app, chunks[1]),
        ActiveTab::Providers => draw_providers(f, app, chunks[1]),
        ActiveTab::Scenarios => draw_scenarios(f, app, chunks[1]),
        ActiveTab::Auth => draw_auth(f, app, chunks[1]),
        ActiveTab::Help => draw_help(f, chunks[1]),
    }

    // Status bar
    let status =
        Paragraph::new(app.status_message.as_str()).style(Style::default().fg(Color::DarkGray));
    f.render_widget(status, chunks[2]);
}

fn draw_status(f: &mut ratatui::Frame, app: &TuiApp, area: ratatui::layout::Rect) {
    let providers = app.config.providers();
    let scenarios = app.config.scenarios();
    let routing = app.config.routing();
    let daemon = app.config.daemon();

    let override_label = app.active_override.as_deref().unwrap_or("auto (analyzer)");

    let lines = vec![
        Line::from(vec![
            Span::styled("Config: ", Style::default().fg(Color::Cyan)),
            Span::raw(&app.config_path),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Daemon Port:   ", Style::default().fg(Color::Cyan)),
            Span::raw(format!(
                "127.0.0.1:{}  (POST /control/override to switch)",
                daemon.port
            )),
        ]),
        Line::from(vec![
            Span::styled("Log Level:     ", Style::default().fg(Color::Cyan)),
            Span::raw(&daemon.log_level),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Active Routing:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(override_label, Style::default().fg(Color::Green)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Providers:     ", Style::default().fg(Color::Cyan)),
            Span::raw(providers.len().to_string()),
        ]),
        Line::from(vec![
            Span::styled("Scenarios:     ", Style::default().fg(Color::Cyan)),
            Span::raw(scenarios.len().to_string()),
        ]),
        Line::from(vec![
            Span::styled("Fallback:      ", Style::default().fg(Color::Cyan)),
            Span::raw(if routing.fallback_enabled {
                "enabled"
            } else {
                "disabled"
            }),
        ]),
        Line::from(vec![
            Span::styled("Timeout:       ", Style::default().fg(Color::Cyan)),
            Span::raw(format!("{}ms", routing.timeout_ms)),
        ]),
        Line::from(vec![
            Span::styled("Retries:       ", Style::default().fg(Color::Cyan)),
            Span::raw(routing.retry_count.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Confidence:    ", Style::default().fg(Color::Cyan)),
            Span::raw(format!("{:.0}%", routing.confidence_threshold * 100.0)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Providers: ", Style::default().fg(Color::Green)),
            Span::raw(providers.keys().cloned().collect::<Vec<_>>().join(", ")),
        ]),
    ];

    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" Status "))
        .alignment(Alignment::Left);
    f.render_widget(para, area);
}

fn draw_providers(f: &mut ratatui::Frame, app: &mut TuiApp, area: ratatui::layout::Rect) {
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    let list = List::new(provider_list_items(app))
        .block(Block::default().borders(Borders::ALL).title(" Providers "))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, panes[0], &mut app.provider_list_state);

    match app.provider_view.clone() {
        ProviderViewState::ProviderDetail => {
            let detail = Paragraph::new(provider_detail_text(app))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(PROVIDER_DETAILS_TITLE),
                )
                .alignment(Alignment::Left);
            f.render_widget(detail, panes[1]);
        }
        ProviderViewState::FetchingModels => {
            f.render_widget(model_loading_paragraph(), panes[1]);
        }
        ProviderViewState::ModelList {
            models,
            selected,
            search_query,
        } => {
            let filtered_models: Vec<&String> = if search_query.is_empty() {
                models.iter().collect()
            } else {
                models
                    .iter()
                    .filter(|m| m.to_lowercase().contains(&search_query.to_lowercase()))
                    .collect()
            };

            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Search: ", Style::default().fg(Color::Cyan)),
                    Span::styled(search_query.as_str(), Style::default().fg(Color::Yellow)),
                    Span::raw("_"),
                ]),
                Line::from(""),
            ];

            for (i, model) in filtered_models.iter().enumerate() {
                let is_selected = i == selected;
                let prefix = if is_selected { "▶ " } else { "  " };
                let style = if is_selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                lines.push(Line::from(Span::styled(
                    format!("{}{}", prefix, model),
                    style,
                )));
            }

            if filtered_models.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  (no matches)",
                    Style::default().fg(Color::DarkGray),
                )));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Type to search  Enter=select  Esc=back",
                Style::default().fg(Color::DarkGray),
            )));

            let title = format!(" Models ({}/{}) ", filtered_models.len(), models.len());
            let para =
                Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(title));
            f.render_widget(para, panes[1]);
        }
        ProviderViewState::CostTierPicker { model, selected } => {
            let tiers = [
                ("low", "适合快速、便宜的任务"),
                ("medium", "平衡性价比"),
                ("high", "最强能力，成本最高"),
            ];
            let mut lines = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("Model: ", Style::default().fg(Color::Cyan)),
                    Span::styled(
                        model.as_str(),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "选择 cost tier：",
                    Style::default().fg(Color::Cyan),
                )),
                Line::from(""),
            ];
            for (i, (tier, desc)) in tiers.iter().enumerate() {
                let is_sel = i == selected;
                let prefix = if is_sel { "▶ " } else { "  " };
                let style = if is_sel {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
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
            model,
            cost_tier,
            selected,
            creating_new,
            new_name_input,
        } => {
            let mut scenario_names: Vec<String> = app.config.scenarios().keys().cloned().collect();
            scenario_names.sort();
            let new_scenario_idx = scenario_names.len();

            let mut lines = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("Model: ", Style::default().fg(Color::Cyan)),
                    Span::styled(
                        format!("{} [{}]", model, cost_tier),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "选择或新建场景：",
                    Style::default().fg(Color::Cyan),
                )),
                Line::from(""),
            ];

            for (i, name) in scenario_names.iter().enumerate() {
                let is_sel = i == selected && !creating_new;
                let prefix = if is_sel { "▶ " } else { "  " };
                let style = if is_sel {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                lines.push(Line::from(Span::styled(
                    format!("{}{}", prefix, name),
                    style,
                )));
            }

            if creating_new {
                lines.push(Line::from(Span::styled(
                    format!("▶ [+ New Scenario]: {}_", new_name_input),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )));
            } else {
                let is_sel = selected == new_scenario_idx;
                let prefix = if is_sel { "▶ " } else { "  " };
                let style = if is_sel {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Green)
                };
                lines.push(Line::from(Span::styled(
                    format!("{}[+ New Scenario]", prefix),
                    style,
                )));
            }

            lines.push(Line::from(""));
            if creating_new {
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

            let para = Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Select Scenario "),
            );
            f.render_widget(para, panes[1]);
        }
        ProviderViewState::Done { message } => {
            let para = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    message.as_str(),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
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

fn draw_scenarios(f: &mut ratatui::Frame, app: &mut TuiApp, area: ratatui::layout::Rect) {
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    let scenarios = app.config.scenarios();
    // Use stable sorted names from app state
    let items: Vec<ListItem> = app
        .scenario_names
        .iter()
        .map(|name| {
            let is_active = app
                .active_override
                .as_deref()
                .map(|o| o == name)
                .unwrap_or(false);
            let is_default = scenarios.get(name).map(|sc| sc.is_default).unwrap_or(false);
            let label = match (is_active, is_default) {
                (true, _) => format!("{name} ●"),
                (false, true) => format!("{name} [default]"),
                _ => name.clone(),
            };
            let style = if is_active {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(label).style(style)
        })
        .collect();

    let override_hint = app
        .active_override
        .as_deref()
        .map(|o| format!(" Scenarios  [active: {}] ", o))
        .unwrap_or_else(|| " Scenarios  [auto] ".to_string());

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(override_hint))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, panes[0], &mut app.scenario_list_state);

    let detail_lines = if let Some(idx) = app.scenario_list_state.selected() {
        if let Some(name) = app.scenario_names.get(idx) {
            if let Some(sc) = scenarios.get(name) {
                let mut lines = vec![
                    Line::from(vec![
                        Span::styled("Name:       ", Style::default().fg(Color::Cyan)),
                        Span::raw(name),
                    ]),
                    Line::from(vec![
                        Span::styled("Default:    ", Style::default().fg(Color::Cyan)),
                        Span::raw(if sc.is_default { "yes" } else { "no" }),
                    ]),
                    Line::from(vec![
                        Span::styled("Priority:   ", Style::default().fg(Color::Cyan)),
                        Span::raw(sc.priority.to_string()),
                    ]),
                    Line::from(vec![
                        Span::styled("Task Types: ", Style::default().fg(Color::Cyan)),
                        Span::raw(if sc.match_task_types.is_empty() {
                            "any".to_string()
                        } else {
                            sc.match_task_types.join(", ")
                        }),
                    ]),
                    Line::from(vec![
                        Span::styled("Languages:  ", Style::default().fg(Color::Cyan)),
                        Span::raw(if sc.match_languages.is_empty() {
                            "any".to_string()
                        } else {
                            sc.match_languages.join(", ")
                        }),
                    ]),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Models (fallback order):",
                        Style::default().fg(Color::Cyan),
                    )),
                ];
                for (i, m) in sc.models.iter().enumerate() {
                    let tier = m.cost_tier.as_deref().unwrap_or("?");
                    lines.push(Line::from(format!(
                        "  {}. {}/{} [{}]",
                        i + 1,
                        m.provider,
                        m.model,
                        tier
                    )));
                }
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "  Enter = pin global routing to this scenario",
                    Style::default().fg(Color::DarkGray),
                )));
                lines.push(Line::from(Span::styled(
                    "  a     = reset to auto (15-dim analyzer)",
                    Style::default().fg(Color::DarkGray),
                )));
                lines
            } else {
                vec![Line::from("Select a scenario")]
            }
        } else {
            vec![Line::from("Select a scenario")]
        }
    } else {
        vec![Line::from("No scenarios configured")]
    };

    let detail = Paragraph::new(detail_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Scenario Details "),
        )
        .alignment(Alignment::Left);
    f.render_widget(detail, panes[1]);
}

fn draw_auth(f: &mut ratatui::Frame, _app: &TuiApp, area: ratatui::layout::Rect) {
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Authentication — OAuth Device Flows & API Keys",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  ─── Device-Flow (browser-based) ──────────────────────────────",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  yolo-router --auth github",
            Style::default().fg(Color::Green),
        )),
        Line::from("    → GitHub Copilot OAuth device flow"),
        Line::from("    → URL: https://github.com/login/device"),
        Line::from("    → Polls GitHub until authorized"),
        Line::from("    → Token saved to ~/.config/yolo-router/github_token"),
        Line::from(""),
        Line::from(Span::styled(
            "  yolo-router --auth codex",
            Style::default().fg(Color::Green),
        )),
        Line::from("    → ChatGPT Plus/Pro OAuth device flow"),
        Line::from("    → URL: https://auth.openai.com/codex/device"),
        Line::from("    → Step 1: Poll for authorization code"),
        Line::from("    → Step 2: Exchange code+verifier for access_token"),
        Line::from("    → Tokens saved to ~/.config/yolo-router/codex_oauth.json"),
        Line::from(""),
        Line::from(Span::styled(
            "  ─── API Keys ──────────────────────────────────────────────────",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  yolo-router --auth anthropic",
            Style::default().fg(Color::Green),
        )),
        Line::from("    → Prompts for Anthropic API key"),
        Line::from(""),
        Line::from(Span::styled(
            "  yolo-router --auth openai",
            Style::default().fg(Color::Green),
        )),
        Line::from("    → Prompts for OpenAI API key"),
        Line::from(""),
        Line::from(Span::styled(
            "  yolo-router --auth gemini",
            Style::default().fg(Color::Green),
        )),
        Line::from("    → Prompts for Google AI Studio API key"),
    ];

    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" Auth "))
        .alignment(Alignment::Left);
    f.render_widget(para, area);
}

fn draw_help(f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Keyboard Shortcuts",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  Tab / Shift+Tab  — Next / previous tab"),
        Line::from("  1–5              — Jump to tab directly"),
        Line::from("  j / Down         — Move selection down"),
        Line::from("  k / Up           — Move selection up"),
        Line::from("  Enter            — [Scenarios] Pin global routing to selected scenario"),
        Line::from("  a                — [Scenarios] Reset to auto (15-dim analyzer)"),
        Line::from("  q / Ctrl+C       — Quit"),
        Line::from(""),
        Line::from(Span::styled(
            "  Protocol Endpoints",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  POST /v1/anthropic           → Anthropic Messages format (Claude Code)"),
        Line::from("  POST /v1/anthropic/v1/messages  (same, full path)"),
        Line::from("  POST /v1/openai              → OpenAI Chat Completions format"),
        Line::from("  POST /v1/openai/chat/completions  (same, full path)"),
        Line::from("  POST /v1/codex               → OpenAI format (Codex CLI)"),
        Line::from("  POST /v1/gemini              → OpenAI-compat format (Gemini)"),
        Line::from("  POST /v1/auto                → 15-dim auto-route"),
        Line::from(""),
        Line::from(Span::styled(
            "  Control API (while daemon is running)",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  GET  /control/status         → current overrides & providers"),
        Line::from(
            "  POST /control/override        → {\"endpoint\":\"global\",\"scenario\":\"coding\"}",
        ),
        Line::from("  DELETE /control/override/{ep} → reset endpoint to auto"),
        Line::from(""),
        Line::from(Span::styled(
            "  CLI",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  yolo-router                  → Start daemon"),
        Line::from("  yolo-router --tui            → Open this TUI"),
        Line::from("  yolo-router --auth github    → GitHub Copilot OAuth"),
        Line::from("  yolo-router --auth codex     → ChatGPT Pro OAuth"),
        Line::from("  YOLO_CONFIG=./config.toml yolo-router"),
    ];

    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" Help "))
        .alignment(Alignment::Left);
    f.render_widget(para, area);
}
