use crate::models::{ChatRequest, ChatResponse};
use crate::config::schema::ScenarioConfig;
use crate::router::ProviderRegistry;
use crate::Result;

pub struct FallbackChain {
    scenario: ScenarioConfig,
}

impl FallbackChain {
    pub fn new(scenario: ScenarioConfig) -> Self {
        Self { scenario }
    }

    pub async fn execute(
        &self,
        request: &ChatRequest,
        registry: &ProviderRegistry,
        max_retries: u32,
    ) -> Result<ChatResponse> {
        let mut last_error: Option<String> = None;

        // Try each model in the scenario's model list
        for (index, model_config) in self.scenario.models.iter().enumerate() {
            for attempt in 0..=max_retries {
                if attempt > 0 {
                    tracing::info!(
                        "Fallback attempt {} for provider: {}",
                        attempt,
                        model_config.provider
                    );
                }

                if let Some(provider) = registry.get(&model_config.provider) {
                    let mut req = request.clone();
                    req.model = model_config.model.clone();

                    match provider.send_request(&req).await {
                        Ok(response) => {
                            tracing::info!(
                                "Successfully routed to provider: {} model: {} (fallback index: {})",
                                model_config.provider,
                                model_config.model,
                                index
                            );
                            return Ok(response);
                        }
                        Err(e) => {
                            last_error = Some(e.to_string());
                            tracing::warn!(
                                "Provider {} failed: {}. Trying next fallback...",
                                model_config.provider,
                                e
                            );
                        }
                    }
                } else {
                    last_error = Some(format!("Provider not found: {}", model_config.provider));
                    tracing::warn!("Provider not found: {}", model_config.provider);
                    break;
                }
            }
        }

        // All attempts failed
        Err(crate::error::YoloRouterError::AllProvidersFailed(
            last_error.unwrap_or_else(|| "No providers available in scenario".to_string()),
        ))
    }

    pub fn model_chain_info(&self) -> Vec<String> {
        self.scenario
            .models
            .iter()
            .map(|m| format!("{}:{}", m.provider, m.model))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::{ModelConfig, ScenarioConfig};

    #[test]
    fn test_fallback_chain_creation() {
        let scenario = ScenarioConfig {
            models: vec![
                ModelConfig {
                    provider: "openai".to_string(),
                    model: "gpt-4".to_string(),
                    cost_tier: None,
                    capabilities: None,
                    fallback_to: None,
                },
                ModelConfig {
                    provider: "anthropic".to_string(),
                    model: "claude-opus".to_string(),
                    cost_tier: None,
                    capabilities: None,
                    fallback_to: None,
                },
            ],
            default_tier: None,
            match_task_types: vec![],
            match_languages: vec![],
            priority: 0,
            is_default: false,
        };

        let chain = FallbackChain::new(scenario);
        assert_eq!(chain.scenario.models.len(), 2);
    }

    #[test]
    fn test_fallback_model_chain_info() {
        let scenario = ScenarioConfig {
            models: vec![
                ModelConfig {
                    provider: "openai".to_string(),
                    model: "gpt-4".to_string(),
                    cost_tier: None,
                    capabilities: None,
                    fallback_to: None,
                },
                ModelConfig {
                    provider: "anthropic".to_string(),
                    model: "claude-opus".to_string(),
                    cost_tier: None,
                    capabilities: None,
                    fallback_to: None,
                },
            ],
            default_tier: None,
            match_task_types: vec![],
            match_languages: vec![],
            priority: 0,
            is_default: false,
        };

        let chain = FallbackChain::new(scenario);
        let info = chain.model_chain_info();
        assert_eq!(info.len(), 2);
        assert_eq!(info[0], "openai:gpt-4");
        assert_eq!(info[1], "anthropic:claude-opus");
    }
}
