use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "user", "assistant", "system"
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub stream: Option<bool>,
    /// system 可以是字符串或 content blocks 数组（保留原始格式供 AnthropicProvider 使用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<Value>,
    #[serde(default, skip_serializing, skip_deserializing)]
    pub anthropic: Option<AnthropicRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
}

impl ChatRequest {
    pub fn requires_tools(&self) -> bool {
        self.anthropic
            .as_ref()
            .map(AnthropicRequest::requires_tools)
            .unwrap_or_else(|| {
                self.messages
                    .iter()
                    .any(|m| m.content.contains("tool_call") || m.content.contains("function_call"))
            })
    }

    pub fn requires_vision(&self) -> bool {
        self.anthropic
            .as_ref()
            .map(AnthropicRequest::requires_vision)
            .unwrap_or_else(|| {
                self.messages
                    .iter()
                    .any(|m| m.content.contains("image:") || m.content.contains("data:image/"))
            })
    }
}

// ─── Anthropic protocol types ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum AnthropicContent {
    Text(String),
    Blocks(Vec<AnthropicContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnthropicContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<Value>,
    #[serde(flatten, default)]
    pub extra: Map<String, Value>,
}

impl AnthropicContentBlock {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            block_type: "text".to_string(),
            text: Some(text.into()),
            id: None,
            name: None,
            input: None,
            tool_use_id: None,
            content: None,
            extra: Map::new(),
        }
    }

    pub fn is_tool_related(&self) -> bool {
        matches!(self.block_type.as_str(), "tool_use" | "tool_result")
    }

    pub fn is_vision_related(&self) -> bool {
        if matches!(self.block_type.as_str(), "image" | "document") {
            return true;
        }

        self.extra
            .values()
            .any(|value| matches!(value, Value::String(s) if s.starts_with("data:image/")))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: AnthropicContent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum AnthropicBetas {
    Single(String),
    Multiple(Vec<String>),
}

impl AnthropicBetas {
    pub fn values(&self) -> Vec<String> {
        match self {
            AnthropicBetas::Single(value) => vec![value.clone()],
            AnthropicBetas::Multiple(values) => values.clone(),
        }
    }
}

/// Request shape sent by Claude Code / Anthropic SDKs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnthropicRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessage>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// system 可以是字符串或 content blocks 数组（来自 Claude Code）
    #[serde(default)]
    pub system: Option<Value>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub stream: Option<bool>,
    #[serde(default)]
    pub tools: Option<Value>,
    #[serde(default)]
    pub tool_choice: Option<Value>,
    #[serde(default)]
    pub thinking: Option<Value>,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(default)]
    pub betas: Option<AnthropicBetas>,
    #[serde(flatten, default)]
    pub extra: Map<String, Value>,
}

impl AnthropicRequest {
    pub fn requires_tools(&self) -> bool {
        self.tools.is_some()
            || self.tool_choice.is_some()
            || self
                .messages
                .iter()
                .any(|message| message.content.has_tooling())
    }

    pub fn requires_vision(&self) -> bool {
        self.messages
            .iter()
            .any(|message| message.content.has_vision())
    }
}

#[derive(Debug, Serialize)]
pub struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Response shape expected by Claude Code / Anthropic SDKs
#[derive(Debug, Serialize)]
pub struct AnthropicResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub response_type: String,
    pub role: String,
    pub model: String,
    pub content: Vec<AnthropicContentBlock>,
    pub stop_reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
    pub usage: AnthropicUsage,
}

/// Error shape expected by Anthropic SDK clients
#[derive(Debug, Serialize)]
pub struct AnthropicError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub error: AnthropicErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct AnthropicErrorDetail {
    #[serde(rename = "type")]
    pub error_kind: String,
    pub message: String,
}

impl AnthropicContent {
    /// Extract plain text from string or content-block arrays.
    /// Non-text blocks are ignored for the flattened routing view.
    pub fn to_text(&self) -> String {
        match self {
            AnthropicContent::Text(s) => s.clone(),
            AnthropicContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|block| block.text.as_deref())
                .collect::<Vec<_>>()
                .join(""),
        }
    }

    pub fn to_routing_text(&self) -> String {
        match self {
            AnthropicContent::Text(s) => s.clone(),
            AnthropicContent::Blocks(blocks) => {
                let mut parts = Vec::new();
                for block in blocks {
                    match block.block_type.as_str() {
                        "text" => {
                            if let Some(text) = block.text.as_deref() {
                                parts.push(text.to_string());
                            }
                        }
                        "tool_use" | "tool_result" => parts.push(" tool_call ".to_string()),
                        "image" | "document" => parts.push(" image: ".to_string()),
                        _ => {}
                    }
                }
                parts.join("")
            }
        }
    }

    pub fn has_tooling(&self) -> bool {
        match self {
            AnthropicContent::Text(text) => {
                text.contains("tool_call") || text.contains("function_call")
            }
            AnthropicContent::Blocks(blocks) => {
                blocks.iter().any(AnthropicContentBlock::is_tool_related)
            }
        }
    }

    pub fn has_vision(&self) -> bool {
        match self {
            AnthropicContent::Text(text) => text.contains("image:") || text.contains("data:image/"),
            AnthropicContent::Blocks(blocks) => {
                blocks.iter().any(AnthropicContentBlock::is_vision_related)
            }
        }
    }
}

