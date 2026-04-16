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

#[async_trait]
impl Provider for GitHubCopilotProvider {
    async fn send_request(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let copilot_token = self.get_copilot_token().await?;

        let model = if request.model.is_empty() || request.model == "auto" {
            "gpt-4o".to_string()
        } else {
            request.model.clone()
        };

        let payload = json!({
            "model": model,
            "messages": request.messages,
            "temperature": request.temperature.unwrap_or(0.7),
            "max_completion_tokens": request.max_tokens.unwrap_or(4096),
            "stream": false
        });

        let response = self
            .client
            .post(COPILOT_CHAT_URL)
            .header("Authorization", format!("Bearer {}", copilot_token))
            .header("Content-Type", "application/json")
            .header("Copilot-Integration-Id", "vscode-chat")
            .header("Editor-Version", COPILOT_EDITOR_VERSION)
            .header("Editor-Plugin-Version", COPILOT_PLUGIN_VERSION)
            .header("User-Agent", COPILOT_USER_AGENT)
            .header("x-github-api-version", COPILOT_API_VERSION)
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

        let content = data["choices"]
            .get(0)
            .and_then(|c| c["message"]["content"].as_str())
            .unwrap_or("No response")
            .to_string();

        Ok(ChatResponse {
            id: data["id"].as_str().unwrap_or("").to_string(),
            model: data["model"].as_str().unwrap_or("gpt-4o").to_string(),
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

    async fn start_streaming_request(&self, request: &ChatRequest) -> Result<reqwest::Response> {
        let copilot_token = self.get_copilot_token().await?;

        let model = if request.model.is_empty() || request.model == "auto" {
            "gpt-4o".to_string()
        } else {
            request.model.clone()
        };

        let payload = json!({
            "model": model,
            "messages": request.messages,
            "temperature": request.temperature.unwrap_or(0.7),
            "max_completion_tokens": request.max_tokens.unwrap_or(4096),
            "stream": true
        });

        let response = self
            .client
            .post(COPILOT_CHAT_URL)
            .header("Authorization", format!("Bearer {}", copilot_token))
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .header("Copilot-Integration-Id", "vscode-chat")
            .header("Editor-Version", COPILOT_EDITOR_VERSION)
            .header("Editor-Plugin-Version", COPILOT_PLUGIN_VERSION)
            .header("User-Agent", COPILOT_USER_AGENT)
            .header("x-github-api-version", COPILOT_API_VERSION)
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
