use crate::models::{ChatRequest, ChatResponse};
use crate::config::Config;
use crate::Result;
use super::{ProviderRegistry, FallbackChain};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::timeout;

pub struct RoutingEngine {
    config: Arc<RwLock<Config>>,
    registry: Arc<ProviderRegistry>,
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
        }
    }

    pub fn new_with_config(config: Config) -> Result<Self> {
        let registry = ProviderRegistry::from_config(&config)?;
        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            registry: Arc::new(registry),
        })
    }

    pub async fn route(&self, request: &ChatRequest, scenario: Option<&str>) -> Result<ChatResponse> {
        let config = self.config.read().await;
        let routing_config = config.routing();
        let timeout_duration = Duration::from_millis(routing_config.timeout_ms);

        // If scenario is specified, use scenario-based routing with fallback
        if let Some(scenario_name) = scenario {
            if let Ok(scenario_config) = config.get_scenario(scenario_name) {
                if routing_config.fallback_enabled {
                    let fallback = FallbackChain::new(scenario_config);
                    return timeout(
                        timeout_duration,
                        fallback.execute(request, &self.registry, routing_config.retry_count),
                    )
                    .await
                    .map_err(|_| crate::error::YoloRouterError::RequestError(
                        "Request timeout".to_string(),
                    ))?;
                } else {
                    // No fallback, use first model only
                    if let Some(model_config) = scenario_config.models.first() {
                        if let Some(provider) = self.registry.get(&model_config.provider) {
                            let mut req = request.clone();
                            req.model = model_config.model.clone();
                            return timeout(
                                timeout_duration,
                                provider.send_request(&req),
                            )
                            .await
                            .map_err(|_| crate::error::YoloRouterError::RequestError(
                                "Request timeout".to_string(),
                            ))?;
                        }
                    }
                }
            }
        }

        // Direct routing: try to find a provider that matches the model name
        let model_parts: Vec<&str> = request.model.split(':').collect();
        if model_parts.len() == 2 {
            let provider_name = model_parts[0];
            if let Some(provider) = self.registry.get(provider_name) {
                return timeout(
                    timeout_duration,
                    provider.send_request(request),
                )
                .await
                .map_err(|_| crate::error::YoloRouterError::RequestError(
                    "Request timeout".to_string(),
                ))?;
            }
        }

        // Fallback to first available provider
        if let Some(provider) = self.registry.get("anthropic") {
            return timeout(
                timeout_duration,
                provider.send_request(request),
            )
            .await
            .map_err(|_| crate::error::YoloRouterError::RequestError(
                "Request timeout".to_string(),
            ))?;
        }

        Err(crate::error::YoloRouterError::RoutingError(
            "No provider available for request".to_string(),
        ))
    }

    pub async fn get_config(&self) -> Config {
        self.config.read().await.clone()
    }

    pub fn registry(&self) -> &ProviderRegistry {
        &self.registry
    }
}
