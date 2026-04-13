// OpenAI / ChatGPT Codex OAuth device-flow provider
//
// Auth flow (mirrors cc-switch codex_oauth_auth.rs):
//   1. POST https://auth.openai.com/api/accounts/deviceauth/usercode
//      body: {"client_id": "..."} → {device_auth_id, user_code, expires_in, interval?}
//   2. Show user_code and verification URL to user
//   3. Poll https://auth.openai.com/api/accounts/deviceauth/token
//      body: {device_auth_id, user_code}
//      403/404 = pending, 410 = expired, 200 → {authorization_code, code_verifier}
//   4. POST https://auth.openai.com/oauth/token (form-encoded)
//      grant_type=authorization_code, code, redirect_uri, client_id, code_verifier
//      → {access_token, refresh_token?, expires_in?}
//   5. Refresh: POST same URL with grant_type=refresh_token, refresh_token, client_id, scope

use super::Provider;
use crate::models::{ChatMessage, ChatRequest, ChatResponse, Choice, Usage};
use crate::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

// ─── Constants ────────────────────────────────────────────────────────────────

/// OpenAI OAuth client ID (same as used by OpenCode / official Codex CLI)
pub const CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

const DEVICE_AUTH_USERCODE_URL: &str = "https://auth.openai.com/api/accounts/deviceauth/usercode";
const DEVICE_AUTH_TOKEN_URL: &str = "https://auth.openai.com/api/accounts/deviceauth/token";
const OAUTH_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const DEVICE_REDIRECT_URI: &str = "https://auth.openai.com/deviceauth/callback";

/// URL shown to the user during device flow
pub const DEVICE_VERIFICATION_URL: &str = "https://auth.openai.com/codex/device";

/// Refresh 60 s before expiry
const TOKEN_REFRESH_BUFFER_MS: i64 = 60_000;

// ─── HTTP response types ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct RawDeviceCodeResponse {
    device_auth_id: String,
    user_code: String,
    /// May be a number or absent
    #[serde(default)]
    interval: Option<Value>,
    #[serde(default)]
    expires_in: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct DevicePollSuccess {
    authorization_code: String,
    code_verifier: String,
}

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

// ─── Public display type for TUI ─────────────────────────────────────────────

/// Info returned by `start_device_flow()`, ready for TUI display.
#[derive(Debug, Clone)]
pub struct CodexDeviceCodeDisplay {
    pub device_auth_id: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval_secs: u64,
}

// ─── Persistent token state ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodexTokenState {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    /// Unix milliseconds
    pub expires_at_ms: Option<i64>,
}

impl CodexTokenState {
    pub fn is_valid(&self) -> bool {
        self.access_token.is_some() && !self.is_expiring_soon()
    }

    fn is_expiring_soon(&self) -> bool {
        if let Some(exp) = self.expires_at_ms {
            now_ms() >= exp - TOKEN_REFRESH_BUFFER_MS
        } else {
            false
        }
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn parse_interval(v: Option<&Value>) -> u64 {
    v.and_then(|v| v.as_u64()).unwrap_or(5)
}

// ─── Provider ─────────────────────────────────────────────────────────────────

pub struct CodexOAuthProvider {
    client_id: String,
    token_state: Arc<RwLock<CodexTokenState>>,
    token_path: Option<PathBuf>,
    client: Client,
}

impl CodexOAuthProvider {
    /// Create provider, loading persisted token from `token_path` if it exists.
    pub fn new(token_path: Option<PathBuf>) -> Self {
        let token_state = token_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<CodexTokenState>(&s).ok())
            .unwrap_or_default();

        Self {
            client_id: CODEX_CLIENT_ID.to_string(),
            token_state: Arc::new(RwLock::new(token_state)),
            token_path,
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to build HTTP client"),
        }
    }

    /// Create provider with a pre-known access token (e.g., from TOML config).
    pub fn with_access_token(
        access_token: String,
        refresh_token: Option<String>,
        token_path: Option<PathBuf>,
    ) -> Self {
        let provider = Self::new(token_path);
        // Overwrite in-memory state — we're in a sync constructor, use try_write
        if let Ok(mut state) = provider.token_state.try_write() {
            *state = CodexTokenState {
                access_token: Some(access_token),
                refresh_token,
                expires_at_ms: None,
            };
        }
        provider
    }

    // ─── Device flow ──────────────────────────────────────────────────────────

    /// Step 1: request a device code from OpenAI.
    pub async fn start_device_flow(&self) -> Result<CodexDeviceCodeDisplay> {
        let resp = self
            .client
            .post(DEVICE_AUTH_USERCODE_URL)
            .header("Content-Type", "application/json")
            .header("User-Agent", "yolo-router-codex-oauth/1.0")
            .json(&json!({ "client_id": self.client_id }))
            .send()
            .await
            .map_err(crate::error::YoloRouterError::HttpError)?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(crate::error::YoloRouterError::RequestError(format!(
                "Codex device code request failed: {status} - {text}"
            )));
        }

        let raw: RawDeviceCodeResponse = resp
            .json()
            .await
            .map_err(crate::error::YoloRouterError::HttpError)?;

        Ok(CodexDeviceCodeDisplay {
            device_auth_id: raw.device_auth_id,
            user_code: raw.user_code,
            verification_uri: DEVICE_VERIFICATION_URL.to_string(),
            expires_in: raw.expires_in.unwrap_or(900),
            interval_secs: parse_interval(raw.interval.as_ref()),
        })
    }

