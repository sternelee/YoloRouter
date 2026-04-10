use actix_web::{web, HttpServer, HttpResponse, middleware, App};
use crate::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod handlers;

pub struct AppState {
    pub config: Arc<RwLock<crate::Config>>,
}

pub async fn start_server(port: u16, config: crate::Config) -> Result<()> {
    let app_state = web::Data::new(AppState {
        config: Arc::new(RwLock::new(config)),
    });

    tracing::info!("Starting YoloRouter HTTP server on 127.0.0.1:{}", port);

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .wrap(middleware::Logger::default())
            .route("/health", web::get().to(health_check))
            .route("/config", web::get().to(get_config))
            .route("/v1/anthropic", web::post().to(proxy_request))
            .route("/v1/openai", web::post().to(proxy_request))
            .route("/v1/gemini", web::post().to(proxy_request))
            .route("/v1/codex", web::post().to(proxy_request))
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

async fn proxy_request(
    _state: web::Data<AppState>,
    _req: web::Json<serde_json::Value>,
) -> Result<HttpResponse> {
    tracing::debug!("Proxy request received");
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Proxy endpoint (implementation in progress)"
    })))
}

async fn auto_route(
    _state: web::Data<AppState>,
    _req: web::Json<serde_json::Value>,
) -> Result<HttpResponse> {
    tracing::debug!("Auto route request received");
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Auto-routing endpoint (implementation in progress)"
    })))
}
