pub mod auth;
pub mod config_editor;
pub mod github_auth;
pub mod codex_auth;

pub use auth::AuthFlow;

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
use std::io;
use std::sync::mpsc;
use std::time::Duration;
use crate::config::Config;

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
            ActiveTab::Status    => ActiveTab::Providers,
            ActiveTab::Providers => ActiveTab::Scenarios,
            ActiveTab::Scenarios => ActiveTab::Auth,
            ActiveTab::Auth      => ActiveTab::Help,
            ActiveTab::Help      => ActiveTab::Status,
        }
    }
    fn prev(&self) -> Self {
        match self {
            ActiveTab::Status    => ActiveTab::Help,
            ActiveTab::Providers => ActiveTab::Status,
            ActiveTab::Scenarios => ActiveTab::Providers,
            ActiveTab::Auth      => ActiveTab::Scenarios,
            ActiveTab::Help      => ActiveTab::Auth,
        }
    }
    fn index(&self) -> usize {
        self.clone() as usize
    }
}

// ─── Override command sent from TUI to background HTTP task ──────────────────

#[derive(Debug)]
pub struct OverrideCommand {
    pub endpoint: String,
    pub scenario: Option<String>, // None = reset to auto
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
    cmd_tx: tokio::sync::mpsc::Sender<OverrideCommand>,
    /// Receive results/status messages from the async HTTP task
    status_rx: mpsc::Receiver<String>,
}

impl TuiApp {
    fn new(
        config: Config,
        config_path: String,
        cmd_tx: tokio::sync::mpsc::Sender<OverrideCommand>,
        status_rx: mpsc::Receiver<String>,
    ) -> Self {
        let mut provider_list_state = ListState::default();
        provider_list_state.select(Some(0));
        let mut scenario_list_state = ListState::default();
        scenario_list_state.select(Some(0));

        let mut scenario_names: Vec<String> = config.scenarios().keys().cloned().collect();
        scenario_names.sort();

        Self {
            tab: ActiveTab::Status,
            config,
            config_path,
            provider_list_state,
            scenario_list_state,
            scenario_names,
            status_message: "Ready. Tab/Shift+Tab: switch tabs  |  Scenarios: Enter=pin, a=auto  |  q=quit".to_string(),
            active_override: None,
            cmd_tx,
            status_rx,
        }
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
        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<OverrideCommand>(16);
        let (status_tx, status_rx) = mpsc::channel::<String>();

        // Background async task: receives override commands, sends HTTP to daemon
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            while let Some(cmd) = cmd_rx.recv().await {
                let url = format!("http://127.0.0.1:{}/control/override", port);
                let body = serde_json::json!({
                    "endpoint": cmd.endpoint,
                    "scenario": cmd.scenario,
                });
                let result = match client
                    .post(&url)
                    .json(&body)
                    .timeout(Duration::from_secs(2))
                    .send()
                    .await
                {
                    Ok(resp) if resp.status().is_success() => {
                        let label = cmd.scenario.as_deref().unwrap_or("auto");
                        format!("✅ {} → {}", cmd.endpoint, label)
                    }
                    Ok(resp) => format!("❌ Override failed: HTTP {}", resp.status()),
                    Err(e) => {
                        if e.is_timeout() || e.is_connect() {
                            "⚠️  Daemon offline — start daemon first (yolo-router)".to_string()
                        } else {
                            format!("❌ {}", e)
                        }
                    }
                };
                let _ = status_tx.send(result);
            }
        });

        // Run blocking TUI in a dedicated OS thread
        tokio::task::spawn_blocking(move || {
            if let Err(e) = run_tui(config, config_path, cmd_tx, status_rx) {
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
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture);
    let _ = terminal.show_cursor();
}

// ─── Main event loop ─────────────────────────────────────────────────────────

fn run_tui(
    config: Config,
    config_path: String,
    cmd_tx: tokio::sync::mpsc::Sender<OverrideCommand>,
    status_rx: mpsc::Receiver<String>,
) -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = TuiApp::new(config, config_path, cmd_tx, status_rx);
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

        terminal.draw(|f| draw_ui(f, app))?;

        // Non-blocking poll so the UI stays responsive and status_rx is checked regularly
        if event::poll(Duration::from_millis(200))? {
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

fn handle_tab_key(app: &mut TuiApp, key: KeyCode) {
    match app.tab {
        ActiveTab::Providers => {
            let providers: Vec<_> = app.config.providers().keys().cloned().collect();
            match key {
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
                _ => {}
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
                    let prev = if i == 0 { count.saturating_sub(1) } else { i - 1 };
                    app.scenario_list_state.select(Some(prev));
                }
                KeyCode::Enter => {
                    if let Some(i) = app.scenario_list_state.selected() {
                        if let Some(name) = app.scenario_names.get(i).cloned() {
                            let cmd = OverrideCommand {
                                endpoint: "global".to_string(),
                                scenario: Some(name.clone()),
                            };
                            match app.cmd_tx.try_send(cmd) {
                                Ok(_) => {
                                    app.status_message =
                                        format!("Sending override: global → {}…", name);
                                }
                                Err(_) => {
                                    app.status_message = "⚠️  Request queue full, try again".to_string();
                                }
                            }
                        }
                    }
                }
                KeyCode::Char('a') => {
                    let cmd = OverrideCommand {
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
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                },
            ))
        })
        .collect();

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(" YoloRouter TUI "))
        .select(app.tab.index())
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, chunks[0]);

    // Content
    match app.tab {
        ActiveTab::Status    => draw_status(f, app, chunks[1]),
        ActiveTab::Providers => draw_providers(f, app, chunks[1]),
        ActiveTab::Scenarios => draw_scenarios(f, app, chunks[1]),
        ActiveTab::Auth      => draw_auth(f, app, chunks[1]),
        ActiveTab::Help      => draw_help(f, chunks[1]),
    }

    // Status bar
    let status = Paragraph::new(app.status_message.as_str())
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(status, chunks[2]);
}