    /// Step 3: poll for authorization.
    ///
    /// Returns `Ok(None)` while still pending (403 / 404).
    /// Returns `Ok(Some((auth_code, code_verifier)))` when the user has authorized.
    /// Returns `Err` on expiry (410) or other errors.
    pub async fn poll_device_flow(
        &self,
        device_auth_id: &str,
        user_code: &str,
    ) -> Result<Option<(String, String)>> {
        let resp = self
            .client
            .post(DEVICE_AUTH_TOKEN_URL)
            .header("Content-Type", "application/json")
            .header("User-Agent", "yolo-router-codex-oauth/1.0")
            .json(&json!({
                "device_auth_id": device_auth_id,
                "user_code": user_code,
            }))
            .send()
            .await
            .map_err(crate::error::YoloRouterError::HttpError)?;

        let status = resp.status();

        if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None); // still pending
        }

        if status == reqwest::StatusCode::GONE {
            return Err(crate::error::YoloRouterError::RequestError(
                "Codex device code expired".to_string(),
            ));
        }

        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(crate::error::YoloRouterError::RequestError(format!(
                "Codex poll failed: {status} - {text}"
            )));
        }

        let success: DevicePollSuccess = resp
            .json()
            .await
            .map_err(crate::error::YoloRouterError::HttpError)?;

        Ok(Some((success.authorization_code, success.code_verifier)))
    }

    /// Step 4: exchange authorization_code + code_verifier for tokens.
    /// Automatically persists to `token_path` if set.
    pub async fn exchange_code(&self, code: &str, code_verifier: &str) -> Result<CodexTokenState> {
        let resp = self
            .client
            .post(OAUTH_TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("User-Agent", "yolo-router-codex-oauth/1.0")
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", DEVICE_REDIRECT_URI),
                ("client_id", &self.client_id),
                ("code_verifier", code_verifier),
            ])
            .send()
            .await
            .map_err(crate::error::YoloRouterError::HttpError)?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(crate::error::YoloRouterError::RequestError(format!(
                "Codex token exchange failed: {status} - {text}"
            )));
        }

        let tok: OAuthTokenResponse = resp
            .json()
            .await
            .map_err(crate::error::YoloRouterError::HttpError)?;

        let state = CodexTokenState {
            access_token: Some(tok.access_token),
            refresh_token: tok.refresh_token,
            expires_at_ms: tok.expires_in.map(|e| now_ms() + e * 1000),
        };

        self.persist_and_cache(state.clone()).await;
        Ok(state)
    }

    /// Refresh access_token using stored refresh_token.
    async fn refresh_access_token(&self) -> Result<String> {
        let refresh_token = {
            let s = self.token_state.read().await;
            s.refresh_token.clone().ok_or_else(|| {
                crate::error::YoloRouterError::RequestError(
                    "No Codex refresh token. Re-authenticate: yolo-router --auth codex".to_string(),
                )
            })?
        };

        let resp = self
            .client
            .post(OAUTH_TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("User-Agent", "yolo-router-codex-oauth/1.0")
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", &refresh_token),
                ("client_id", &self.client_id),
                ("scope", "openid profile email"),
            ])
            .send()
            .await
            .map_err(crate::error::YoloRouterError::HttpError)?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(crate::error::YoloRouterError::RequestError(
                "Codex refresh token invalid. Re-authenticate: yolo-router --auth codex"
                    .to_string(),
            ));
        }

        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(crate::error::YoloRouterError::RequestError(format!(
                "Codex token refresh failed: {status} - {text}"
            )));
        }

        let tok: OAuthTokenResponse = resp
            .json()
            .await
            .map_err(crate::error::YoloRouterError::HttpError)?;

        let new_token = tok.access_token.clone();
        let state = CodexTokenState {
            access_token: Some(tok.access_token),
            refresh_token: tok.refresh_token.or(Some(refresh_token)),
            expires_at_ms: tok.expires_in.map(|e| now_ms() + e * 1000),
        };

        self.persist_and_cache(state).await;
        Ok(new_token)
    }

    /// Get a valid access token, refreshing automatically if needed.
    pub async fn get_valid_token(&self) -> Result<String> {
        {
            let s = self.token_state.read().await;
            if s.is_valid() {
                return Ok(s.access_token.clone().unwrap());
            }
            if s.access_token.is_some() && s.refresh_token.is_some() {
                // About to expire; fall through to refresh
            } else if s.access_token.is_none() {
                return Err(crate::error::YoloRouterError::RequestError(
                    "No Codex OAuth token. Authenticate with: yolo-router --auth codex".to_string(),
                ));
            }
        }
        self.refresh_access_token().await
    }

    async fn persist_and_cache(&self, state: CodexTokenState) {
        if let Some(ref path) = self.token_path {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(data) = serde_json::to_string_pretty(&state) {
                let _ = std::fs::write(path, data);
            }
        }
        *self.token_state.write().await = state;
    }
}

