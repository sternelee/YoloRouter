use super::Provider;
use crate::models::{ChatMessage, ChatRequest, ChatResponse, Choice, Usage};
use crate::Result;
use async_trait::async_trait;
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
        let model = if request.model.is_empty() || request.model == "auto" {
            "gemini-pro"
        } else {
            &request.model
        };
        let url = format!("{}/v1beta/models/{}:generateContent", self.base_url, model);

        let mut contents = serde_json::Map::new();
        contents.insert(
            "contents".to_string(),
            json!([{
                "parts": request.messages.iter().map(|m| {
                    json!({ "text": &m.content })
                }).collect::<Vec<_>>()
            }]),
        );

        let mut payload = Value::Object(contents);

        // Pass through generation config if set
        let mut gen_config = serde_json::Map::new();
        if let Some(temp) = request.temperature {
            gen_config.insert("temperature".to_string(), json!(temp));
        }
        if let Some(max_tokens) = request.max_tokens {
            gen_config.insert("maxOutputTokens".to_string(), json!(max_tokens));
        }
        if !gen_config.is_empty() {
            payload["generationConfig"] = Value::Object(gen_config);
        }

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("x-goog-api-key", &self.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(crate::error::YoloRouterError::HttpError)?;

        if !response.status().is_success() {
            return Err(crate::error::YoloRouterError::RequestError(format!(
                "Gemini API error: {}",
                response.status()
            )));
        }

        let data: Value = response
            .json()
            .await
            .map_err(crate::error::YoloRouterError::HttpError)?;

        let content = data["candidates"]
            .get(0)
            .and_then(|c| c["content"]["parts"].get(0))
            .and_then(|p| p["text"].as_str())
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
            anthropic_content: None,
            anthropic_stop_sequence: None,
        })
    }

    fn name(&self) -> &str {
        "gemini"
    }

    fn model_list(&self) -> Vec<String> {
        vec!["gemini-pro".to_string(), "gemini-pro-vision".to_string()]
    }
}
