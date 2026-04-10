// Codex (ChatGPT) OAuth device-flow TUI
//
// Three-step state machine vs GitHub's two-step:
//   Fetching → WaitingForUser → Exchanging → Success / Failed / Cancelled
//
// The extra "Exchanging" state covers the code+verifier → access_token exchange.

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Terminal,
};
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use crate::provider::CodexOAuthProvider;

// ─── State machine ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum CodexFlowState {
    /// Requesting device code from OpenAI
    Fetching,
    /// User must visit URL and enter the code
    WaitingForUser {
        device_auth_id: String,
        user_code: String,
        verification_uri: String,
        expires_at: Instant,
        interval_secs: u64,
    },
    /// Got authorization_code; exchanging for access_token
    Exchanging,
    /// Full success — access_token (and optional refresh_token) obtained
    Success {
        access_token: String,
        refresh_token: Option<String>,
    },
    Failed { reason: String },
    Cancelled,
}

// ─── App state ────────────────────────────────────────────────────────────────

struct CodexAuthApp {
    state: Arc<Mutex<CodexFlowState>>,
    spinner_tick: u64,
}

// ─── Public entry point ───────────────────────────────────────────────────────

/// Run the Codex OAuth device-flow TUI.
///
/// Returns `Ok(Some((access_token, refresh_token)))` on success.
/// Tokens are automatically persisted to `token_path` by the provider.
pub async fn run_codex_device_flow(
    token_path: Option<PathBuf>,
) -> io::Result<Option<(String, Option<String>)>> {
    let state = Arc::new(Mutex::new(CodexFlowState::Fetching));

    let state_clone = Arc::clone(&state);
    let path_clone = token_path.clone();
    tokio::spawn(async move {
        drive_codex_flow(state_clone, path_clone).await;
    });

    let mut terminal = setup_terminal()?;
    let app = CodexAuthApp {
        state: Arc::clone(&state),
        spinner_tick: 0,
    };

    let result = event_loop(&mut terminal, app);
    restore_terminal(&mut terminal);

    result
}

// ─── Async driver ─────────────────────────────────────────────────────────────

async fn drive_codex_flow(state: Arc<Mutex<CodexFlowState>>, token_path: Option<PathBuf>) {
    let provider = CodexOAuthProvider::new(token_path);

    // Step 1: start device flow
    let display = match provider.start_device_flow().await {
        Ok(d) => d,
        Err(e) => {
            set_state(&state, CodexFlowState::Failed { reason: e.to_string() });
            return;
        }
    };

    let expires_at = Instant::now() + Duration::from_secs(display.expires_in);
    let device_auth_id = display.device_auth_id.clone();
    let user_code = display.user_code.clone();
    let interval_secs = display.interval_secs.max(5); // floor at 5s

    set_state(
        &state,
        CodexFlowState::WaitingForUser {
            device_auth_id: display.device_auth_id,
            user_code: display.user_code,
            verification_uri: display.verification_uri,
            expires_at,
            interval_secs,
        },
    );

    // Step 2: poll until user authorizes or times out
    let deadline = Instant::now() + Duration::from_secs(display.expires_in);
    let (auth_code, code_verifier) = loop {
        if Instant::now() >= deadline {
            set_state(
                &state,
                CodexFlowState::Failed {
                    reason: "Device code expired (user did not authorize in time)".to_string(),
                },
            );
            return;
        }

        tokio::time::sleep(Duration::from_secs(interval_secs)).await;

        match provider.poll_device_flow(&device_auth_id, &user_code).await {
            Ok(Some((code, verifier))) => break (code, verifier),
            Ok(None) => continue, // still pending
            Err(e) => {
                set_state(&state, CodexFlowState::Failed { reason: e.to_string() });
                return;
            }
        }
    };

    // Step 3: exchange code + verifier for tokens
    set_state(&state, CodexFlowState::Exchanging);

    match provider.exchange_code(&auth_code, &code_verifier).await {
        Ok(token_state) => {
            set_state(
                &state,
                CodexFlowState::Success {
                    access_token: token_state.access_token.unwrap_or_default(),
                    refresh_token: token_state.refresh_token,
                },
            );
        }
        Err(e) => {
            set_state(&state, CodexFlowState::Failed { reason: e.to_string() });
        }
    }
}

fn set_state(state: &Arc<Mutex<CodexFlowState>>, new: CodexFlowState) {
    if let Ok(mut s) = state.lock() {
        *s = new;
    }
}

// ─── Sync event loop ──────────────────────────────────────────────────────────

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut app: CodexAuthApp,
) -> io::Result<Option<(String, Option<String>)>> {
    let tick_rate = Duration::from_millis(200);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| draw_ui(f, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_default();

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match (key.code, key.modifiers) {
                    (KeyCode::Char('q'), _)
                    | (KeyCode::Esc, _)
                    | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        set_state(&app.state, CodexFlowState::Cancelled);
                        return Ok(None);
                    }
                    (KeyCode::Enter, _) => {
                        let s = app.state.lock().unwrap();
                        if let CodexFlowState::Success {
                            ref access_token,
                            ref refresh_token,
                        } = *s
                        {
                            return Ok(Some((access_token.clone(), refresh_token.clone())));
                        }
                        if matches!(
                            *s,
                            CodexFlowState::Failed { .. } | CodexFlowState::Cancelled
                        ) {
                            return Ok(None);
                        }
                    }
                    _ => {}
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.spinner_tick = app.spinner_tick.wrapping_add(1);
            last_tick = Instant::now();

            let s = app.state.lock().unwrap();
            match &*s {
                CodexFlowState::Success { access_token, refresh_token } => {
                    return Ok(Some((access_token.clone(), refresh_token.clone())));
                }
                CodexFlowState::Failed { .. } | CodexFlowState::Cancelled => {
                    return Ok(None);
                }
                _ => {}
            }
        }
    }
}

