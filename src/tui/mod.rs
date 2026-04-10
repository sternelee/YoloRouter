pub mod auth;

pub use auth::AuthFlow;

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

    pub async fn run(&self) {
        println!("TUI mode not yet implemented");
    }
}
