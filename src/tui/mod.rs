pub mod auth;
pub mod config_editor;
pub mod github_auth;

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

// ─── TuiApp State ────────────────────────────────────────────────────────────

struct TuiApp {
    tab: ActiveTab,
    config: Config,
    config_path: String,
    provider_list_state: ListState,
    scenario_list_state: ListState,
    status_message: String,
}

impl TuiApp {
    fn new(config: Config, config_path: String) -> Self {
        let mut provider_list_state = ListState::default();
        provider_list_state.select(Some(0));
        let mut scenario_list_state = ListState::default();
        scenario_list_state.select(Some(0));

        Self {
            tab: ActiveTab::Status,
            config,
            config_path,
            provider_list_state,
            scenario_list_state,
            status_message: "Ready. Press Tab/Shift+Tab to switch tabs, q to quit.".to_string(),
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
        if let Err(e) = run_tui(config, config_path) {
            eprintln!("TUI error: {e}");
        }
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

fn run_tui(config: Config, config_path: String) -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = TuiApp::new(config, config_path);
    let result = event_loop(&mut terminal, &mut app);
    restore_terminal(&mut terminal);
    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut TuiApp,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| draw_ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            // Global keys
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
            let scenarios: Vec<_> = app.config.scenarios().keys().cloned().collect();
            match key {
                KeyCode::Down | KeyCode::Char('j') => {
                    let i = app.scenario_list_state.selected().unwrap_or(0);
                    let next = if scenarios.is_empty() { 0 } else { (i + 1) % scenarios.len() };
                    app.scenario_list_state.select(Some(next));
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    let i = app.scenario_list_state.selected().unwrap_or(0);
                    let prev = if i == 0 { scenarios.len().saturating_sub(1) } else { i - 1 };
                    app.scenario_list_state.select(Some(prev));
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

    let lines = vec![
        Line::from(vec![
            Span::styled("Config: ", Style::default().fg(Color::Cyan)),
            Span::raw(&app.config_path),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Daemon Port:   ", Style::default().fg(Color::Cyan)),
            Span::raw(daemon.port.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Log Level:     ", Style::default().fg(Color::Cyan)),
            Span::raw(&daemon.log_level),
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
    let items: Vec<ListItem> = scenarios
        .iter()
        .map(|(name, sc)| {
            let label = if sc.is_default {
                format!("{name} [default]")
            } else {
                name.clone()
            };
            ListItem::new(label)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Scenarios "))
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, panes[0], &mut app.scenario_list_state);

    let scenario_names: Vec<String> = scenarios.keys().cloned().collect();
    let detail_lines = if let Some(idx) = app.scenario_list_state.selected() {
        if let Some(name) = scenario_names.get(idx) {
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
                    Line::from(Span::styled("Models:", Style::default().fg(Color::Cyan))),
                ];
                for (i, m) in sc.models.iter().enumerate() {
                    let tier = m.cost_tier.as_deref().unwrap_or("?");
                    lines.push(Line::from(format!(
                        "  {}. {}/{} [{}]",
                        i + 1, m.provider, m.model, tier
                    )));
                }
                lines
            } else { vec![Line::from("Select a scenario")] }
        } else { vec![Line::from("Select a scenario")] }
    } else { vec![Line::from("No scenarios configured")] };

    let detail = Paragraph::new(detail_lines)
        .block(Block::default().borders(Borders::ALL).title(" Scenario Details "))
        .alignment(Alignment::Left);
    f.render_widget(detail, panes[1]);
}

fn draw_auth(f: &mut ratatui::Frame, _app: &TuiApp, area: ratatui::layout::Rect) {
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Authentication Options",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  Use the auth sub-command to authenticate providers:"),
        Line::from(""),
        Line::from(Span::styled(
            "  yolo-router auth anthropic",
            Style::default().fg(Color::Green),
        )),
        Line::from("    → Prompts for Anthropic API key"),
        Line::from(""),
        Line::from(Span::styled(
            "  yolo-router auth openai",
            Style::default().fg(Color::Green),
        )),
        Line::from("    → Prompts for OpenAI API key"),
        Line::from(""),
        Line::from(Span::styled(
            "  yolo-router auth github",
            Style::default().fg(Color::Green),
        )),
        Line::from("    → Starts GitHub OAuth device flow"),
        Line::from("    → Opens browser: https://github.com/login/device"),
        Line::from("    → Displays user code to enter"),
        Line::from("    → Polls until authorized"),
        Line::from(""),
        Line::from(Span::styled(
            "  yolo-router auth gemini",
            Style::default().fg(Color::Green),
        )),
        Line::from("    → Prompts for Google AI Studio API key"),
        Line::from(""),
        Line::from(Span::styled(
            "  yolo-router auth codex",
            Style::default().fg(Color::Green),
        )),
        Line::from("    → Prompts for OpenAI / Azure Codex API key"),
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
        Line::from("  q / Ctrl+C       — Quit"),
        Line::from(""),
        Line::from(Span::styled(
            "  HTTP Endpoints",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  POST /v1/anthropic/chat/completions  → Anthropic Claude"),
        Line::from("  POST /v1/openai/chat/completions     → OpenAI GPT"),
        Line::from("  POST /v1/gemini/chat/completions     → Google Gemini"),
        Line::from("  POST /v1/codex/chat/completions      → OpenAI Codex / Azure"),
        Line::from("  POST /v1/github/chat/completions     → GitHub Copilot"),
        Line::from("  POST /v1/auto/chat/completions       → 15-dim auto-route"),
        Line::from(""),
        Line::from(Span::styled(
            "  Config",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  YOLO_CONFIG=/path/to/config.toml yolo-router"),
        Line::from("  yolo-router --tui           → Open this TUI"),
        Line::from("  yolo-router --tui --config /path/to/config.toml"),
    ];

    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" Help "))
        .alignment(Alignment::Left);
    f.render_widget(para, area);
}
