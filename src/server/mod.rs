use actix_web::{web, HttpServer, HttpResponse, middleware, App};
use crate::Result;
use crate::models::ChatRequest;
use crate::router::{Router, RoutingEngine};
use crate::utils::StatsCollector;
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod handlers;

pub struct AppState {
    pub config: Arc<RwLock<crate::Config>>,
    pub router: Arc<Router>,
    pub stats: Arc<StatsCollector>,
}

pub async fn start_server(port: u16, config: crate::Config) -> Result<()> {
    // Create routing engine from config
    let routing_engine = RoutingEngine::new_with_config(config.clone())?;
    let router = Arc::new(Router::new(routing_engine));
    let stats = Arc::new(StatsCollector::new());

    let app_state = web::Data::new(AppState {
        config: Arc::new(RwLock::new(config)),
        router,
        stats,
    });

    tracing::info!("Starting YoloRouter HTTP server on 127.0.0.1:{}", port);

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .wrap(middleware::Logger::default())
            .route("/health", web::get().to(health_check))
            .route("/config", web::get().to(get_config))
            .route("/stats", web::get().to(get_stats))
            .route("/v1/anthropic", web::post().to(anthropic_proxy))
            .route("/v1/openai", web::post().to(openai_proxy))
            .route("/v1/gemini", web::post().to(gemini_proxy))
            .route("/v1/codex", web::post().to(codex_proxy))
            .route("/v1/auto", web::post().to(auto_route))
    })
    .bind(format!("127.0.0.1:{}", port))?
    .run()
    .await?;

    Ok(())
}

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
        Ok(content) => Ok(HttpResponse::Ok()
            .content_type("text/plain")
            .body(content)),
        Err(_) => Ok(HttpResponse::InternalServerError().finish()),
    }
}

async fn get_stats(state: web::Data<AppState>) -> Result<HttpResponse> {
    let stats = state.stats.get_stats().await;
    Ok(HttpResponse::Ok().json(stats))
}

// Provider-specific proxies
async fn anthropic_proxy(
    state: web::Data<AppState>,
    req: web::Json<ChatRequest>,
) -> Result<HttpResponse> {
    let router = &state.router;
    let stats = &state.stats;
    let start = std::time::Instant::now();

    // Use auto-detection or extract scenario from request headers
    // Default to None to let routing engine auto-detect
    match router.route(&req, None).await {
        Ok(response) => {
            let elapsed = start.elapsed().as_millis() as u64;
            stats
                .record_request("anthropic".to_string(), req.model.clone(), true, elapsed)
                .await;
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            stats
                .record_request("anthropic".to_string(), req.model.clone(), false, elapsed)
                .await;
            tracing::error!("Anthropic proxy error: {}", e);
            Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": e.to_string()
            })))
        }
    }
}

async fn openai_proxy(
    state: web::Data<AppState>,
    req: web::Json<ChatRequest>,
) -> Result<HttpResponse> {
    let router = &state.router;
    let stats = &state.stats;
    let start = std::time::Instant::now();

    match router.route(&req, None).await {
        Ok(response) => {
            let elapsed = start.elapsed().as_millis() as u64;
            stats
                .record_request("openai".to_string(), req.model.clone(), true, elapsed)
                .await;
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            stats
                .record_request("openai".to_string(), req.model.clone(), false, elapsed)
                .await;
            tracing::error!("OpenAI proxy error: {}", e);
            Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": e.to_string()
            })))
        }
    }
}

async fn gemini_proxy(
    state: web::Data<AppState>,
    req: web::Json<ChatRequest>,
) -> Result<HttpResponse> {
    let router = &state.router;
    let stats = &state.stats;
    let start = std::time::Instant::now();

    match router.route(&req, None).await {
        Ok(response) => {
            let elapsed = start.elapsed().as_millis() as u64;
            stats
                .record_request("gemini".to_string(), req.model.clone(), true, elapsed)
                .await;
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            stats
                .record_request("gemini".to_string(), req.model.clone(), false, elapsed)
                .await;
            tracing::error!("Gemini proxy error: {}", e);
            Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": e.to_string()
            })))
        }
    }
}

async fn codex_proxy(
    state: web::Data<AppState>,
    req: web::Json<ChatRequest>,
) -> Result<HttpResponse> {
    let router = &state.router;
    let stats = &state.stats;
    let start = std::time::Instant::now();

    match router.route(&req, None).await {
        Ok(response) => {
            let elapsed = start.elapsed().as_millis() as u64;
            stats
                .record_request("codex".to_string(), req.model.clone(), true, elapsed)
                .await;
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            stats
                .record_request("codex".to_string(), req.model.clone(), false, elapsed)
                .await;
            tracing::error!("Codex proxy error: {}", e);
            Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": e.to_string()
            })))
        }
    }
}

async fn auto_route(
    state: web::Data<AppState>,
    req: web::Json<ChatRequest>,
) -> Result<HttpResponse> {
    let router = &state.router;
    let stats = &state.stats;
    let start = std::time::Instant::now();

    // Auto routing: detect scenario from request if possible
    let scenario = extract_scenario(&req);

    match router.route(&req, scenario.as_deref()).await {
        Ok(response) => {
            let elapsed = start.elapsed().as_millis() as u64;
            stats
                .record_request("auto".to_string(), req.model.clone(), true, elapsed)
                .await;
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            stats
                .record_request("auto".to_string(), req.model.clone(), false, elapsed)
                .await;
            tracing::error!("Auto route error: {}", e);
            Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": e.to_string()
            })))
        }
    }
}

fn extract_scenario(request: &ChatRequest) -> Option<String> {
    // Simple heuristic: check if model name contains "coding" or "analysis"
    if request.model.contains("coding") {
        Some("coding".to_string())
    } else if request.model.contains("analysis") {
        Some("analysis".to_string())
    } else {
        None
    }
}
