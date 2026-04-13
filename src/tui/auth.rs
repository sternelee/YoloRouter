use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use std::io;

#[derive(Debug, Clone)]
pub enum AuthProvider {
    Anthropic,
    OpenAI,
    Google,
    GitHub,
    Codex,
}

impl std::fmt::Display for AuthProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthProvider::Anthropic => write!(f, "Anthropic"),
            AuthProvider::OpenAI => write!(f, "OpenAI"),
            AuthProvider::Google => write!(f, "Google Gemini"),
            AuthProvider::GitHub => write!(f, "GitHub"),
            AuthProvider::Codex => write!(f, "GitHub Codex"),
        }
    }
}

pub struct AuthFlow {
    selected_provider: usize,
    providers: Vec<AuthProvider>,
    api_key_input: String,
    current_step: AuthStep,
}

#[derive(Debug, Clone, PartialEq)]
enum AuthStep {
    SelectProvider,
    InputApiKey,
    ConfirmKey,
    Complete,
}

impl AuthFlow {
    pub fn new() -> Self {
        Self {
            selected_provider: 0,
            providers: vec![
                AuthProvider::Anthropic,
                AuthProvider::OpenAI,
                AuthProvider::Google,
                AuthProvider::GitHub,
                AuthProvider::Codex,
            ],
            api_key_input: String::new(),
            current_step: AuthStep::SelectProvider,
        }
    }

    pub fn next_provider(&mut self) {
        self.selected_provider = (self.selected_provider + 1) % self.providers.len();
    }

    pub fn prev_provider(&mut self) {
        if self.selected_provider == 0 {
            self.selected_provider = self.providers.len() - 1;
        } else {
            self.selected_provider -= 1;
        }
    }

    pub fn select_provider(&mut self) {
        self.current_step = AuthStep::InputApiKey;
        self.api_key_input.clear();
    }

    pub fn input_char(&mut self, c: char) {
        if self.current_step == AuthStep::InputApiKey {
            self.api_key_input.push(c);
        }
    }

    pub fn backspace(&mut self) {
        if self.current_step == AuthStep::InputApiKey {
            self.api_key_input.pop();
        }
    }

    pub fn confirm_key(&mut self) {
        if !self.api_key_input.is_empty() {
            self.current_step = AuthStep::ConfirmKey;
        }
    }

    pub fn complete_auth(&mut self) -> Option<(AuthProvider, String)> {
        if self.current_step == AuthStep::ConfirmKey {
            self.current_step = AuthStep::Complete;
            Some((
                self.providers[self.selected_provider].clone(),
                self.api_key_input.clone(),
            ))
        } else {
            None
        }
    }

    pub fn back(&mut self) {
        match self.current_step {
            AuthStep::SelectProvider => {}
            AuthStep::InputApiKey => {
                self.current_step = AuthStep::SelectProvider;
                self.api_key_input.clear();
            }
            AuthStep::ConfirmKey => {
                self.current_step = AuthStep::InputApiKey;
            }
            AuthStep::Complete => {
                self.current_step = AuthStep::SelectProvider;
            }
        }
    }

    fn get_provider_help_text(&self, provider: &AuthProvider) -> &'static str {
        match provider {
            AuthProvider::Anthropic => {
                "Get your API key from: https://console.anthropic.com/account/keys"
            }
            AuthProvider::OpenAI => {
                "Get your API key from: https://platform.openai.com/account/api-keys"
            }
            AuthProvider::Google => {
                "Get your API key from: https://makersuite.google.com/app/apikey"
            }
            AuthProvider::GitHub => "Get your token from: https://github.com/settings/tokens",
            AuthProvider::Codex => "Get your GitHub token from: https://github.com/settings/tokens",
        }
    }
}

