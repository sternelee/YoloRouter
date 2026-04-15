// OpenAI Codex / Azure OpenAI provider
// Supports OpenAI API with custom base URL (Azure, proxies, etc.)

use super::Provider;
use crate::models::{ChatMessage, ChatRequest, ChatResponse, Choice, Usage};
use crate::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;

pub struct CodexProvider {
    api_key: String,
    base_url: String,
    api_version: Option<String>, // For Azure OpenAI
    client: Client,
}

impl CodexProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "https://api.openai.com/v1".to_string(),
            api_version: None,
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .expect("Failed to build HTTP client"),
        }
    }

    pub fn with_azure(api_key: String, endpoint: String, api_version: String) -> Self {
        Self {
            api_key,
            base_url: endpoint,
            api_version: Some(api_version),
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .expect("Failed to build HTTP client"),
        }
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    fn build_url(&self, model: &str) -> String {
        if self.api_version.is_some() {
            // Azure OpenAI format: {endpoint}/openai/deployments/{model}/chat/completions?api-version={version}
            format!(
                "{}/openai/deployments/{}/chat/completions?api-version={}",
                self.base_url,
                model,
                self.api_version.as_deref().unwrap_or("2024-02-01")
            )
        } else {
            format!("{}/chat/completions", self.base_url)
        }
    }

    fn build_auth_header(&self) -> (&'static str, String) {
        if self.api_version.is_some() {
            // Azure uses api-key header
            ("api-key", self.api_key.clone())
        } else {
            ("Authorization", format!("Bearer {}", self.api_key))
        }
    }
}

#[async_trait]
impl Provider for CodexProvider {
    async fn send_request(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let url = self.build_url(&request.model);
        let (auth_header_name, auth_header_value) = self.build_auth_header();

        let payload = json!({
            "model": request.model,
            "messages": request.messages,
            "temperature": request.temperature.unwrap_or(0.7),
            "max_tokens": request.max_tokens.unwrap_or(4096),
        });

        let response = self
            .client
            .post(&url)
            .header(auth_header_name, auth_header_value)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(crate::error::YoloRouterError::HttpError)?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::YoloRouterError::RequestError(format!(
                "Codex API error {}: {}",
                status, body
            )));
        }

        let data: Value = response
            .json()
            .await
            .map_err(crate::error::YoloRouterError::HttpError)?;

        let content = data["choices"]
            .get(0)
            .and_then(|c| c["message"]["content"].as_str())
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
                finish_reason: data["choices"]
                    .get(0)
                    .and_then(|c| c["finish_reason"].as_str())
                    .unwrap_or("stop")
                    .to_string(),
            }],
            usage: Usage {
                prompt_tokens: data["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                completion_tokens: data["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
                total_tokens: data["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
            },
            anthropic_content: None,
            anthropic_stop_sequence: None,
        })
    }

    fn name(&self) -> &str {
        "codex"
    }

    fn model_list(&self) -> Vec<String> {
        vec![
            "o1-preview".to_string(),
            "o1-mini".to_string(),
            "gpt-4o".to_string(),
            "gpt-4-turbo".to_string(),
            "gpt-4".to_string(),
            "gpt-3.5-turbo".to_string(),
        ]
    }
}
