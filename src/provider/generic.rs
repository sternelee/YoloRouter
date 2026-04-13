use super::Provider;
use crate::models::{ChatMessage, ChatRequest, ChatResponse, Choice, Usage};
use crate::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;

pub struct GenericProvider {
    name: String,
    #[allow(dead_code)]
    api_key: String,
    base_url: String,
    models: Vec<String>,
    client: Client,
}

impl GenericProvider {
    pub fn new(name: String, api_key: String, base_url: String, models: Vec<String>) -> Self {
        Self {
            name,
            api_key,
            base_url,
            models,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl Provider for GenericProvider {
    async fn send_request(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let url = format!("{}/chat/completions", self.base_url);

        let payload = serde_json::json!({
            "model": request.model,
            "messages": request.messages,
            "temperature": request.temperature.unwrap_or(0.7),
            "max_tokens": request.max_tokens.unwrap_or(2048),
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&payload)
            .send()
            .await
            .map_err(|e| crate::error::YoloRouterError::HttpError(e))?;

        if !response.status().is_success() {
            return Err(crate::error::YoloRouterError::RequestError(format!(
                "{} API error: {}",
                self.name,
                response.status()
            )));
        }

        let data: Value = response
            .json()
            .await
            .map_err(|e| crate::error::YoloRouterError::HttpError(e))?;

        let content = data["choices"][0]["message"]["content"]
            .as_str()
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
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
        })
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn model_list(&self) -> Vec<String> {
        self.models.clone()
    }
}
