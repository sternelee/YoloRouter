// GitHub Copilot OAuth device flow TUI
// Shows device code, polls for authorization, displays result

use crate::provider::GitHubCopilotProvider;
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
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub enum DeviceFlowState {
    /// Fetching device code from GitHub
    Fetching,
    /// Waiting for user to authorize — shows user_code and verification_uri
    WaitingForUser {
        user_code: String,
        verification_uri: String,
        expires_at: Instant,
        interval_secs: u64,
    },
    /// Successfully got token
    Success { github_token: String },
    /// Auth failed
    Failed { reason: String },
    /// User cancelled
    Cancelled,
}

struct DeviceFlowApp {
    state: Arc<Mutex<DeviceFlowState>>,
    spinner_tick: u64,
}

/// Run the GitHub device flow TUI. Returns the GitHub OAuth token on success.
pub async fn run_github_device_flow(client_id: Option<String>) -> io::Result<Option<String>> {
    let state = Arc::new(Mutex::new(DeviceFlowState::Fetching));

    // Spawn async task to drive the OAuth flow
    let state_clone = Arc::clone(&state);
    let cid = client_id.unwrap_or_else(|| "Iv1.b507a08c87ecfe98".to_string());
    tokio::spawn(async move {
        drive_device_flow(state_clone, cid).await;
    });

    // Run the blocking TUI event loop on a dedicated thread so the tokio
    // runtime (which may be single-threaded under #[actix_web::main]) stays
    // free to drive the async `drive_device_flow` task above.
    let state_for_tui = Arc::clone(&state);
    let tui_result = tokio::task::spawn_blocking(move || {
        let mut terminal = setup_terminal()?;
        let app = DeviceFlowApp {
            state: state_for_tui,
            spinner_tick: 0,
        };
        let result = event_loop(&mut terminal, app);
        restore_terminal(&mut terminal);
        result
    })
    .await
    .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))??;

    match tui_result {
        Some(token) => Ok(Some(token)),
        None => Ok(None),
    }
}

async fn drive_device_flow(state: Arc<Mutex<DeviceFlowState>>, client_id: String) {
    let provider = GitHubCopilotProvider::new_with_client_id(String::new(), client_id);

    let device_resp = match provider.request_device_code().await {
        Ok(r) => r,
        Err(e) => {
            let mut s = state.lock().unwrap();
            *s = DeviceFlowState::Failed {
                reason: e.to_string(),
            };
            return;
        }
    };

    let expires_at = Instant::now() + Duration::from_secs(device_resp.expires_in);
    {
        let mut s = state.lock().unwrap();
        *s = DeviceFlowState::WaitingForUser {
            user_code: device_resp.user_code.clone(),
            verification_uri: device_resp.verification_uri.clone(),
            expires_at,
            interval_secs: device_resp.interval,
        };
    }

    match provider
        .poll_for_token(
            &device_resp.device_code,
            device_resp.interval,
            device_resp.expires_in,
        )
        .await
    {
        Ok(token) => {
            let mut s = state.lock().unwrap();
            *s = DeviceFlowState::Success {
                github_token: token,
            };
        }
        Err(e) => {
            let mut s = state.lock().unwrap();
            *s = DeviceFlowState::Failed {
                reason: e.to_string(),
            };
        }
    }
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut app: DeviceFlowApp,
) -> io::Result<Option<String>> {
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
                        let mut s = app.state.lock().unwrap();
                        *s = DeviceFlowState::Cancelled;
                        return Ok(None);
                    }
                    (KeyCode::Enter, _) => {
                        let s = app.state.lock().unwrap();
                        if let DeviceFlowState::Success { ref github_token } = *s {
                            return Ok(Some(github_token.clone()));
                        }
                        if matches!(
                            *s,
                            DeviceFlowState::Failed { .. } | DeviceFlowState::Cancelled
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

            // Auto-exit on terminal states
            let s = app.state.lock().unwrap();
            match &*s {
                DeviceFlowState::Success { github_token } => {
                    return Ok(Some(github_token.clone()));
                }
                DeviceFlowState::Failed { .. } | DeviceFlowState::Cancelled => {
                    return Ok(None);
                }
                _ => {}
            }
        }
    }
}

fn draw_ui(f: &mut ratatui::Frame, app: &DeviceFlowApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.size());

    // Header
    let header = Paragraph::new("  GitHub Copilot OAuth Device Flow")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // Body
    let state = app.state.lock().unwrap();
    match &*state {
        DeviceFlowState::Fetching => draw_fetching(f, app.spinner_tick, chunks[1]),
        DeviceFlowState::WaitingForUser {
            user_code,
            verification_uri,
            expires_at,
            ..
        } => {
            draw_waiting(
                f,
                user_code,
                verification_uri,
                *expires_at,
                app.spinner_tick,
                chunks[1],
            );
        }
        DeviceFlowState::Success { github_token } => draw_success(f, github_token, chunks[1]),
        DeviceFlowState::Failed { reason } => draw_failed(f, reason, chunks[1]),
        DeviceFlowState::Cancelled => draw_cancelled(f, chunks[1]),
    }

    // Footer
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
            format!("  {spin} Requesting device code from GitHub..."),
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
    let total_secs = 900_u64; // typical GitHub device flow expiry
    let ratio = remaining as f64 / total_secs as f64;

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(area);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Step 1: Visit this URL in your browser",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
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
            "  Step 2: Enter this code",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("  ┌─────────────┐"),
            Style::default().fg(Color::White),
        )),
        Line::from(Span::styled(
            format!("  │  {}  │", user_code),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("  └─────────────┘"),
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

    let para =
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" Authorize "));
    f.render_widget(para, inner[0]);

    // Countdown gauge
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

fn draw_success(f: &mut ratatui::Frame, token: &str, area: ratatui::layout::Rect) {
    let masked = if token.len() > 8 {
        format!("{}...{}", &token[..4], &token[token.len() - 4..])
    } else {
        "****".to_string()
    };

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  ✅  Authorization successful!",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Token: ", Style::default().fg(Color::Cyan)),
            Span::raw(&masked),
        ]),
        Line::from(""),
        Line::from("  Press Enter to save token to config, or q to discard."),
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
