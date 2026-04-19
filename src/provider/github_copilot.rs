// GitHub Copilot provider with OAuth device flow authentication
// Supports the GitHub Copilot Chat API

use super::Provider;
use crate::models::{ChatMessage, ChatRequest, ChatResponse, Choice, Usage};
use crate::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const GITHUB_CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";
const GITHUB_DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const GITHUB_COPILOT_TOKEN_URL: &str = "https://api.github.com/copilot_internal/v2/token";
const COPILOT_CHAT_URL: &str = "https://api.githubcopilot.com/chat/completions";

// Header constants kept in sync with VS Code Copilot extension
const COPILOT_EDITOR_VERSION: &str = "vscode/1.110.1";
const COPILOT_PLUGIN_VERSION: &str = "copilot-chat/0.38.2";
const COPILOT_USER_AGENT: &str = "GitHubCopilotChat/0.38.2";
const COPILOT_API_VERSION: &str = "2025-10-01";

#[derive(Debug, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Deserialize)]
pub struct AccessTokenResponse {
    pub access_token: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CopilotToken {
    pub token: String,
    #[serde(default, deserialize_with = "deserialize_optional_int_as_string")]
    pub expires_at: Option<String>,
    #[serde(rename = "sku")]
    pub sku: Option<String>,
}

/// Deserialize an optional field that can be either a string or an integer (Unix timestamp).
fn deserialize_optional_int_as_string<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Option<Value> = Option::deserialize(deserializer)?;
    match value {
        Some(Value::String(s)) => Ok(Some(s)),
        Some(Value::Number(n)) => Ok(Some(n.to_string())),
        _ => Ok(None),
    }
}

pub struct GitHubCopilotProvider {
    github_token: String,
    client_id: String,
    copilot_token: tokio::sync::RwLock<Option<CopilotToken>>,
    client: Client,
}

impl GitHubCopilotProvider {
    pub fn new(github_token: String) -> Self {
        Self::new_with_client_id(github_token, GITHUB_CLIENT_ID.to_string())
    }

    pub fn new_with_client_id(github_token: String, client_id: String) -> Self {
        Self {
            github_token,
            client_id,
            copilot_token: tokio::sync::RwLock::new(None),
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to build HTTP client"),
        }
    }

