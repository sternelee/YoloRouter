pub mod config;
pub mod error;
pub mod models;
pub mod provider;
pub mod router;
pub mod server;
pub mod tui;
pub mod utils;

pub use config::Config;
pub use error::YoloRouterError;
pub use models::*;
pub use provider::Provider;

pub type Result<T> = std::result::Result<T, YoloRouterError>;