fn draw_status(f: &mut ratatui::Frame, app: &TuiApp, area: ratatui::layout::Rect) {
    let providers = app.config.providers();
    let scenarios = app.config.scenarios();
    let routing = app.config.routing();
    let daemon = app.config.daemon();

    let override_label = app
        .active_override
        .as_deref()
        .unwrap_or("auto (analyzer)");

    let lines = vec![
        Line::from(vec![
            Span::styled("Config: ", Style::default().fg(Color::Cyan)),
            Span::raw(&app.config_path),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Daemon Port:   ", Style::default().fg(Color::Cyan)),
            Span::raw(format!("127.0.0.1:{}  (POST /control/override to switch)", daemon.port)),
        ]),
        Line::from(vec![
            Span::styled("Log Level:     ", Style::default().fg(Color::Cyan)),
            Span::raw(&daemon.log_level),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Active Routing:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
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
            Span::raw(if routing.fallback_enabled { "enabled" } else { "disabled" }),
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

    // Detail pane
    let provider_names: Vec<String> = providers.keys().cloned().collect();
    let detail_text = if let Some(idx) = app.provider_list_state.selected() {
        if let Some(name) = provider_names.get(idx) {
            if let Some(cfg) = providers.get(name) {
                vec![
                    Line::from(vec![
                        Span::styled("Name:      ", Style::default().fg(Color::Cyan)),
                        Span::raw(name),
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
                ]
            } else { vec![Line::from("Select a provider")] }
        } else { vec![Line::from("Select a provider")] }
    } else { vec![Line::from("No providers configured")] };

    let detail = Paragraph::new(detail_text)
        .block(Block::default().borders(Borders::ALL).title(" Provider Details "))
        .alignment(Alignment::Left);
    f.render_widget(detail, panes[1]);
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
            let is_default = scenarios
                .get(name)
                .map(|sc| sc.is_default)
                .unwrap_or(false);
            let label = match (is_active, is_default) {
                (true, _) => format!("{name} ●"),
                (false, true) => format!("{name} [default]"),
                _ => name.clone(),
            };
            let style = if is_active {
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
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
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
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
                    Line::from(Span::styled("Models (fallback order):", Style::default().fg(Color::Cyan))),
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
        .block(Block::default().borders(Borders::ALL).title(" Scenario Details "))
        .alignment(Alignment::Left);
    f.render_widget(detail, panes[1]);
}

fn draw_auth(f: &mut ratatui::Frame, _app: &TuiApp, area: ratatui::layout::Rect) {
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Authentication — OAuth Device Flows & API Keys",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
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
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
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
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
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
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  GET  /control/status         → current overrides & providers"),
        Line::from("  POST /control/override        → {\"endpoint\":\"global\",\"scenario\":\"coding\"}"),
        Line::from("  DELETE /control/override/{ep} → reset endpoint to auto"),
        Line::from(""),
        Line::from(Span::styled(
            "  CLI",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
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