    /// Request device code for OAuth flow - call this to start auth
    pub async fn request_device_code(&self) -> Result<DeviceCodeResponse> {
        let resp = self
            .client
            .post(GITHUB_DEVICE_CODE_URL)
            .header("Accept", "application/json")
            .header("User-Agent", COPILOT_USER_AGENT)
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("scope", "read:user"),
            ])
            .send()
            .await
            .map_err(crate::error::YoloRouterError::HttpError)?;

        resp.json::<DeviceCodeResponse>()
            .await
            .map_err(crate::error::YoloRouterError::HttpError)
    }

    /// Poll for access token after user visits device URL
    pub async fn poll_for_token(
        &self,
        device_code: &str,
        interval_secs: u64,
        timeout_secs: u64,
    ) -> Result<String> {
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);

        loop {
            if Instant::now() > deadline {
                return Err(crate::error::YoloRouterError::RequestError(
                    "Device auth timeout: user did not authorize in time".to_string(),
                ));
            }

            tokio::time::sleep(Duration::from_secs(interval_secs)).await;

            let resp = self
                .client
                .post(GITHUB_TOKEN_URL)
                .header("Accept", "application/json")
                .header("User-Agent", COPILOT_USER_AGENT)
                .form(&[
                    ("client_id", self.client_id.as_str()),
                    ("device_code", device_code),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ])
                .send()
                .await
                .map_err(crate::error::YoloRouterError::HttpError)?;

            let token_resp: AccessTokenResponse = resp
                .json()
                .await
                .map_err(crate::error::YoloRouterError::HttpError)?;

            match token_resp.error.as_deref() {
                Some("authorization_pending") => continue,
                Some("slow_down") => {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
                Some("expired_token") | Some("access_denied") => {
                    return Err(crate::error::YoloRouterError::RequestError(format!(
                        "Device auth failed: {}",
                        token_resp.error.unwrap_or_default()
                    )));
                }
                None => {
                    if let Some(token) = token_resp.access_token {
                        return Ok(token);
                    }
                }
                Some(e) => {
                    return Err(crate::error::YoloRouterError::RequestError(format!(
                        "Device auth error: {}",
                        e
                    )));
                }
            }
        }
    }

    /// Exchange GitHub token for Copilot API token
    async fn get_copilot_token(&self) -> Result<String> {
        // Check cached token with expiry
        {
            let cached = self.copilot_token.read().await;
            if let Some(ref token) = *cached {
                if let Some(ref expires_at) = token.expires_at {
                    if let Ok(ts) = expires_at.parse::<i64>() {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64;
                        // Refresh 60s before expiry
                        if now < ts - 60 {
                            return Ok(token.token.clone());
                        }
                        // Token expired or expiring soon — fall through to refresh
                        tracing::info!("Copilot token expired or expiring soon, refreshing");
                    }
                }
                // No expires_at or unparseable — use cached token as-is
                else {
                    return Ok(token.token.clone());
                }
            }
        }

        // Fetch new copilot token
        let resp = self
            .client
            .get(GITHUB_COPILOT_TOKEN_URL)
            .header("Authorization", format!("token {}", self.github_token))
            .header("Accept", "application/json")
            .header("User-Agent", COPILOT_USER_AGENT)
            .header("Editor-Version", COPILOT_EDITOR_VERSION)
            .header("Editor-Plugin-Version", COPILOT_PLUGIN_VERSION)
            .send()
            .await
            .map_err(crate::error::YoloRouterError::HttpError)?;

        if !resp.status().is_success() {
            return Err(crate::error::YoloRouterError::RequestError(format!(
                "Failed to get Copilot token: {}",
                resp.status()
            )));
        }

        let copilot_token: CopilotToken = resp
            .json()
            .await
            .map_err(crate::error::YoloRouterError::HttpError)?;

        let token = copilot_token.token.clone();

        // Cache the token
        let mut cached = self.copilot_token.write().await;
        *cached = Some(copilot_token);

        Ok(token)
    }
}

impl GitHubCopilotProvider {
    fn resolve_model(request: &ChatRequest) -> String {
        if request.model.is_empty() || request.model == "auto" {
            "gpt-4o".to_string()
        } else {
            request.model.clone()
        }
    }

    /// Reasoning models (o1, o3, o4-mini, gpt-5, codex, computer-use) don't support temperature/top_p.
    fn is_reasoning_model(model: &str) -> bool {
        model.starts_with("o1")
            || model.starts_with("o3")
            || model.starts_with("o4")
            || model.starts_with("gpt-5")
            || model.starts_with("codex-")
            || model.starts_with("computer-use")
    }

    /// Build the request payload. Mirrors opencode-dev's copilot chat logic.
    fn build_payload(&self, request: &ChatRequest, stream: bool) -> Value {
        let model = Self::resolve_model(request);
        let is_reasoning = Self::is_reasoning_model(&model);

        let mut payload = json!({
            "model": model,
            "messages": request.messages,
        });

        // Reasoning models don't support temperature/top_p
        if !is_reasoning {
            if let Some(temp) = request.temperature {
                payload["temperature"] = json!(temp);
            }
            if let Some(top_p) = request.top_p {
                payload["top_p"] = json!(top_p);
            }
        }

        // max_completion_tokens: opencode-dev omits it for plain gpt models,
        // but keeps it for o-series and others. We keep it for all non-gpt-4o
        // and make it optional for gpt-4o to match Copilot CLI behavior.
        if let Some(max_tokens) = request.max_tokens {
            payload["max_completion_tokens"] = json!(max_tokens);
        } else if is_reasoning {
            // Reasoning models benefit from an explicit limit
            payload["max_completion_tokens"] = json!(4096);
        }
        // For regular gpt models, omit max_completion_tokens when not set
        // to let Copilot use its own defaults.

        if let Some(tools) = request.tools.clone() {
            payload["tools"] = tools;
        }
        if let Some(tool_choice) = request.tool_choice.clone() {
            payload["tool_choice"] = tool_choice;
        }
        if let Some(stop_sequences) = request.stop_sequences.clone() {
            payload["stop_sequences"] = json!(stop_sequences);
        }
        if stream {
            payload["stream"] = json!(true);
        } else {
            payload["stream"] = json!(false);
        }

        payload
    }

