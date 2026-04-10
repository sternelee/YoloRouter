use super::*;
use crate::config::schema::ProviderConfig;
use crate::{Result, YoloRouterError};
use std::sync::Arc;

pub struct ProviderFactory;

impl ProviderFactory {
    pub fn create_provider(name: &str, config: &ProviderConfig) -> Result<Arc<dyn Provider>> {
        match config.provider_type.as_str() {
            "anthropic" => {
                let api_key = config.api_key.clone()
                    .ok_or_else(|| YoloRouterError::ConfigError("Missing api_key for anthropic provider".to_string()))?;
                Ok(Arc::new(AnthropicProvider::new(api_key)))
            }
            "openai" => {
                let api_key = config.api_key.clone()
                    .ok_or_else(|| YoloRouterError::ConfigError("Missing api_key for openai provider".to_string()))?;
                Ok(Arc::new(OpenAIProvider::new(api_key)))
            }
            "gemini" => {
                let api_key = config.api_key.clone()
                    .ok_or_else(|| YoloRouterError::ConfigError("Missing api_key for gemini provider".to_string()))?;
                Ok(Arc::new(GeminiProvider::new(api_key)))
            }
            _ => {
                let api_key = config.api_key.clone()
                    .ok_or_else(|| YoloRouterError::ConfigError(format!("Missing api_key for {} provider", config.provider_type)))?;
                let base_url = config.base_url.clone()
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