// ─── UI rendering ─────────────────────────────────────────────────────────────

fn draw_ui(f: &mut ratatui::Frame, app: &CodexAuthApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.size());

    let header = Paragraph::new("  ChatGPT / Codex OAuth Device Flow")
        .style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    let state = app.state.lock().unwrap();
    match &*state {
        CodexFlowState::Fetching => draw_fetching(f, app.spinner_tick, chunks[1]),
        CodexFlowState::WaitingForUser {
            user_code,
            verification_uri,
            expires_at,
            ..
        } => draw_waiting(
            f,
            user_code,
            verification_uri,
            *expires_at,
            app.spinner_tick,
            chunks[1],
        ),
        CodexFlowState::Exchanging => draw_exchanging(f, app.spinner_tick, chunks[1]),
        CodexFlowState::Success {
            access_token,
            refresh_token,
        } => draw_success(f, access_token, refresh_token.as_deref(), chunks[1]),
        CodexFlowState::Failed { reason } => draw_failed(f, reason, chunks[1]),
        CodexFlowState::Cancelled => draw_cancelled(f, chunks[1]),
    }

    let hint = Paragraph::new("  q/Esc — cancel  |  Enter — confirm when done")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(hint, chunks[2]);
}

const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn draw_fetching(f: &mut ratatui::Frame, tick: u64, area: ratatui::layout::Rect) {
    let spin = SPINNER[(tick as usize) % SPINNER.len()];
    let para = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  {spin} Requesting device code from OpenAI..."),
            Style::default().fg(Color::Yellow),
        )),
    ])
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(para, area);
}

fn draw_waiting(
    f: &mut ratatui::Frame,
    user_code: &str,
    verification_uri: &str,
    expires_at: Instant,
    tick: u64,
    area: ratatui::layout::Rect,
) {
    let spin = SPINNER[(tick as usize) % SPINNER.len()];
    let remaining = expires_at
        .duration_since(Instant::now().min(expires_at))
        .as_secs();
    let total_secs = 900_u64;
    let ratio = remaining as f64 / total_secs as f64;

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(area);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Step 1: Visit this URL in your browser",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("  ➜  {}", verification_uri),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::UNDERLINED),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Step 2: Enter this one-time code",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  ┌───────────────────┐",
            Style::default().fg(Color::White),
        )),
        Line::from(Span::styled(
            format!("  │  {:^17}  │", user_code),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "  └───────────────────┘",
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!("  {spin} Waiting for authorization  "),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                format!("({}s remaining)", remaining),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ];

    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" Authorize "));
    f.render_widget(para, inner[0]);

    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Time Remaining "),
        )
        .gauge_style(Style::default().fg(Color::Green))
        .ratio(ratio.clamp(0.0, 1.0));
    f.render_widget(gauge, inner[1]);
}

fn draw_exchanging(f: &mut ratatui::Frame, tick: u64, area: ratatui::layout::Rect) {
    let spin = SPINNER[(tick as usize) % SPINNER.len()];
    let para = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  {spin} Authorization received! Exchanging code for token..."),
            Style::default().fg(Color::Cyan),
        )),
    ])
    .block(Block::default().borders(Borders::ALL).title(" Exchanging "));
    f.render_widget(para, area);
}

fn draw_success(
    f: &mut ratatui::Frame,
    access_token: &str,
    refresh_token: Option<&str>,
    area: ratatui::layout::Rect,
) {
    let masked_access = mask_token(access_token);
    let refresh_line = if let Some(rt) = refresh_token {
        format!("  Refresh:  {}", mask_token(rt))
    } else {
        "  Refresh:  (none)".to_string()
    };

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  ✅  ChatGPT authorization successful!",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Access:   ", Style::default().fg(Color::Cyan)),
            Span::raw(&masked_access),
        ]),
        Line::from(Span::styled(
            &refresh_line,
            Style::default().fg(Color::Cyan),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Tokens saved to ~/.config/yolo-router/codex_oauth.json",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from("  Press Enter to continue, or q to discard."),
    ];

    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" Success "))
        .alignment(Alignment::Left);
    f.render_widget(para, area);
}

fn draw_failed(f: &mut ratatui::Frame, reason: &str, area: ratatui::layout::Rect) {
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  ✗  Authorization failed",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Reason: ", Style::default().fg(Color::Yellow)),
            Span::raw(reason),
        ]),
        Line::from(""),
        Line::from("  Press q to exit."),
    ];

    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" Error "))
        .alignment(Alignment::Left);
    f.render_widget(para, area);
}

fn draw_cancelled(f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
    let para = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Authorization cancelled.",
            Style::default().fg(Color::DarkGray),
        )),
    ])
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(para, area);
}

fn mask_token(token: &str) -> String {
    if token.len() > 8 {
        format!("{}...{}", &token[..4], &token[token.len() - 4..])
    } else {
        "****".to_string()
    }
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout))
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) {
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();
}
