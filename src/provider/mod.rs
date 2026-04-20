mod anthropic;
pub mod codex;
pub mod codex_oauth;
pub mod cursor;
pub mod factory;
pub mod gemini;
pub mod generic;
pub mod github_copilot;
pub mod models;
pub mod openai;

pub use anthropic::AnthropicProvider;
pub use codex::CodexProvider;
pub use codex_oauth::CodexOAuthProvider;
pub use cursor::CursorProvider;
pub use factory::ProviderFactory;
pub use gemini::GeminiProvider;
pub use generic::GenericProvider;
pub use github_copilot::GitHubCopilotProvider;
pub use openai::OpenAIProvider;

use crate::models::{ChatRequest, ChatResponse};
use crate::Result;
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::Stream;
use std::pin::Pin;

/// Byte stream returned by providers for streaming responses.
pub type ByteStream = Pin<Box<dyn Stream<Item = std::io::Result<Bytes>> + Send>>;

#[async_trait]
pub trait Provider: Send + Sync {
    async fn send_request(&self, request: &ChatRequest) -> Result<ChatResponse>;

    /// Start a streaming request. Returns a [`ByteStream`] of response chunks.
    /// Default implementation returns an error indicating streaming is not supported.
    async fn start_streaming_request(&self, _request: &ChatRequest) -> Result<ByteStream> {
        Err(crate::error::YoloRouterError::NotImplemented(format!(
            "{} does not support streaming",
            self.name()
        )))
    }

    /// Returns true if this provider supports streaming requests.
    fn supports_streaming(&self) -> bool {
        false
    }

    fn name(&self) -> &str;
    fn model_list(&self) -> Vec<String>;
}
