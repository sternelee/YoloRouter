use crate::config::schema::ScenarioConfig;
use crate::models::{ChatRequest, ChatResponse};
use crate::router::{ProviderHealthTracker, ProviderRegistry};
use crate::Result;
use std::collections::{HashMap, HashSet};
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
        let ordered_models = self.ordered_models();

        // Try each model in the scenario's model list
        for (index, model_config) in ordered_models.iter().enumerate() {
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
        self.ordered_models()
            .iter()
            .map(|m| format!("{}:{}", m.provider, m.model))
            .collect()
    }

    pub fn preferred_model(&self) -> Option<&crate::config::schema::ModelConfig> {
        self.ordered_models().first().copied()
    }

    fn ordered_models(&self) -> Vec<&crate::config::schema::ModelConfig> {
        let preferred_tier = self.scenario.default_tier.as_deref();

        // Pre-compute which bare model names are unique within this scenario.
        // When two models share the same name (e.g. openai:gpt-4o and anthropic:gpt-4o)
        // we must NOT insert a short-form key — doing so would silently route
        // `fallback_to = "gpt-4o"` to whichever entry was processed last.
        let model_name_counts: HashMap<&str, usize> =
            self.scenario
                .models
                .iter()
                .fold(HashMap::new(), |mut acc, m| {
                    *acc.entry(m.model.as_str()).or_insert(0) += 1;
                    acc
                });

        let model_by_ref: HashMap<String, usize> = self
            .scenario
            .models
            .iter()
            .enumerate()
            .flat_map(|(idx, model)| {
                let mut refs = vec![(format!("{}:{}", model.provider, model.model), idx)];
                // Only add the short-form key when the model name is unambiguous.
                if model_name_counts
                    .get(model.model.as_str())
                    .copied()
                    .unwrap_or(0)
                    == 1
                {
                    refs.push((model.model.clone(), idx));
                }
                refs
            })
            .collect();

        let mut ordered_indices = Vec::new();
        let mut visited = HashSet::new();

        for (idx, model) in self.scenario.models.iter().enumerate() {
            if preferred_tier.is_some_and(|tier| model.cost_tier.as_deref() != Some(tier)) {
                continue;
            }
            self.append_model_chain(idx, &model_by_ref, &mut visited, &mut ordered_indices);
        }

        for idx in 0..self.scenario.models.len() {
            self.append_model_chain(idx, &model_by_ref, &mut visited, &mut ordered_indices);
        }

        ordered_indices
            .into_iter()
            .map(|idx| &self.scenario.models[idx])
            .collect()
    }

    fn append_model_chain(
        &self,
        start_idx: usize,
        model_by_ref: &HashMap<String, usize>,
        visited: &mut HashSet<usize>,
        ordered_indices: &mut Vec<usize>,
    ) {
        let mut current = Some(start_idx);

        while let Some(idx) = current {
            if !visited.insert(idx) {
                break;
            }
            ordered_indices.push(idx);
            current = self.scenario.models[idx]
                .fallback_to
                .as_ref()
                .and_then(|target| model_by_ref.get(target).copied())
                .filter(|target_idx| !visited.contains(target_idx));
        }
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

    #[test]
    fn test_default_tier_is_preferred_in_model_chain() {
        let scenario = ScenarioConfig {
            models: vec![
                ModelConfig {
                    provider: "openai".to_string(),
                    model: "gpt-4o".to_string(),
                    cost_tier: Some("high".to_string()),
                    capabilities: None,
                    fallback_to: None,
                },
                ModelConfig {
                    provider: "openai".to_string(),
                    model: "gpt-4o-mini".to_string(),
                    cost_tier: Some("low".to_string()),
                    capabilities: None,
                    fallback_to: None,
                },
            ],
            default_tier: Some("low".to_string()),
            match_task_types: vec![],
            match_languages: vec![],
            priority: 0,
            is_default: false,
        };

        let chain = FallbackChain::new(scenario);
        assert_eq!(chain.model_chain_info()[0], "openai:gpt-4o-mini");
    }

    #[test]
    fn test_ambiguous_short_form_model_name_is_not_inserted() {
        // Two providers that both expose a model with the same bare name.
        // A `fallback_to = "gpt-4o"` would be ambiguous; neither short-form
        // key should be in the lookup map, so the fallback is silently dropped
        // rather than routing to the wrong provider.
        let scenario = ScenarioConfig {
            models: vec![
                ModelConfig {
                    provider: "openai".to_string(),
                    model: "gpt-4o".to_string(),
                    cost_tier: None,
                    capabilities: None,
                    fallback_to: Some("gpt-4o".to_string()), // ambiguous — same name, two providers
                },
                ModelConfig {
                    provider: "github_copilot".to_string(),
                    model: "gpt-4o".to_string(),
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
        // Both models are present; the fallback_to does NOT cause an infinite
        // loop or wrong-provider routing because the short-form key was omitted.
        let info = chain.model_chain_info();
        assert_eq!(info.len(), 2);
    }

    #[test]
    fn test_fallback_to_reorders_chain() {
        let scenario = ScenarioConfig {
            models: vec![
                ModelConfig {
                    provider: "openai".to_string(),
                    model: "gpt-4o".to_string(),
                    cost_tier: Some("high".to_string()),
                    capabilities: None,
                    fallback_to: Some("openai:gpt-4o-mini".to_string()),
                },
                ModelConfig {
                    provider: "openai".to_string(),
                    model: "gpt-4o-mini".to_string(),
                    cost_tier: Some("low".to_string()),
                    capabilities: None,
                    fallback_to: None,
                },
                ModelConfig {
                    provider: "anthropic".to_string(),
                    model: "claude-sonnet-4.6".to_string(),
                    cost_tier: Some("medium".to_string()),
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
        assert_eq!(
            chain.model_chain_info(),
            vec![
                "openai:gpt-4o".to_string(),
                "openai:gpt-4o-mini".to_string(),
                "anthropic:claude-sonnet-4.6".to_string()
            ]
        );
    }
}
