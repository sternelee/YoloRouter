use crate::config::schema::ScenarioConfig;
use crate::models::{ChatRequest, ChatResponse};
use crate::router::{ProviderHealthTracker, ProviderRegistry};
use crate::Result;
use std::time::Duration;

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
        tracker: &ProviderHealthTracker,
        cooldown: Duration,
    ) -> Result<ChatResponse> {
        let mut last_error: Option<String> = None;

        // Try each model in the scenario's model list
        for (index, model_config) in self.scenario.models.iter().enumerate() {
            // Check cooldown before attempting this provider
            if tracker.is_cooling_down(&model_config.provider, cooldown) {
                let remaining = tracker
                    .remaining(&model_config.provider, cooldown)
                    .unwrap_or_default();
                tracing::warn!(
                    provider = %model_config.provider,
                    remaining_secs = remaining.as_secs(),
                    "Provider is cooling down, skipping"
                );
                last_error = Some(format!(
                    "Provider '{}' is cooling down ({} secs remaining)",
                    model_config.provider,
                    remaining.as_secs()
                ));
                continue;
            }

            if let Some(provider) = registry.get(&model_config.provider) {
                let mut req = request.clone();
                req.model = model_config.model.clone();

                // When cooldown is enabled, only attempt once — any failure starts cooldown
                // and we move on to the next provider. When cooldown is disabled, honour
                // max_retries for the same provider.
                let max_attempts = if cooldown.is_zero() {
                    max_retries + 1
                } else {
                    1
                };
                let mut attempt = 0u32;
                while attempt < max_attempts {
                    if attempt > 0 {
                        tracing::info!(
                            provider = %model_config.provider,
                            attempt,
                            "Retrying provider"
                        );
                    }
                    match provider.send_request(&req).await {
                        Ok(response) => {
                            tracing::info!(
                                provider = %model_config.provider,
                                model = %model_config.model,
                                fallback_index = index,
                                "Successfully routed to provider"
                            );
                            tracker.record_success(&model_config.provider);
                            return Ok(response);
                        }
                        Err(e) => {
                            last_error = Some(e.to_string());
                            tracing::warn!(
                                provider = %model_config.provider,
                                cooldown_secs = cooldown.as_secs(),
                                error = %e,
                                "Provider failed, entering cooldown"
                            );
                            tracker.record_failure(&model_config.provider);
                            attempt += 1;
                        }
                    }
                }
            } else {
                last_error = Some(format!("Provider not found: {}", model_config.provider));
                tracing::warn!(provider = %model_config.provider, "Provider not found");
                // Not a transient failure — no cooldown recorded
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

    fn make_scenario(providers: &[(&str, &str)]) -> ScenarioConfig {
        ScenarioConfig {
            models: providers
                .iter()
                .map(|(provider, model)| ModelConfig {
                    provider: provider.to_string(),
                    model: model.to_string(),
                    cost_tier: None,
                    capabilities: None,
                    fallback_to: None,
                })
                .collect(),
            default_tier: None,
            match_task_types: vec![],
            match_languages: vec![],
            priority: 0,
            is_default: false,
        }
    }

    #[test]
    fn test_fallback_chain_creation() {
        let chain = FallbackChain::new(make_scenario(&[
            ("openai", "gpt-4"),
            ("anthropic", "claude-opus"),
        ]));
        assert_eq!(chain.scenario.models.len(), 2);
    }

    #[test]
    fn test_fallback_model_chain_info() {
        let chain = FallbackChain::new(make_scenario(&[
            ("openai", "gpt-4"),
            ("anthropic", "claude-opus"),
        ]));
        let info = chain.model_chain_info();
        assert_eq!(info.len(), 2);
        assert_eq!(info[0], "openai:gpt-4");
        assert_eq!(info[1], "anthropic:claude-opus");
    }

    #[test]
    fn test_cooling_provider_is_skipped() {
        let tracker = ProviderHealthTracker::new();
        tracker.record_failure("openai");
        assert!(tracker.is_cooling_down("openai", Duration::from_secs(60)));
        assert!(!tracker.is_cooling_down("anthropic", Duration::from_secs(60)));
    }
}