impl From<AnthropicRequest> for ChatRequest {
    fn from(req: AnthropicRequest) -> Self {
        let messages = req
            .messages
            .iter()
            .map(|m| ChatMessage {
                role: m.role.clone(),
                content: m.content.to_routing_text(),
                ..Default::default()
            })
            .collect();
        ChatRequest {
            model: req.model.clone(),
            messages,
            max_tokens: req.max_tokens,
            temperature: req.temperature,
            top_p: req.top_p,
            stream: req.stream,
            system: req.system.clone(),
            anthropic: Some(req.clone()),
            tools: req.tools.clone(),
            tool_choice: req.tool_choice.clone(),
            stop_sequences: req.stop_sequences.clone(),
        }
    }
}

fn map_finish_reason(reason: &str) -> String {
    match reason {
        "stop" => "end_turn",
        "length" => "max_tokens",
        "tool_calls" => "tool_use",
        other => other,
    }
    .to_string()
}

impl From<ChatResponse> for AnthropicResponse {
    fn from(resp: ChatResponse) -> Self {
        let ChatResponse {
            id,
            model,
            choices,
            usage,
            anthropic_content,
            anthropic_stop_sequence,
        } = resp;

        let first_choice = choices.into_iter().next();
        let stop_reason = first_choice
            .as_ref()
            .map(|c| map_finish_reason(&c.finish_reason))
            .unwrap_or_else(|| "end_turn".to_string());
        let fallback_text = first_choice.map(|c| c.message.content).unwrap_or_default();
        let content =
            anthropic_content.unwrap_or_else(|| vec![AnthropicContentBlock::text(fallback_text)]);

        let response_id = if id.starts_with("msg_") {
            id
        } else {
            let raw = id.replace('-', "");
            let id_part = &raw[..raw.len().min(24)];
            format!("msg_{id_part}")
        };

        AnthropicResponse {
            id: response_id,
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            model,
            content,
            stop_reason,
            stop_sequence: anthropic_stop_sequence,
            usage: AnthropicUsage {
                input_tokens: usage.prompt_tokens,
                output_tokens: usage.completion_tokens,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
    #[serde(default, skip_serializing, skip_deserializing)]
    pub anthropic_content: Option<Vec<AnthropicContentBlock>>,
    #[serde(default, skip_serializing, skip_deserializing)]
    pub anthropic_stop_sequence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRequest {
    pub scenario: Option<String>,
    pub request: ChatRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CostTier {
    Low,
    Medium,
    High,
    VeryHigh,
}

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub provider: String,
    pub model_name: String,
    pub cost_tier: CostTier,
    pub capabilities: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn anthropic_request_preserves_structured_fields() {
        let req: AnthropicRequest = serde_json::from_value(json!({
            "model": "claude-sonnet-4-5",
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {"type": "text", "text": "hello"},
                        {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "abc"}}
                    ]
                },
                {
                    "role": "assistant",
                    "content": [
                        {"type": "tool_use", "id": "toolu_1", "name": "Read", "input": {"file_path": "/tmp/x"}}
                    ]
                }
            ],
            "system": [{"type": "text", "text": "system prompt"}],
            "tools": [{"name": "Read", "input_schema": {"type": "object"}}],
            "tool_choice": {"type": "auto"},
            "thinking": {"type": "enabled"},
            "metadata": {"user_id": "abc"},
            "stop_sequences": ["STOP"],
            "container": {"id": "session-1"}
        }))
        .expect("anthropic request should deserialize");

        assert!(req.requires_tools());
        assert!(req.requires_vision());
        assert_eq!(
            req.extra.get("container"),
            Some(&json!({"id": "session-1"}))
        );
        assert!(matches!(req.system, Some(Value::Array(_))));
    }

    #[test]
    fn chat_request_from_anthropic_preserves_native_payload() {
        let req: AnthropicRequest = serde_json::from_value(json!({
            "model": "claude-sonnet-4-5",
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "hi"}]}
            ],
            "tools": [{"name": "Read"}]
        }))
        .expect("anthropic request should deserialize");

        let chat_req = ChatRequest::from(req.clone());
        assert_eq!(chat_req.messages[0].content, "hi");
        assert_eq!(chat_req.anthropic, Some(req));
        assert!(chat_req.requires_tools());
    }

    #[test]
    fn anthropic_response_uses_structured_blocks_when_present() {
        let response = ChatResponse {
            id: "msg_123".to_string(),
            model: "claude-sonnet-4-5".to_string(),
            choices: vec![Choice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content: "ignored text".to_string(),
                    ..Default::default()
                },
                finish_reason: "tool_calls".to_string(),
            }],
            usage: Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            },
            anthropic_content: Some(vec![AnthropicContentBlock {
                block_type: "tool_use".to_string(),
                text: None,
                id: Some("toolu_1".to_string()),
                name: Some("Read".to_string()),
                input: Some(json!({"file_path": "/tmp/x"})),
                tool_use_id: None,
                content: None,
                extra: Map::new(),
            }]),
            anthropic_stop_sequence: Some("STOP".to_string()),
        };

        let anthropic = AnthropicResponse::from(response);
        assert_eq!(anthropic.stop_reason, "tool_use");
        assert_eq!(anthropic.stop_sequence.as_deref(), Some("STOP"));
        assert_eq!(anthropic.content[0].block_type, "tool_use");
        assert_eq!(anthropic.content[0].name.as_deref(), Some("Read"));
    }
}
