use crate::models::{ChatRequest, ChatResponse, Choice, ChatMessage, Usage};
use crate::Result;
use async_trait::async_trait;
use super::Provider;
use reqwest::Client;
use serde_json::{json, Value};

pub struct AnthropicProvider {
    #[allow(dead_code)]
    api_key: String,
    #[allow(dead_code)]
    base_url: String,
    client: Client,
}

impl AnthropicProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "https://api.anthropic.com".to_string(),
            client: Client::new(),
        }
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    async fn send_request(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let url = format!("{}/v1/messages", self.base_url);
        
        let payload = json!({
            "model": request.model,
            "max_tokens": request.max_tokens.unwrap_or(2048),
            "messages": request.messages,
            "temperature": request.temperature.unwrap_or(0.7),
        });

        let response = self.client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&payload)
            .send()
            .await
            .map_err(|e| crate::error::YoloRouterError::HttpError(e))?;

        if !response.status().is_success() {
            return Err(crate::error::YoloRouterError::RequestError(
                format!("Anthropic API error: {}", response.status())
            ));
        }

        let data: Value = response.json().await
            .map_err(|e| crate::error::YoloRouterError::HttpError(e))?;

        let content = data["content"]
            .get(0)
            .and_then(|c| c["text"].as_str())
            .unwrap_or("No response")
            .to_string();

        Ok(ChatResponse {
            id: uuid::Uuid::new_v4().to_string(),
            model: request.model.clone(),
            choices: vec![Choice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content,
                },
                finish_reason: "stop".to_string(),
            }],
            usage: Usage {
                prompt_tokens: data["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32,
                completion_tokens: data["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
                total_tokens: (data["usage"]["input_tokens"].as_u64().unwrap_or(0)
                    + data["usage"]["output_tokens"].as_u64().unwrap_or(0)) as u32,
            },
        })
    }

    fn name(&self) -> &str {
        "anthropic"
    }

    fn model_list(&self) -> Vec<String> {
        vec![
            "claude-opus".to_string(),
            "claude-sonnet".to_string(),
            "claude-haiku".to_string(),
        ]
    }
}
