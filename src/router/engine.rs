use super::{FallbackChain, ProviderRegistry};
use crate::analyzer::{match_scenario, FastAnalyzer, ModelCandidate, ScenarioMeta};
use crate::config::Config;
use crate::models::{ChatRequest, ChatResponse};
use crate::Result;
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
                providers: None,
                scenarios: None,
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
    ) -> Result<ChatResponse> {
        let config = self.config.read().await;
        let routing_config = config.routing();
        let timeout_duration = Duration::from_millis(routing_config.timeout_ms);

        // Explicit scenario wins immediately
        if let Some(scenario_name) = scenario {
            return self
                .route_via_scenario(request, scenario_name, &config, timeout_duration)
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

            let (analysis, _scores) = self
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

            tracing::debug!(
                task_type = analysis.features.task_type.as_str(),
                language = analysis.features.language.as_str(),
                confidence = analysis.features.confidence,
                "Analyzer result for auto-routing"
            );

            if let Some(scenario_name) = match_scenario(
                &analysis,
                &scenario_data,
                routing_config.confidence_threshold,
            ) {
                return self
                    .route_via_scenario(request, &scenario_name, &config, timeout_duration)
                    .await;
            }
        }

        // Last resort: first available provider
        if let Some(provider) = self.registry.first() {
            return timeout(timeout_duration, provider.send_request(request))
                .await
                .map_err(|_| {
                    crate::error::YoloRouterError::RequestError("Request timeout".to_string())
                })?;
        }

        Err(crate::error::YoloRouterError::RoutingError(
            "No provider available for request".to_string(),
        ))
    }

    async fn route_via_scenario(
        &self,
        request: &ChatRequest,
        scenario_name: &str,
        config: &Config,
        timeout_duration: Duration,
    ) -> Result<ChatResponse> {
        let routing_config = config.routing();
        if let Ok(scenario_config) = config.get_scenario(scenario_name) {
            if routing_config.fallback_enabled {
                let fallback = FallbackChain::new(scenario_config);
                return timeout(
                    timeout_duration,
                    fallback.execute(request, &self.registry, routing_config.retry_count),
                )
                .await
                .map_err(|_| {
                    crate::error::YoloRouterError::RequestError("Request timeout".to_string())
                })?;
            } else if let Some(model_config) = scenario_config.models.first() {
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
}
