use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "user", "assistant", "system"
    pub content: String,
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
    /// Top-level system prompt preserved from Anthropic format; used by AnthropicProvider
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
}

// ─── Anthropic protocol types ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum AnthropicContent {
    Text(String),
    Blocks(Vec<AnthropicBlock>),
}

#[derive(Debug, Deserialize)]
pub struct AnthropicBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: AnthropicContent,
}

/// Request shape sent by Claude Code / Anthropic SDKs
#[derive(Debug, Deserialize)]
pub struct AnthropicRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessage>,
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub system: Option<String>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
}

#[derive(Debug, Serialize)]
pub struct AnthropicContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: String,
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
    /// Non-text blocks (tool_use, image, etc.) are skipped.
    pub fn to_text(&self) -> String {
        match self {
            AnthropicContent::Text(s) => s.clone(),
            AnthropicContent::Blocks(blocks) => blocks
                .iter()
                .filter(|b| b.block_type == "text")
                .filter_map(|b| b.text.as_deref())
                .collect::<Vec<_>>()
                .join(""),
        }
    }
}

impl From<AnthropicRequest> for ChatRequest {
    fn from(req: AnthropicRequest) -> Self {
        let messages = req
            .messages
            .into_iter()
            .map(|m| ChatMessage {
                role: m.role,
                content: m.content.to_text(),
            })
            .collect();
        ChatRequest {
            model: req.model,
            messages,
            max_tokens: req.max_tokens,
            temperature: req.temperature,
            top_p: req.top_p,
            system: req.system,
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
        let (content_text, stop_reason) = resp
            .choices
            .into_iter()
            .next()
            .map(|c| (c.message.content, map_finish_reason(&c.finish_reason)))
            .unwrap_or_else(|| ("".to_string(), "end_turn".to_string()));

        let raw = resp.id.replace('-', "");
        let id_part = &raw[..raw.len().min(24)];

        AnthropicResponse {
            id: format!("msg_{id_part}"),
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            model: resp.model,
            content: vec![AnthropicContentBlock {
                block_type: "text".to_string(),
                text: content_text,
            }],
            stop_reason,
            stop_sequence: None,
            usage: AnthropicUsage {
                input_tokens: resp.usage.prompt_tokens,
                output_tokens: resp.usage.completion_tokens,
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
