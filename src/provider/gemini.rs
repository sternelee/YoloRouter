use crate::models::{ChatRequest, ChatResponse, Choice, ChatMessage, Usage};
use crate::Result;
use async_trait::async_trait;
use super::Provider;
use reqwest::Client;
use serde_json::{json, Value};

pub struct GeminiProvider {
    #[allow(dead_code)]
    api_key: String,
    #[allow(dead_code)]
    base_url: String,
    client: Client,
}

impl GeminiProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "https://generativelanguage.googleapis.com".to_string(),
            client: Client::new(),
        }
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }
}

#[async_trait]
impl Provider for GeminiProvider {
    async fn send_request(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let url = format!("{}/v1beta/models/gemini-pro:generateContent?key={}", self.base_url, self.api_key);
        
        let payload = json!({
            "contents": [{
                "parts": request.messages.iter().map(|m| {
                    json!({ "text": &m.content })
                }).collect::<Vec<_>>()
            }]
        });

        let response = self.client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| crate::error::YoloRouterError::HttpError(e))?;

        if !response.status().is_success() {
            return Err(crate::error::YoloRouterError::RequestError(
                format!("Gemini API error: {}", response.status())
            ));
        }

        let data: Value = response.json().await
            .map_err(|e| crate::error::YoloRouterError::HttpError(e))?;

        let content = data["candidates"][0]["content"]["parts"][0]["text"]
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
        "gemini"
    }

    fn model_list(&self) -> Vec<String> {
        vec![
            "gemini-pro".to_string(),
            "gemini-pro-vision".to_string(),
        ]
    }
}
