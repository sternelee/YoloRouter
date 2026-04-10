use crate::models::{ChatRequest, ChatResponse, Choice, ChatMessage, Usage};
use crate::Result;
use async_trait::async_trait;
use super::Provider;
use reqwest::Client;
use serde_json::{json, Value};

pub struct OpenAIProvider {
    #[allow(dead_code)]
    api_key: String,
    #[allow(dead_code)]
    base_url: String,
    client: Client,
}

impl OpenAIProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "https://api.openai.com/v1".to_string(),
            client: Client::new(),
        }
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }
}

#[async_trait]
impl Provider for OpenAIProvider {
    async fn send_request(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let url = format!("{}/chat/completions", self.base_url);
        
        let payload = json!({
            "model": request.model,
            "messages": request.messages,
            "temperature": request.temperature.unwrap_or(0.7),
            "max_tokens": request.max_tokens.unwrap_or(2048),
        });

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&payload)
            .send()
            .await
            .map_err(|e| crate::error::YoloRouterError::HttpError(e))?;

        if !response.status().is_success() {
            return Err(crate::error::YoloRouterError::RequestError(
                format!("OpenAI API error: {}", response.status())
            ));
        }

        let data: Value = response.json().await
            .map_err(|e| crate::error::YoloRouterError::HttpError(e))?;

        let content = data["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("No response")
            .to_string();

        Ok(ChatResponse {
            id: data["id"].as_str().unwrap_or("").to_string(),
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
                prompt_tokens: data["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                completion_tokens: data["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
                total_tokens: data["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
            },
        })
    }

    fn name(&self) -> &str {
        "openai"
    }

    fn model_list(&self) -> Vec<String> {
        vec![
            "gpt-4".to_string(),
            "gpt-3.5-turbo".to_string(),
        ]
    }
}
