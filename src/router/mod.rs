use crate::models::{ChatRequest, ChatResponse};
use crate::Result;

pub mod engine;
pub use engine::RoutingEngine;

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
