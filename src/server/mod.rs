use crate::models::{
    AnthropicError, AnthropicErrorDetail, AnthropicRequest, AnthropicResponse, ChatRequest,
};
use crate::router::{Router, RoutingEngine};
use crate::utils::StatsCollector;
use crate::Result;
use actix_web::{middleware, web, App, HttpResponse, HttpServer};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod handlers;

// ─── Scenario overrides ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "mode", content = "scenario")]
pub enum ScenarioOverride {
    Auto,
    Pinned(String),
}

/// Shared map: endpoint key → override.
/// Keys: "global" | "anthropic" | "openai" | "codex" | "gemini"
pub type OverrideMap = Arc<RwLock<HashMap<String, ScenarioOverride>>>;

async fn resolve_scenario(overrides: &OverrideMap, endpoint: &str) -> Option<String> {
    let map = overrides.read().await;
    // endpoint-specific wins over global
    for key in &[endpoint, "global"] {
        if let Some(ov) = map.get(*key) {
            return match ov {
                ScenarioOverride::Pinned(s) => Some(s.clone()),
                ScenarioOverride::Auto => None,
            };
        }
    }
    None
}

// ─── App state ────────────────────────────────────────────────────────────────

pub struct AppState {
    pub config: Arc<RwLock<crate::Config>>,
    pub router: Arc<Router>,
    pub stats: Arc<StatsCollector>,
    pub overrides: OverrideMap,
    pub config_path: String,
}

pub async fn start_server(port: u16, config: crate::Config, config_path: String) -> Result<()> {
    let routing_engine = RoutingEngine::new_with_config(config.clone())?;
    let router = Arc::new(Router::new(routing_engine));
    let stats = Arc::new(StatsCollector::new());
    let overrides: OverrideMap = Arc::new(RwLock::new(HashMap::new()));

    let app_state = web::Data::new(AppState {
        config: Arc::new(RwLock::new(config)),
        router,
        stats,
        overrides,
        config_path: config_path.clone(),
    });

    tracing::info!("Starting YoloRouter HTTP server on 127.0.0.1:{}", port);

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .wrap(middleware::Logger::default())
            // Health / introspection
            .route("/health", web::get().to(health_check))
            .route("/config", web::get().to(get_config))
            .route("/stats", web::get().to(get_stats))
            // Control API — TUI connects here
            .route("/control/status", web::get().to(control_status))
            .route("/control/override", web::post().to(control_set_override))
            .route(
                "/control/override/{endpoint}",
                web::delete().to(control_clear_override),
            )
            .route("/control/reload", web::post().to(control_reload))
            // Protocol adapters
            .route("/v1/anthropic", web::post().to(anthropic_proxy))
            .route("/v1/anthropic/v1/messages", web::post().to(anthropic_proxy))
            .route("/v1/openai", web::post().to(openai_proxy))
            .route("/v1/openai/chat/completions", web::post().to(openai_proxy))
            .route("/v1/gemini", web::post().to(gemini_proxy))
            .route("/v1/gemini/chat/completions", web::post().to(gemini_proxy))
            .route("/v1/codex", web::post().to(codex_proxy))
            .route("/v1/codex/chat/completions", web::post().to(codex_proxy))
            .route("/v1/auto", web::post().to(auto_route))
    })
    .bind(format!("127.0.0.1:{}", port))?
    .run()
    .await?;

    Ok(())
}

// ─── Health / config / stats ──────────────────────────────────────────────────

async fn health_check(state: web::Data<AppState>) -> Result<HttpResponse> {
    let config = state.config.read().await;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "service": "yolo-router",
        "version": "0.1.0",
        "providers": config.providers().keys().collect::<Vec<_>>(),
        "scenarios": config.scenarios().keys().collect::<Vec<_>>(),
    })))
}

async fn get_config(state: web::Data<AppState>) -> Result<HttpResponse> {
    let config = state.config.read().await;
    match config.to_string() {
        Ok(content) => Ok(HttpResponse::Ok().content_type("text/plain").body(content)),
        Err(_) => Ok(HttpResponse::InternalServerError().finish()),
    }
}

async fn get_stats(state: web::Data<AppState>) -> Result<HttpResponse> {
    let stats = state.stats.get_stats().await;
    Ok(HttpResponse::Ok().json(stats))
}

// ─── Control API ─────────────────────────────────────────────────────────────

async fn control_status(state: web::Data<AppState>) -> Result<HttpResponse> {
    let config = state.config.read().await;
    let overrides = state.overrides.read().await;

    let active: HashMap<String, String> = overrides
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                match v {
                    ScenarioOverride::Auto => "auto".to_string(),
                    ScenarioOverride::Pinned(s) => s.clone(),
                },
            )
        })
        .collect();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "providers": config.providers().keys().collect::<Vec<_>>(),
        "scenarios": config.scenarios().keys().collect::<Vec<_>>(),
        "overrides": active,
    })))
}

#[derive(Debug, Deserialize)]
struct OverrideRequest {
    /// "global" | "anthropic" | "openai" | "codex" | "gemini"
    endpoint: String,
    /// Scenario name, or null/"auto" to clear
    scenario: Option<String>,
}

async fn control_set_override(
    state: web::Data<AppState>,
    body: web::Json<OverrideRequest>,
) -> Result<HttpResponse> {
    let ov = match &body.scenario {
        None => ScenarioOverride::Auto,
        Some(s) if s == "auto" => ScenarioOverride::Auto,
        Some(s) => {
            // Validate scenario exists
            let config = state.config.read().await;
            if config.get_scenario(s).is_err() {
                return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "error": format!("Scenario '{}' not found", s)
                })));
            }
            ScenarioOverride::Pinned(s.clone())
        }
    };

    let label = match &ov {
        ScenarioOverride::Auto => "auto".to_string(),
        ScenarioOverride::Pinned(s) => s.clone(),
    };

    state
        .overrides
        .write()
        .await
        .insert(body.endpoint.clone(), ov);
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "endpoint": &body.endpoint,
        "scenario": label,
    })))
}

