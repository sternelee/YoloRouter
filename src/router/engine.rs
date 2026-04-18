use super::{FallbackChain, ProviderRegistry};
use crate::analyzer::{match_scenario_by_model_scores, FastAnalyzer, ModelCandidate, ScenarioMeta};
use crate::config::Config;
use crate::models::{ChatRequest, ChatResponse};
use crate::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::timeout;

pub struct RoutingEngine {
    config: Arc<RwLock<Config>>,
    registry: Arc<ProviderRegistry>,
    analyzer: FastAnalyzer,
}

impl Default for RoutingEngine {
    fn default() -> Self {
        Self::new_empty()
    }
}

impl RoutingEngine {
    pub fn new_empty() -> Self {
        Self {
            config: Arc::new(RwLock::new(Config {
                daemon: None,
                providers: HashMap::new(),
                scenarios: HashMap::new(),
                routing: None,
            })),
            registry: Arc::new(ProviderRegistry::new()),
            analyzer: FastAnalyzer::new(),
        }
    }

    pub fn new_with_config(config: Config) -> Result<Self> {
        let registry = ProviderRegistry::from_config(&config)?;
        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            registry: Arc::new(registry),
            analyzer: FastAnalyzer::new(),
        })
    }

    pub async fn route(
        &self,
        request: &ChatRequest,
        scenario: Option<&str>,
        tracker: &crate::router::ProviderHealthTracker,
    ) -> Result<ChatResponse> {
        let config = self.config.read().await;
        let routing_config = config.routing();
        let timeout_duration = Duration::from_millis(routing_config.timeout_ms);

        // Explicit scenario wins immediately
        if let Some(scenario_name) = scenario {
            return self
                .route_via_scenario(request, scenario_name, &config, timeout_duration, tracker)
                .await;
        }

        // Direct routing: "provider:model" format — user explicitly chose provider
        // Skip when model is "auto" to let the analyzer handle it
        if request.model != "auto" {
            let model_parts: Vec<&str> = request.model.split(':').collect();
            if model_parts.len() == 2 {
                let provider_name = model_parts[0];
                if let Some(provider) = self.registry.get(provider_name) {
                    tracing::info!(
                        provider = provider_name,
                        model = model_parts[1],
                        "Direct routing via provider:model format"
                    );
                    let mut req = request.clone();
                    req.model = model_parts[1].to_string();
                    return timeout(timeout_duration, provider.send_request(&req))
                        .await
                        .map_err(|_| {
                            crate::error::YoloRouterError::RequestError(
                                "Request timeout".to_string(),
                            )
                        })?;
                } else {
                    tracing::warn!(
                        provider = provider_name,
                        "Provider not found for direct routing, falling back to auto-routing"
                    );
                }
            }

            // Bare model name inference: if exactly one configured provider
            // advertises this model name, route there directly and skip the
            // analyzer.  This is intentional — the caller explicitly chose a
            // model, so cost/scenario analysis would only add noise.
            // If the name is ambiguous (multiple providers match), this returns
            // None and we fall through to the analyzer below.
            if let Some(provider_name) = self.resolve_provider_for_model(&request.model, &config) {
                if let Some(provider) = self.registry.get(&provider_name) {
                    tracing::info!(
                        provider = provider_name,
                        model = request.model,
                        "Direct routing via inferred provider for model"
                    );
                    return timeout(timeout_duration, provider.send_request(request))
                        .await
                        .map_err(|_| {
                            crate::error::YoloRouterError::RequestError(
                                "Request timeout".to_string(),
                            )
                        })?;
                }
            }
        }

        // Auto-routing via analyzer
        let scenarios = config.scenarios();
        if !scenarios.is_empty() {
            let candidates: Vec<ModelCandidate> = scenarios
                .values()
                .flat_map(|sc| {
                    sc.models.iter().map(|m| ModelCandidate {
                        id: format!("{}/{}", m.provider, m.model),
                        provider: m.provider.clone(),
                        model: m.model.clone(),
                        capabilities: m.capabilities.clone().unwrap_or_default(),
                        cost_tier: m.cost_tier.clone().unwrap_or_else(|| "medium".to_string()),
                    })
                })
                .collect();

            let (analysis, scores) = self
                .analyzer
                .analyze_and_score(&request.messages, &candidates);

            let scenario_data: Vec<ScenarioMeta<'_>> = scenarios
                .iter()
                .map(|(name, sc)| {
                    (
                        name.as_str(),
                        sc.match_task_types.as_slice(),
                        sc.match_languages.as_slice(),
                        sc.priority,
                        sc.is_default,
                    )
                })
                .collect();

            let scenario_model_ids: HashMap<String, Vec<String>> = scenarios
                .iter()
                .map(|(name, sc)| {
                    (
                        name.clone(),
                        sc.models
                            .iter()
                            .map(|m| format!("{}/{}", m.provider, m.model))
                            .collect(),
                    )
                })
                .collect();

            tracing::debug!(
                task_type = analysis.features.task_type.as_str(),
                language = analysis.features.language.as_str(),
                confidence = analysis.features.confidence,
                "Analyzer result for auto-routing"
            );

            if let Some(scenario_name) = match_scenario_by_model_scores(
                &analysis,
                &scenario_data,
                &scenario_model_ids,
                &scores,
                routing_config.confidence_threshold,
            ) {
                return self
                    .route_via_scenario(request, &scenario_name, &config, timeout_duration, tracker)
                    .await;
            }
        }

        Err(crate::error::YoloRouterError::RoutingError(
            format!(
                "No routing decision for model '{}'. Configure scenarios/defaults or use provider:model",
                request.model
            ),
        ))
    }

    /// Select the best model for a request without executing it.
    /// Returns (provider_name, model_name) for the selected model.
    pub async fn select_best_model(
        &self,
        request: &ChatRequest,
        scenario: Option<&str>,
    ) -> Result<(String, String)> {
        let config = self.config.read().await;
        let routing_config = config.routing();

        // Explicit scenario wins immediately
        if let Some(scenario_name) = scenario {
            if let Ok(scenario_config) = config.get_scenario(scenario_name) {
                let fallback = FallbackChain::new(scenario_config);
                if let Some(model_config) = fallback.preferred_model() {
                    return Ok((model_config.provider.clone(), model_config.model.clone()));
                }
            }
        }

        // Direct routing: "provider:model" format
        if request.model != "auto" {
            let model_parts: Vec<&str> = request.model.split(':').collect();
            if model_parts.len() == 2 {
                let provider_name = model_parts[0];
                if self.registry.get(provider_name).is_some() {
                    return Ok((provider_name.to_string(), model_parts[1].to_string()));
                }
            }

            if let Some(provider_name) = self.resolve_provider_for_model(&request.model, &config) {
                return Ok((provider_name, request.model.clone()));
            }
        }

        // Auto-routing via analyzer
        let scenarios = config.scenarios();
        if !scenarios.is_empty() {
            let candidates: Vec<ModelCandidate> = scenarios
                .values()
                .flat_map(|sc| {
                    sc.models.iter().map(|m| ModelCandidate {
                        id: format!("{}/{}", m.provider, m.model),
                        provider: m.provider.clone(),
                        model: m.model.clone(),
                        capabilities: m.capabilities.clone().unwrap_or_default(),
                        cost_tier: m.cost_tier.clone().unwrap_or_else(|| "medium".to_string()),
                    })
                })
                .collect();

            let (analysis, scores) = self
                .analyzer
                .analyze_and_score(&request.messages, &candidates);

            let scenario_data: Vec<ScenarioMeta<'_>> = scenarios
                .iter()
                .map(|(name, sc)| {
                    (
                        name.as_str(),
                        sc.match_task_types.as_slice(),
                        sc.match_languages.as_slice(),
                        sc.priority,
                        sc.is_default,
                    )
                })
                .collect();

            let scenario_model_ids: HashMap<String, Vec<String>> = scenarios
                .iter()
                .map(|(name, sc)| {
                    (
                        name.clone(),
                        sc.models
                            .iter()
                            .map(|m| format!("{}/{}", m.provider, m.model))
                            .collect(),
                    )
                })
                .collect();

            if let Some(scenario_name) = match_scenario_by_model_scores(
                &analysis,
                &scenario_data,
                &scenario_model_ids,
                &scores,
                routing_config.confidence_threshold,
            ) {
                if let Ok(scenario_config) = config.get_scenario(&scenario_name) {
                    let fallback = FallbackChain::new(scenario_config);
                    if let Some(model_config) = fallback.preferred_model() {
                        return Ok((model_config.provider.clone(), model_config.model.clone()));
                    }
                }
            }
        }

        Err(crate::error::YoloRouterError::RoutingError(
            format!(
                "No routing decision for model '{}'. Configure scenarios/defaults or use provider:model",
                request.model
            ),
        ))
    }

    async fn route_via_scenario(
        &self,
        request: &ChatRequest,
        scenario_name: &str,
        config: &Config,
        timeout_duration: Duration,
        tracker: &crate::router::ProviderHealthTracker,
    ) -> Result<ChatResponse> {
        let routing_config = config.routing();
        if let Ok(scenario_config) = config.get_scenario(scenario_name) {
            let fallback = FallbackChain::new(scenario_config);
            if routing_config.fallback_enabled {
                let cooldown = if routing_config.cooldown_enabled {
                    Duration::from_secs(routing_config.cooldown_secs)
                } else {
                    Duration::ZERO
                };
                return timeout(
                    timeout_duration,
                    fallback.execute(
                        request,
                        &self.registry,
                        routing_config.retry_count,
                        tracker,
                        cooldown,
                    ),
                )
                .await
                .map_err(|_| {
                    crate::error::YoloRouterError::RequestError("Request timeout".to_string())
                })?;
            } else if let Some(model_config) = fallback.preferred_model() {
                if let Some(provider) = self.registry.get(&model_config.provider) {
                    let mut req = request.clone();
                    req.model = model_config.model.clone();
                    return timeout(timeout_duration, provider.send_request(&req))
                        .await
                        .map_err(|_| {
                            crate::error::YoloRouterError::RequestError(
                                "Request timeout".to_string(),
                            )
                        })?;
                }
            }
        }
        Err(crate::error::YoloRouterError::RoutingError(format!(
            "Scenario '{}' not found or has no configured models",
            scenario_name
        )))
    }

    pub async fn get_config(&self) -> Config {
        self.config.read().await.clone()
    }

    pub fn registry(&self) -> &ProviderRegistry {
        &self.registry
    }

    fn resolve_provider_for_model(&self, model: &str, config: &Config) -> Option<String> {
        let mut matches = self.registry.list().into_iter().filter(|provider_name| {
            self.registry
                .get(provider_name)
                .map(|provider| provider.model_list().iter().any(|known| known == model))
                .unwrap_or(false)
        });

        if let Some(first) = matches.next() {
            return if matches.next().is_some() {
                None
            } else {
                Some(first)
            };
        }

        let mut configured_matches = config
            .scenarios()
            .values()
            .flat_map(|scenario| scenario.models.iter())
            .filter(|model_config| model_config.model == model)
            .map(|model_config| model_config.provider.clone());

        let first = configured_matches.next()?;
        if configured_matches.any(|provider| provider != first) {
            None
        } else {
            Some(first)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_tiered_scenario() -> Config {
        Config::from_string(
            r#"
[providers.openai]
type = "openai"
api_key = "test-key"

[providers.anthropic]
type = "anthropic"
api_key = "test-key"

[scenarios.coding]
default_tier = "low"
match_task_types = ["coding"]
models = [
    { provider = "openai", model = "gpt-4o", cost_tier = "high", fallback_to = "openai:gpt-4o-mini" },
    { provider = "openai", model = "gpt-4o-mini", cost_tier = "low" },
    { provider = "anthropic", model = "claude-sonnet-4.6", cost_tier = "medium" }
]

[routing]
fallback_enabled = false
"#,
        )
        .unwrap()
    }

    fn coding_request() -> ChatRequest {
        ChatRequest {
            model: "auto".to_string(),
            messages: vec![crate::models::ChatMessage {
                role: "user".to_string(),
                content: "write code to implement a rust parser".to_string(),
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
        }
    }

    #[tokio::test]
    async fn explicit_scenario_uses_preferred_tier_model() {
        let engine = RoutingEngine::new_with_config(config_with_tiered_scenario()).unwrap();
        let selected = engine
            .select_best_model(&coding_request(), Some("coding"))
            .await
            .unwrap();

        assert_eq!(selected, ("openai".to_string(), "gpt-4o-mini".to_string()));
    }

    #[tokio::test]
    async fn explicit_model_resolves_provider_without_first_provider_fallback() {
        let engine = RoutingEngine::new_with_config(config_with_tiered_scenario()).unwrap();
        let mut request = coding_request();
        request.model = "claude-sonnet-4.6".to_string();

        let selected = engine.select_best_model(&request, None).await.unwrap();
        assert_eq!(
            selected,
            ("anthropic".to_string(), "claude-sonnet-4.6".to_string())
        );
    }

    #[tokio::test]
    async fn explicit_unknown_model_returns_clear_error() {
        let engine = RoutingEngine::new_with_config(config_with_tiered_scenario()).unwrap();
        let mut request = coding_request();
        request.model = "unknown-model".to_string();

        let error = engine.select_best_model(&request, None).await.unwrap_err();
        assert!(error
            .to_string()
            .contains("Configure scenarios/defaults or use provider:model"));
    }
}