pub fn run_auth_tui(provider: AuthProvider) -> io::Result<Option<String>> {
    let mut terminal = setup_terminal()?;
    let mut auth_flow = AuthFlow::new();

    // Find and select the provider
    for (i, p) in auth_flow.providers.iter().enumerate() {
        if std::mem::discriminant(p) == std::mem::discriminant(&provider) {
            auth_flow.selected_provider = i;
            auth_flow.select_provider();
            break;
        }
    }

    let result = loop {
        terminal.draw(|f| ui(f, &auth_flow))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Esc => break None,
                KeyCode::Char('q') => break None,
                KeyCode::Up => {
                    if auth_flow.current_step == AuthStep::SelectProvider {
                        auth_flow.prev_provider();
                    }
                }
                KeyCode::Down => {
                    if auth_flow.current_step == AuthStep::SelectProvider {
                        auth_flow.next_provider();
                    }
                }
                KeyCode::Right | KeyCode::Tab => {
                    if auth_flow.current_step == AuthStep::SelectProvider {
                        auth_flow.select_provider();
                    }
                }
                KeyCode::Enter => {
                    if auth_flow.current_step == AuthStep::InputApiKey {
                        auth_flow.confirm_key();
                    } else if auth_flow.current_step == AuthStep::ConfirmKey {
                        if let Some((_, key)) = auth_flow.complete_auth() {
                            break Some(key);
                        }
                    } else if auth_flow.current_step == AuthStep::SelectProvider {
                        auth_flow.select_provider();
                    }
                }
                KeyCode::Backspace => auth_flow.backspace(),
                KeyCode::Char(c) => {
                    if c != 'q' {
                        auth_flow.input_char(c);
                    }
                }
                KeyCode::Left => auth_flow.back(),
                _ => {}
            }
        }
    };

    restore_terminal(&mut terminal)?;
    Ok(result)
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn ui(f: &mut ratatui::Frame, auth_flow: &AuthFlow) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(f.size());

    // Title
    let title = Paragraph::new("YoloRouter Authentication Setup")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center);
    f.render_widget(title, chunks[0]);

    // Main content - use closures to avoid generic inference issues
    match auth_flow.current_step {
        AuthStep::SelectProvider => {
            let providers_text: Vec<Line> = auth_flow
                .providers
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let selected = if i == auth_flow.selected_provider {
                        "► "
                    } else {
                        "  "
                    };
                    let style = if i == auth_flow.selected_provider {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    Line::from(vec![Span::styled(format!("{}{}", selected, p), style)])
                })
                .collect();

            let block = Block::default()
                .title("Select Provider")
                .borders(Borders::ALL);
            let paragraph = Paragraph::new(providers_text).block(block);
            f.render_widget(paragraph, chunks[1]);
        }
        AuthStep::InputApiKey => {
            let provider = &auth_flow.providers[auth_flow.selected_provider];
            let help_text = auth_flow.get_provider_help_text(provider);

            let inner_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(4), Constraint::Min(5)])
                .split(chunks[1]);

            let help = Paragraph::new(help_text)
                .block(Block::default().title("Instructions").borders(Borders::ALL))
                .style(Style::default().fg(Color::Blue));
            f.render_widget(help, inner_chunks[0]);

            let input_text = format!("Key: {}", "*".repeat(auth_flow.api_key_input.len()));
            let input = Paragraph::new(input_text)
                .block(Block::default().title("API Key").borders(Borders::ALL))
                .style(Style::default().fg(Color::Green));
            f.render_widget(input, inner_chunks[1]);

            // Show cursor position
            let cursor_x = 6 + auth_flow.api_key_input.len() as u16;
            let cursor_y = inner_chunks[1].y + 1;
            f.set_cursor(cursor_x, cursor_y);
        }
        AuthStep::ConfirmKey => {
            let provider = &auth_flow.providers[auth_flow.selected_provider];
            let key_display = format!(
                "Provider: {}\nKey length: {} characters\n\nPress Enter to save, Esc to edit",
                provider,
                auth_flow.api_key_input.len()
            );

            let block = Block::default().title("Confirm").borders(Borders::ALL);
            let paragraph = Paragraph::new(key_display)
                .block(block)
                .style(Style::default().fg(Color::Yellow));
            f.render_widget(paragraph, chunks[1]);
        }
        AuthStep::Complete => {
            let success_text = "✓ Authentication successful!\n\nYour API key has been stored in the configuration.\nPress Esc to exit.";

            let block = Block::default()
                .title("Complete")
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Green));
            let paragraph = Paragraph::new(success_text)
                .block(block)
                .alignment(Alignment::Center);
            f.render_widget(paragraph, chunks[1]);
        }
    }

    // Help text
    let help = match auth_flow.current_step {
        AuthStep::SelectProvider => "Use ↑↓ to select provider, Enter to confirm, Esc to exit",
        AuthStep::InputApiKey => "Enter your API key, Tab to confirm, Esc to back",
        AuthStep::ConfirmKey => "Review and press Enter to save, Esc to edit",
        AuthStep::Complete => "Authentication complete! Press Esc to exit",
    };

    let help_text = Paragraph::new(help)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);
    f.render_widget(help_text, chunks[2]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_flow_creation() {
        let auth = AuthFlow::new();
        assert_eq!(auth.current_step, AuthStep::SelectProvider);
        assert_eq!(auth.selected_provider, 0);
        assert!(auth.api_key_input.is_empty());
    }

    #[test]
    fn test_auth_flow_navigation() {
        let mut auth = AuthFlow::new();
        auth.next_provider();
        assert_eq!(auth.selected_provider, 1);
        auth.prev_provider();
        assert_eq!(auth.selected_provider, 0);
    }

    #[test]
    fn test_auth_flow_transitions() {
        let mut auth = AuthFlow::new();
        assert_eq!(auth.current_step, AuthStep::SelectProvider);

        auth.select_provider();
        assert_eq!(auth.current_step, AuthStep::InputApiKey);

        auth.input_char('s');
        auth.input_char('k');
        auth.input_char('-');
        assert_eq!(auth.api_key_input.len(), 3);

        auth.confirm_key();
        assert_eq!(auth.current_step, AuthStep::ConfirmKey);
    }

    #[test]
    fn test_backspace() {
        let mut auth = AuthFlow::new();
        auth.select_provider();
        auth.input_char('a');
        auth.input_char('b');
        assert_eq!(auth.api_key_input.len(), 2);

        auth.backspace();
        assert_eq!(auth.api_key_input.len(), 1);
    }

    #[test]
    fn test_back_navigation() {
        let mut auth = AuthFlow::new();
        auth.select_provider();
        assert_eq!(auth.current_step, AuthStep::InputApiKey);

        auth.back();
        assert_eq!(auth.current_step, AuthStep::SelectProvider);
    }
}
