use super::*;
use crate::config::schema::ProviderConfig;
use crate::{Result, YoloRouterError};
use std::sync::Arc;

pub struct ProviderFactory;

impl ProviderFactory {
    pub fn create_provider(name: &str, config: &ProviderConfig) -> Result<Arc<dyn Provider>> {
        match config.provider_type.as_str() {
            "anthropic" => {
                let api_key = config.api_key.clone().ok_or_else(|| {
                    YoloRouterError::ConfigError(
                        "Missing api_key for anthropic provider".to_string(),
                    )
                })?;
                Ok(Arc::new(AnthropicProvider::new(api_key)))
            }
            "openai" => {
                let api_key = config.api_key.clone().ok_or_else(|| {
                    YoloRouterError::ConfigError("Missing api_key for openai provider".to_string())
                })?;
                let mut p = OpenAIProvider::new(api_key);
                if let Some(base_url) = &config.base_url {
                    p = p.with_base_url(base_url.clone());
                }
                Ok(Arc::new(p))
            }
            "gemini" => {
                let api_key = config.api_key.clone().ok_or_else(|| {
                    YoloRouterError::ConfigError("Missing api_key for gemini provider".to_string())
                })?;
                Ok(Arc::new(GeminiProvider::new(api_key)))
            }
            "github_copilot" | "github" => {
                // Prefer token (long-lived GitHub OAuth token), fall back to api_key
                let token = config
                    .token
                    .clone()
                    .or_else(|| config.api_key.clone())
                    .ok_or_else(|| {
                        YoloRouterError::ConfigError(
                            "Missing token/api_key for github_copilot provider".to_string(),
                        )
                    })?;
                // client_id can be overridden via extra.client_id
                let client_id = config
                    .extra
                    .get("client_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Iv1.b507a08c87ecfe98")
                    .to_string();
                Ok(Arc::new(GitHubCopilotProvider::new_with_client_id(
                    token, client_id,
                )))
            }
            "codex" => {
                let api_key = config.api_key.clone().ok_or_else(|| {
                    YoloRouterError::ConfigError("Missing api_key for codex provider".to_string())
                })?;

                // Check for Azure-specific config in extra
                let azure_endpoint = config
                    .extra
                    .get("azure_endpoint")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let api_version = config
                    .extra
                    .get("api_version")
                    .and_then(|v| v.as_str())
                    .map(String::from);

                let provider =
                    if let (Some(endpoint), Some(version)) = (azure_endpoint, api_version) {
                        CodexProvider::with_azure(api_key, endpoint, version)
                    } else {
                        let mut p = CodexProvider::new(api_key);
                        if let Some(base_url) = &config.base_url {
                            p = p.with_base_url(base_url.clone());
                        }
                        p
                    };

                Ok(Arc::new(provider))
            }
            "codex_oauth" => {
                // ChatGPT Plus/Pro OAuth — token persisted in ~/.config/yolo-router/codex_oauth.json
                let token_path = config
                    .extra
                    .get("token_path")
                    .and_then(|v| v.as_str())
                    .map(std::path::PathBuf::from)
                    .or_else(|| {
                        dirs::config_dir().map(|d| d.join("yolo-router").join("codex_oauth.json"))
                    });

                // If an access_token is explicitly provided in config, use it
                if let Some(access_token) = config.api_key.clone().or(config.token.clone()) {
                    let refresh = config
                        .extra
                        .get("refresh_token")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    Ok(Arc::new(CodexOAuthProvider::with_access_token(
                        access_token,
                        refresh,
                        token_path,
                    )))
                } else {
                    // No key in config — load persisted token from disk
                    Ok(Arc::new(CodexOAuthProvider::new(token_path)))
                }
            }
            _ => {
                let api_key = config.api_key.clone().ok_or_else(|| {
                    YoloRouterError::ConfigError(format!(
                        "Missing api_key for {} provider",
                        config.provider_type
                    ))
                })?;
                let base_url = config
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "https://api.example.com/v1".to_string());
                Ok(Arc::new(GenericProvider::new(
                    name.to_string(),
                    api_key,
                    base_url,
                    vec!["gpt-3.5".to_string()],
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::ProviderConfig;
    use std::collections::HashMap;

    #[test]
    fn test_create_anthropic_provider() {
        let config = ProviderConfig {
            provider_type: "anthropic".to_string(),
            api_key: Some("test-key".to_string()),
            auth_type: None,
            token: None,
            base_url: None,
            extra: HashMap::new(),
        };
        let provider = ProviderFactory::create_provider("anthropic", &config).unwrap();
        assert_eq!(provider.name(), "anthropic");
    }

    #[test]
    fn test_create_provider_missing_api_key() {
        let config = ProviderConfig {
            provider_type: "openai".to_string(),
            api_key: None,
            auth_type: None,
            token: None,
            base_url: None,
            extra: HashMap::new(),
        };
        let result = ProviderFactory::create_provider("openai", &config);
        assert!(result.is_err());
    }
}
