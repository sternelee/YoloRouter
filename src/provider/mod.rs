mod anthropic;
pub mod codex;
pub mod codex_oauth;
pub mod factory;
pub mod gemini;
pub mod generic;
pub mod github_copilot;
pub mod models;
pub mod openai;

pub use anthropic::AnthropicProvider;
pub use codex::CodexProvider;
pub use codex_oauth::CodexOAuthProvider;
pub use factory::ProviderFactory;
pub use gemini::GeminiProvider;
pub use generic::GenericProvider;
pub use github_copilot::GitHubCopilotProvider;
pub use openai::OpenAIProvider;

use crate::models::{ChatRequest, ChatResponse};
use crate::Result;
use async_trait::async_trait;
use reqwest::Response;

#[async_trait]
pub trait Provider: Send + Sync {
    async fn send_request(&self, request: &ChatRequest) -> Result<ChatResponse>;
    
    /// Start a streaming request. Returns a Response with bytes_stream.
    /// Default implementation returns an error indicating streaming is not supported.
    async fn start_streaming_request(&self, _request: &ChatRequest) -> Result<Response> {
        Err(crate::error::YoloRouterError::NotImplemented(
            format!("{} does not support streaming", self.name())
        ))
    }
    
    /// Returns true if this provider supports streaming requests.
    fn supports_streaming(&self) -> bool {
        false
    }
    
    fn name(&self) -> &str;
    fn model_list(&self) -> Vec<String>;
}
