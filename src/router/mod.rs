use crate::config::Config;
use crate::models::{ChatRequest, ChatResponse};
use crate::provider::{Provider, ProviderFactory};
use crate::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod engine;
pub mod fallback;
pub mod health;
pub use engine::RoutingEngine;
pub use fallback::FallbackChain;
pub use health::ProviderHealthTracker;

pub struct Router {
    engine: RwLock<RoutingEngine>,
}

impl Router {
    pub fn new(engine: RoutingEngine) -> Self {
        Self {
            engine: RwLock::new(engine),
        }
    }

    pub async fn route(
        &self,
        request: &ChatRequest,
        scenario: Option<&str>,
    ) -> Result<ChatResponse> {
        let engine = self.engine.read().await;
        engine.route(request, scenario).await
    }

    /// Select the best model for a request without executing it.
    /// Returns (provider_name, model_name).
    pub async fn select_best_model(
        &self,
        request: &ChatRequest,
        scenario: Option<&str>,
    ) -> Result<(String, String)> {
        let engine = self.engine.read().await;
        engine.select_best_model(request, scenario).await
    }

    pub async fn reload(&self, config: &Config) -> Result<()> {
        let new_engine = RoutingEngine::new_with_config(config.clone())?;
        *self.engine.write().await = new_engine;
        Ok(())
    }

    pub async fn provider_names(&self) -> Vec<String> {
        let engine = self.engine.read().await;
        engine.registry().list()
    }
}

pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn Provider>>,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    pub fn from_config(config: &Config) -> Result<Self> {
        let mut registry = Self::new();
        for name in config.providers().keys() {
            // get_provider() applies ${ENV_VAR} expansion
            let provider_config = config.get_provider(name)?;
            let provider = ProviderFactory::create_provider(name, &provider_config)?;
            registry.providers.insert(name.to_string(), provider);
        }
        Ok(registry)
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn Provider>> {
        self.providers.get(name)
    }

    pub fn first(&self) -> Option<&Arc<dyn Provider>> {
        self.providers.values().next()
    }

    pub fn list(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_provider(name: &str, provider_type: &str) -> Config {
        Config::from_string(&format!(
            r#"
[providers.{name}]
type = "{provider_type}"
api_key = "test-key"

[scenarios.default]
models = [
    {{ provider = "{name}", model = "test-model" }}
]
"#
        ))
        .unwrap()
    }

    #[tokio::test]
    async fn test_router_reload_updates_provider_registry() {
        let router = Router::new(
            RoutingEngine::new_with_config(config_with_provider("openai", "openai")).unwrap(),
        );

        assert_eq!(router.provider_names().await, vec!["openai".to_string()]);

        router
            .reload(&config_with_provider("anthropic", "anthropic"))
            .await
            .unwrap();

        assert_eq!(router.provider_names().await, vec!["anthropic".to_string()]);
    }
}