#[async_trait]
impl Provider for CodexOAuthProvider {
    async fn send_request(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let token = self.get_valid_token().await?;

        let model = if request.model.is_empty() || request.model == "auto" {
            "gpt-4o".to_string()
        } else {
            request.model.clone()
        };

        let payload = json!({
            "model": model,
            "messages": request.messages,
            "temperature": request.temperature.unwrap_or(0.7),
            "max_tokens": request.max_tokens.unwrap_or(4096),
        });

        let response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(crate::error::YoloRouterError::HttpError)?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::YoloRouterError::RequestError(format!(
                "Codex OAuth API error {status}: {body}"
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
            model: data["model"].as_str().unwrap_or(&model).to_string(),
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
        })
    }

    fn name(&self) -> &str {
        "codex_oauth"
    }

    fn model_list(&self) -> Vec<String> {
        vec![
            // GPT-5 series (latest frontier models)
            "gpt-5.4".to_string(),              // Current - Latest frontier agentic coding model
            "gpt-5.4-mini".to_string(),         // Smaller frontier agentic coding model
            "gpt-5.3-codex".to_string(),        // Frontier Codex-optimized agentic coding model
            "gpt-5.2".to_string(),              // Optimized for professional work and long-running agents
            "gpt-5.2-codex".to_string(),        // Codex variant
            "gpt-5.1".to_string(),              // Previous stable release
            "gpt-5-mini".to_string(),           // Lightweight variant
            
            // GPT-4 series (stable)
            "gpt-4o".to_string(),               // GPT-4 optimized
            "gpt-4o-mini".to_string(),          // Lightweight GPT-4o
            "gpt-4-turbo".to_string(),          // High-performance variant
            "gpt-4".to_string(),                // Base GPT-4
            
            // Reasoning models (o-series)
            "o1".to_string(),                   // Latest reasoning model
            "o1-preview".to_string(),           // Preview of o1
            "o1-mini".to_string(),              // Lightweight reasoning
            
            // Legacy models
            "gpt-3.5-turbo".to_string(),        // Previous generation
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_state_is_valid_when_has_token() {
        let state = CodexTokenState {
            access_token: Some("tok_abc".to_string()),
            refresh_token: None,
            expires_at_ms: None, // no expiry = doesn't expire
        };
        assert!(state.is_valid());
    }

    #[test]
    fn test_token_state_invalid_when_no_token() {
        let state = CodexTokenState::default();
        assert!(!state.is_valid());
    }

    #[test]
    fn test_token_state_expiring_soon() {
        let soon_ms = now_ms() + 30_000; // expires in 30s (< 60s buffer)
        let state = CodexTokenState {
            access_token: Some("tok_abc".to_string()),
            refresh_token: Some("refresh_xyz".to_string()),
            expires_at_ms: Some(soon_ms),
        };
        assert!(!state.is_valid()); // expiring soon → not "valid"
    }

    #[test]
    fn test_token_state_not_expiring() {
        let later_ms = now_ms() + 3600_000; // expires in 1h
        let state = CodexTokenState {
            access_token: Some("tok_abc".to_string()),
            refresh_token: None,
            expires_at_ms: Some(later_ms),
        };
        assert!(state.is_valid());
    }

    #[test]
    fn test_parse_interval_from_value() {
        let v = serde_json::json!(5u64);
        assert_eq!(parse_interval(Some(&v)), 5);
        assert_eq!(parse_interval(None), 5); // default
    }
}
