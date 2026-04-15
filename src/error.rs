use actix_web::{error::ResponseError, http::StatusCode, HttpResponse};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum YoloRouterError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Provider error: {0}")]
    ProviderError(String),

    #[error("Authentication error: {0}")]
    AuthError(String),

    #[error("Request error: {0}")]
    RequestError(String),

    #[error("Routing error: {0}")]
    RoutingError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("TOML error: {0}")]
    TomlError(#[from] toml::de::Error),

    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("All providers failed: {0}")]
    AllProvidersFailed(String),

    #[error("Timeout error: {0}")]
    TimeoutError(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl ResponseError for YoloRouterError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::ConfigError(_) => StatusCode::BAD_REQUEST,
            Self::AuthError(_) => StatusCode::UNAUTHORIZED,
            Self::ProviderError(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::RequestError(_) => StatusCode::BAD_REQUEST,
            Self::RoutingError(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::AllProvidersFailed(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::TimeoutError(_) => StatusCode::GATEWAY_TIMEOUT,
            Self::NotImplemented(_) => StatusCode::NOT_IMPLEMENTED,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let status = self.status_code();
        HttpResponse::build(status).json(serde_json::json!({
            "error": self.to_string(),
            "status": status.as_u16(),
        }))
    }
}
