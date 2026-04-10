use crate::models::{ChatRequest, ChatResponse};
use crate::provider::{Provider, ProviderFactory};
use crate::config::Config;
use crate::Result;
use std::sync::Arc;
use std::collections::HashMap;

pub mod engine;
pub mod fallback;
pub use engine::RoutingEngine;
pub use fallback::FallbackChain;

pub struct Router {
    engine: RoutingEngine,
}

impl Router {
    pub fn new(engine: RoutingEngine) -> Self {
        Self { engine }
    }

    pub async fn route(&self, request: &ChatRequest, scenario: Option<&str>) -> Result<ChatResponse> {
        self.engine.route(request, scenario).await
    }
}

pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn Provider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    pub fn from_config(config: &Config) -> Result<Self> {
        let mut registry = Self::new();
        
        for (name, provider_config) in config.providers() {
            let provider = ProviderFactory::create_provider(&name, &provider_config)?;
            registry.providers.insert(name, provider);
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