    fn common_headers(&self, copilot_token: &str) -> Vec<(&str, String)> {
        vec![
            ("Authorization", format!("Bearer {}", copilot_token)),
            ("Content-Type", "application/json".to_string()),
            ("Copilot-Integration-Id", "vscode-chat".to_string()),
            ("Editor-Version", COPILOT_EDITOR_VERSION.to_string()),
            ("Editor-Plugin-Version", COPILOT_PLUGIN_VERSION.to_string()),
            ("User-Agent", COPILOT_USER_AGENT.to_string()),
            ("x-github-api-version", COPILOT_API_VERSION.to_string()),
        ]
    }
}

#[async_trait]
impl Provider for GitHubCopilotProvider {
    async fn send_request(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let copilot_token = self.get_copilot_token().await?;
        let payload = self.build_payload(request, false);
        let model = Self::resolve_model(request);

        let mut builder = self.client.post(COPILOT_CHAT_URL);
        for (key, value) in self.common_headers(&copilot_token) {
            builder = builder.header(key, value);
        }

        let response = builder
            .json(&payload)
            .send()
            .await
            .map_err(crate::error::YoloRouterError::HttpError)?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::YoloRouterError::RequestError(format!(
                "GitHub Copilot API error {}: {}",
                status, body
            )));
        }

        let data: Value = response
            .json()
            .await
            .map_err(crate::error::YoloRouterError::HttpError)?;

        let choice = data["choices"].get(0);
        let message_obj = choice
            .and_then(|c| c.get("message"))
            .unwrap_or(&Value::Null);

        let content = message_obj["content"].as_str().unwrap_or("").to_string();
        let tool_calls = message_obj.get("tool_calls").cloned();
        let refusal = message_obj["refusal"].as_str().map(|s| s.to_string());
        let reasoning_content = message_obj["reasoning_content"]
            .as_str()
            .map(|s| s.to_string());

        Ok(ChatResponse {
            id: data["id"].as_str().unwrap_or("").to_string(),
            model: data["model"].as_str().unwrap_or(&model).to_string(),
            choices: vec![Choice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content,
                    tool_calls,
                    refusal,
                    reasoning_content,
                },
                finish_reason: choice
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

    async fn start_streaming_request(&self, request: &ChatRequest) -> Result<reqwest::Response> {
        let copilot_token = self.get_copilot_token().await?;
        let payload = self.build_payload(request, true);

        let mut builder = self
            .client
            .post(COPILOT_CHAT_URL)
            .header("Accept", "text/event-stream");
        for (key, value) in self.common_headers(&copilot_token) {
            builder = builder.header(key, value);
        }

        let response = builder
            .json(&payload)
            .send()
            .await
            .map_err(crate::error::YoloRouterError::HttpError)?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::YoloRouterError::RequestError(format!(
                "GitHub Copilot API error {}: {}",
                status, body
            )));
        }

        Ok(response)
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "github_copilot"
    }

    fn model_list(&self) -> Vec<String> {
        crate::provider::models::static_provider_models("github_copilot").unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_is_reasoning_model() {
        assert!(GitHubCopilotProvider::is_reasoning_model("o1-mini"));
        assert!(GitHubCopilotProvider::is_reasoning_model("o1-preview"));
        assert!(GitHubCopilotProvider::is_reasoning_model("o3-mini"));
        assert!(GitHubCopilotProvider::is_reasoning_model("o4-mini"));
        assert!(GitHubCopilotProvider::is_reasoning_model("gpt-5.4"));
        assert!(GitHubCopilotProvider::is_reasoning_model("gpt-5-mini"));
        assert!(GitHubCopilotProvider::is_reasoning_model("codex-latest"));
        assert!(GitHubCopilotProvider::is_reasoning_model(
            "computer-use-preview"
        ));
        assert!(!GitHubCopilotProvider::is_reasoning_model("gpt-4o"));
        assert!(!GitHubCopilotProvider::is_reasoning_model("gpt-4.1"));
        assert!(!GitHubCopilotProvider::is_reasoning_model(
            "claude-sonnet-4.6"
        ));
    }

    #[test]
    fn test_build_payload_passthrough_fields() {
        let provider = GitHubCopilotProvider::new("test-token".to_string());
        let request = ChatRequest {
            model: "gpt-4o".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "hi".to_string(),
                ..Default::default()
            }],
            temperature: Some(0.5),
            max_tokens: Some(1024),
            top_p: Some(0.9),
            stream: Some(true),
            system: None,
            anthropic: None,
            tools: Some(json!([{"name": "Read"}])),
            tool_choice: Some(json!({"type": "auto"})),
            stop_sequences: Some(vec!["STOP".to_string()]),
        };

        let payload = provider.build_payload(&request, false);
        assert_eq!(payload["model"], "gpt-4o");
        assert_eq!(payload["temperature"], 0.5);
        assert_eq!(payload["max_completion_tokens"], 1024);
        assert!((payload["top_p"].as_f64().unwrap() - 0.9).abs() < 0.001);
        assert_eq!(payload["tools"][0]["name"], "Read");
        assert_eq!(payload["tool_choice"]["type"], "auto");
        assert_eq!(payload["stop_sequences"][0], "STOP");
        assert_eq!(payload["stream"], false);

        let stream_payload = provider.build_payload(&request, true);
        assert_eq!(stream_payload["stream"], true);
    }

    #[test]
    fn test_build_payload_reasoning_model_skips_temperature_top_p() {
        let provider = GitHubCopilotProvider::new("test-token".to_string());
        let request = ChatRequest {
            model: "o3-mini".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "think".to_string(),
                ..Default::default()
            }],
            temperature: Some(0.7),
            max_tokens: Some(2048),
            top_p: Some(0.9),
            stream: None,
            system: None,
            anthropic: None,
            tools: None,
            tool_choice: None,
            stop_sequences: None,
        };

        let payload = provider.build_payload(&request, false);
        assert_eq!(payload["model"], "o3-mini");
        assert_eq!(payload["max_completion_tokens"], 2048);
        assert!(payload.get("temperature").is_none());
        assert!(payload.get("top_p").is_none());
    }

    #[test]
    fn test_build_payload_gpt_omits_max_tokens_when_unset() {
        let provider = GitHubCopilotProvider::new("test-token".to_string());
        let request = ChatRequest {
            model: "gpt-4o".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "hi".to_string(),
                ..Default::default()
            }],
            temperature: None,
            max_tokens: None,
            top_p: None,
            stream: None,
            system: None,
            anthropic: None,
            tools: None,
            tool_choice: None,
            stop_sequences: None,
        };

        let payload = provider.build_payload(&request, false);
        // gpt-4o without explicit max_tokens should omit max_completion_tokens
        assert!(payload.get("max_completion_tokens").is_none());
        assert!(payload.get("temperature").is_none());
        assert!(payload.get("top_p").is_none());
    }

    #[test]
    fn test_build_payload_reasoning_defaults_max_tokens() {
        let provider = GitHubCopilotProvider::new("test-token".to_string());
        let request = ChatRequest {
            model: "gpt-5.4".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "hi".to_string(),
                ..Default::default()
            }],
            temperature: None,
            max_tokens: None,
            top_p: None,
            stream: None,
            system: None,
            anthropic: None,
            tools: None,
            tool_choice: None,
            stop_sequences: None,
        };

        let payload = provider.build_payload(&request, false);
        // reasoning models get a default max_completion_tokens
        assert_eq!(payload["max_completion_tokens"], 4096);
    }
}
