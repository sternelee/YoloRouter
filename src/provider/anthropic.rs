use super::Provider;
use crate::models::{
    AnthropicContentBlock, AnthropicRequest, ChatMessage, ChatRequest, ChatResponse, Choice, Usage,
};
use crate::Result;
use async_trait::async_trait;
use reqwest::{Client, Response};
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

    fn extract_beta_values(native: &AnthropicRequest) -> Vec<String> {
        let mut values = Vec::new();

        if let Some(betas) = &native.betas {
            values.extend(betas.values());
        }

        for key in ["anthropic-beta", "anthropic_beta"] {
            if let Some(value) = native.extra.get(key) {
                match value {
                    Value::String(s) => values.push(s.clone()),
                    Value::Array(items) => values.extend(
                        items
                            .iter()
                            .filter_map(|item| item.as_str().map(|s| s.to_string())),
                    ),
                    _ => {}
                }
            }
        }

        let mut deduped = Vec::new();
        for value in values {
            let trimmed = value.trim();
            if !trimmed.is_empty() && !deduped.iter().any(|item| item == trimmed) {
                deduped.push(trimmed.to_string());
            }
        }
        deduped
    }

    fn beta_header_value(native: &AnthropicRequest) -> Option<String> {
        let values = Self::extract_beta_values(native);
        if values.is_empty() {
            None
        } else {
            Some(values.join(","))
        }
    }

    fn build_payload(&self, request: &ChatRequest) -> Value {
        if let Some(native) = &request.anthropic {
            let mut native = native.clone();
            native.betas = None;
            native.extra.remove("betas");
            native.extra.remove("anthropic-beta");
            native.extra.remove("anthropic_beta");

            let mut payload = json!({
                "model": request.model,
                "messages": native.messages,
                "max_tokens": native.max_tokens.or(request.max_tokens).unwrap_or(2048),
                "temperature": native.temperature.or(request.temperature),
                "top_p": native.top_p.or(request.top_p),
            });

            if let Some(system) = native.system.clone().or_else(|| request.system.clone()) {
                payload["system"] = system;
            }
            if let Some(tools) = native.tools.clone() {
                payload["tools"] = tools;
            }
            if let Some(tool_choice) = native.tool_choice.clone() {
                payload["tool_choice"] = tool_choice;
            }
            if let Some(thinking) = native.thinking.clone() {
                payload["thinking"] = thinking;
            }
            if let Some(metadata) = native.metadata.clone() {
                payload["metadata"] = metadata;
            }
            if let Some(stop_sequences) = native.stop_sequences.clone() {
                payload["stop_sequences"] = json!(stop_sequences);
            }

            if let Some(map) = payload.as_object_mut() {
                if map.get("temperature").is_some_and(Value::is_null) {
                    map.remove("temperature");
                }
                if map.get("top_p").is_some_and(Value::is_null) {
                    map.remove("top_p");
                }
                for (key, value) in &native.extra {
                    map.entry(key.clone()).or_insert_with(|| value.clone());
                }
            }

            return payload;
        }

        let system: Option<serde_json::Value> = request.system.clone().or_else(|| {
            request
                .messages
                .iter()
                .find(|m| m.role == "system")
                .map(|m| serde_json::Value::String(m.content.clone()))
        });

        let messages: Vec<_> = request
            .messages
            .iter()
            .filter(|m| m.role != "system")
            .collect();

        let mut payload = json!({
            "model": request.model,
            "max_tokens": request.max_tokens.unwrap_or(2048),
            "messages": messages,
            "temperature": request.temperature.unwrap_or(0.7),
        });

        if let Some(sys) = system {
            payload["system"] = sys;
        }

        payload
    }

    fn request_builder(&self, request: &ChatRequest) -> reqwest::RequestBuilder {
        let url = format!("{}/v1/messages", self.base_url);
        let payload = self.build_payload(request);
        let mut builder = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01");

        if let Some(native) = &request.anthropic {
            if let Some(beta_header) = Self::beta_header_value(native) {
                builder = builder.header("anthropic-beta", beta_header);
            }
        }

        builder.json(&payload)
    }

    pub async fn start_streaming_request(&self, request: &ChatRequest) -> Result<Response> {
        let response = self
            .request_builder(request)
            .header("accept", "text/event-stream")
            .send()
            .await
            .map_err(crate::error::YoloRouterError::HttpError)?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            let message = if body.is_empty() {
                format!("Anthropic API error: {}", status)
            } else {
                format!("Anthropic API error: {} - {}", status, body)
            };
            return Err(crate::error::YoloRouterError::RequestError(message));
        }

        Ok(response)
    }

    fn parse_content_blocks(data: &Value) -> Vec<AnthropicContentBlock> {
        data["content"]
            .as_array()
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|block| serde_json::from_value(block.clone()).ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn extract_text_content(blocks: &[AnthropicContentBlock]) -> String {
        let text = blocks
            .iter()
            .filter_map(|block| block.text.as_deref())
            .collect::<Vec<_>>()
            .join("");

        if text.is_empty() {
            "No response".to_string()
        } else {
            text
        }
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    async fn send_request(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let response = self
            .request_builder(request)
            .send()
            .await
            .map_err(crate::error::YoloRouterError::HttpError)?;

        if !response.status().is_success() {
            return Err(crate::error::YoloRouterError::RequestError(format!(
                "Anthropic API error: {}",
                response.status()
            )));
        }

        let data: Value = response
            .json()
            .await
            .map_err(crate::error::YoloRouterError::HttpError)?;

        let content_blocks = Self::parse_content_blocks(&data);
        let content = Self::extract_text_content(&content_blocks);
        let stop_reason = data["stop_reason"].as_str().unwrap_or("stop").to_string();
        let stop_sequence = data["stop_sequence"]
            .as_str()
            .map(|value| value.to_string());

        Ok(ChatResponse {
            id: data["id"]
                .as_str()
                .map(|value| value.to_string())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            model: data["model"]
                .as_str()
                .unwrap_or(request.model.as_str())
                .to_string(),
            choices: vec![Choice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content,
                },
                finish_reason: stop_reason,
            }],
            usage: Usage {
                prompt_tokens: data["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32,
                completion_tokens: data["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
                total_tokens: (data["usage"]["input_tokens"].as_u64().unwrap_or(0)
                    + data["usage"]["output_tokens"].as_u64().unwrap_or(0))
                    as u32,
            },
            anthropic_content: Some(content_blocks),
            anthropic_stop_sequence: stop_sequence,
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

    async fn start_streaming_request(&self, request: &ChatRequest) -> Result<Response> {
        self.start_streaming_request(request).await
    }

    fn supports_streaming(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AnthropicBetas, AnthropicContent, AnthropicMessage};
    use serde_json::json;

    #[test]
    fn build_payload_prefers_native_anthropic_request() {
        let request = ChatRequest {
            model: "claude-sonnet-4-5".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "flattened".to_string(),
            }],
            temperature: Some(0.4),
            max_tokens: Some(512),
            top_p: Some(0.9),
            stream: None,
            system: Some(json!([{"type": "text", "text": "system prompt"}])),
            anthropic: Some(AnthropicRequest {
                model: "ignored-original-model".to_string(),
                messages: vec![AnthropicMessage {
                    role: "user".to_string(),
                    content: AnthropicContent::Blocks(vec![AnthropicContentBlock::text("hello")]),
                }],
                max_tokens: Some(256),
                system: None,
                temperature: Some(0.2),
                top_p: Some(0.8),
                stream: Some(false),
                tools: Some(json!([{"name": "Read"}])),
                tool_choice: Some(json!({"type": "auto"})),
                thinking: Some(json!({"type": "enabled"})),
                metadata: Some(json!({"user_id": "abc"})),
                stop_sequences: Some(vec!["STOP".to_string()]),
                betas: Some(AnthropicBetas::Multiple(vec![
                    "fine-grained-tool-streaming-2025-05-14".to_string(),
                ])),
                extra: serde_json::from_value(json!({"container": {"id": "session-1"}})).unwrap(),
            }),
        };

        let provider = AnthropicProvider::new("test-key".to_string());
        let payload = provider.build_payload(&request);

        assert_eq!(payload["model"], json!("claude-sonnet-4-5"));
        assert_eq!(payload["messages"][0]["content"][0]["text"], json!("hello"));
        assert_eq!(payload["tools"][0]["name"], json!("Read"));
        assert_eq!(payload["tool_choice"]["type"], json!("auto"));
        assert_eq!(payload["thinking"]["type"], json!("enabled"));
        assert_eq!(payload["metadata"]["user_id"], json!("abc"));
        assert_eq!(payload["stop_sequences"][0], json!("STOP"));
        assert_eq!(payload["container"]["id"], json!("session-1"));
        assert_eq!(payload["system"][0]["text"], json!("system prompt"));
        assert!(payload.get("betas").is_none());
    }

    #[test]
    fn beta_header_value_merges_known_sources() {
        let request = AnthropicRequest {
            model: "claude-sonnet-4-5".to_string(),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: AnthropicContent::Text("hello".to_string()),
            }],
            max_tokens: Some(128),
            system: None,
            temperature: None,
            top_p: None,
            stream: Some(true),
            tools: None,
            tool_choice: None,
            thinking: None,
            metadata: None,
            stop_sequences: None,
            betas: Some(AnthropicBetas::Multiple(vec![
                "fine-grained-tool-streaming-2025-05-14".to_string(),
                "files-api-2025-04-14".to_string(),
            ])),
            extra: serde_json::from_value(json!({
                "anthropic-beta": ["files-api-2025-04-14", "code-execution-2025-02-15"]
            }))
            .unwrap(),
        };

        let header = AnthropicProvider::beta_header_value(&request).unwrap();
        assert_eq!(
            header,
            "fine-grained-tool-streaming-2025-05-14,files-api-2025-04-14,code-execution-2025-02-15"
        );
    }

    #[test]
    fn parse_content_blocks_keeps_tool_use() {
        let data = json!({
            "content": [
                {"type": "text", "text": "hello"},
                {"type": "tool_use", "id": "toolu_1", "name": "Read", "input": {"file_path": "/tmp/x"}}
            ]
        });

        let blocks = AnthropicProvider::parse_content_blocks(&data);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[1].block_type, "tool_use");
        assert_eq!(blocks[1].name.as_deref(), Some("Read"));
    }
}