async fn control_clear_override(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    state.overrides.write().await.remove(path.as_str());
    Ok(HttpResponse::Ok().json(serde_json::json!({"cleared": path.as_str()})))
}

async fn control_reload(state: web::Data<AppState>) -> actix_web::Result<HttpResponse> {
    match crate::Config::from_file(&state.config_path) {
        Ok(new_config) => {
            if let Err(e) = new_config.validate() {
                tracing::error!("Config reload validation failed: {}", e);
                return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "error": e.to_string(),
                })));
            }

            if let Err(e) = state.router.reload(&new_config).await {
                tracing::error!("Config reload failed while rebuilding router: {}", e);
                return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": e.to_string(),
                })));
            }

            {
                let mut overrides = state.overrides.write().await;
                overrides.retain(|_, ov| match ov {
                    ScenarioOverride::Auto => true,
                    ScenarioOverride::Pinned(name) => new_config.get_scenario(name).is_ok(),
                });
            }

            *state.config.write().await = new_config;
            tracing::info!("Config hot-reloaded from {}", state.config_path);
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "status": "reloaded",
                "config_path": state.config_path,
            })))
        }
        Err(e) => {
            tracing::error!("Config reload failed: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e.to_string(),
            })))
        }
    }
}

// ─── Protocol adapters ────────────────────────────────────────────────────────

/// `/v1/anthropic` — accepts Anthropic Messages API format, returns Anthropic format.
/// Used by Claude Code and Anthropic SDKs.
async fn anthropic_proxy(
    state: web::Data<AppState>,
    body: web::Json<serde_json::Value>,
) -> HttpResponse {
    let anthro_req: AnthropicRequest = match serde_json::from_value(body.into_inner()) {
        Ok(r) => r,
        Err(e) => {
            return HttpResponse::BadRequest().json(AnthropicError {
                error_type: "error".to_string(),
                error: AnthropicErrorDetail {
                    error_kind: "invalid_request_error".to_string(),
                    message: e.to_string(),
                },
            })
        }
    };

    let chat_req = ChatRequest::from(anthro_req);
    let scenario = resolve_scenario(&state.overrides, "anthropic").await;
    let start = std::time::Instant::now();

    match state.router.route(&chat_req, scenario.as_deref()).await {
        Ok(resp) => {
            let elapsed = start.elapsed().as_millis() as u64;
            state
                .stats
                .record_request("anthropic".to_string(), chat_req.model, true, elapsed)
                .await;
            HttpResponse::Ok().json(AnthropicResponse::from(resp))
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            state
                .stats
                .record_request("anthropic".to_string(), chat_req.model, false, elapsed)
                .await;
            tracing::error!("Anthropic proxy error: {}", e);
            HttpResponse::ServiceUnavailable().json(AnthropicError {
                error_type: "error".to_string(),
                error: AnthropicErrorDetail {
                    error_kind: "api_error".to_string(),
                    message: e.to_string(),
                },
            })
        }
    }
}

/// `/v1/openai` — accepts OpenAI Chat Completions format, returns OpenAI format.
/// Used by OpenAI SDKs and most CLI tools.
async fn openai_proxy(
    state: web::Data<AppState>,
    req: web::Json<ChatRequest>,
) -> Result<HttpResponse> {
    route_endpoint(state, req.into_inner(), "openai").await
}

/// `/v1/gemini` — accepts OpenAI-compatible format (Gemini supports this).
async fn gemini_proxy(
    state: web::Data<AppState>,
    req: web::Json<ChatRequest>,
) -> Result<HttpResponse> {
    route_endpoint(state, req.into_inner(), "gemini").await
}

/// `/v1/codex` — accepts OpenAI format; used by Codex CLI.
async fn codex_proxy(
    state: web::Data<AppState>,
    req: web::Json<ChatRequest>,
) -> Result<HttpResponse> {
    route_endpoint(state, req.into_inner(), "codex").await
}

async fn route_endpoint(
    state: web::Data<AppState>,
    req: ChatRequest,
    endpoint: &str,
) -> Result<HttpResponse> {
    let scenario = resolve_scenario(&state.overrides, endpoint).await;
    let start = std::time::Instant::now();

    match state.router.route(&req, scenario.as_deref()).await {
        Ok(response) => {
            let elapsed = start.elapsed().as_millis() as u64;
            state
                .stats
                .record_request(endpoint.to_string(), req.model, true, elapsed)
                .await;
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            state
                .stats
                .record_request(endpoint.to_string(), req.model, false, elapsed)
                .await;
            tracing::error!("{} proxy error: {}", endpoint, e);
            Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": { "message": e.to_string(), "type": "api_error" }
            })))
        }
    }
}

/// `/v1/auto` — 15-dim analyzer picks scenario automatically.
async fn auto_route(
    state: web::Data<AppState>,
    req: web::Json<ChatRequest>,
) -> Result<HttpResponse> {
    let start = std::time::Instant::now();

    // Always let the analyzer decide — no hardcoded scenario extraction
    match state.router.route(&req, None).await {
        Ok(response) => {
            let elapsed = start.elapsed().as_millis() as u64;
            state
                .stats
                .record_request("auto".to_string(), req.model.clone(), true, elapsed)
                .await;
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            state
                .stats
                .record_request("auto".to_string(), req.model.clone(), false, elapsed)
                .await;
            tracing::error!("Auto route error: {}", e);
            Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": { "message": e.to_string(), "type": "api_error" }
            })))
        }
    }
}
