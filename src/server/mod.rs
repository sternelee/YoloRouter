use crate::models::{
    AnthropicError, AnthropicErrorDetail, AnthropicRequest, AnthropicResponse, ChatRequest,
};
use crate::router::{Router, RoutingEngine};
use crate::utils::StatsCollector;
use crate::Result;
use actix_web::{http::header, middleware, web, App, HttpResponse, HttpServer};
use futures_util::StreamExt;
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
        // Configure JSON extractor with larger payload limit (10MB for large conversations)
        let json_config = web::JsonConfig::default()
            .limit(10 * 1024 * 1024) // 10MB
            .error_handler(|err, _req| {
                let err_msg = err.to_string();
                tracing::error!("JSON parse error: {}", err_msg);
                actix_web::error::InternalError::from_response(
                    err,
                    HttpResponse::BadRequest().json(serde_json::json!({
                        "error": {
                            "message": format!("Invalid JSON: {}", err_msg),
                            "type": "invalid_request_error"
                        }
                    })),
                )
                .into()
            });

        App::new()
            .app_data(app_state.clone())
            .app_data(json_config)
            .wrap(middleware::Logger::default())
            .route("/health", web::get().to(health_check))
            .route("/config", web::get().to(get_config))
            .route("/stats", web::get().to(get_stats))
            .route("/control/status", web::get().to(control_status))
            .route("/control/override", web::post().to(control_set_override))
            .route(
                "/control/override/{endpoint}",
                web::delete().to(control_clear_override),
            )
            .route("/control/reload", web::post().to(control_reload))
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
    endpoint: String,
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

fn anthropic_error_response(
    status: actix_web::http::StatusCode,
    kind: &str,
    message: String,
) -> HttpResponse {
    HttpResponse::build(status).json(AnthropicError {
        error_type: "error".to_string(),
        error: AnthropicErrorDetail {
            error_kind: kind.to_string(),
            message,
        },
    })
}

fn streaming_target_is_supported(model: &str) -> bool {
    if model.is_empty() {
        return false;
    }

    // "auto" is supported - we'll resolve it before streaming
    if model == "auto" {
        return true;
    }

    // Reject known placeholder/invalid values
    let invalid_values = ["auth", "default", "placeholder"];
    if invalid_values.contains(&model) {
        return false;
    }

    // Allow any provider:model format or direct model names
    true
}

fn streaming_target_error_message(model: &str) -> String {
    match model {
        "auth" => "Invalid model name 'auth'. This looks like a typo - did you mean 'auto'? Or set a specific model like 'claude-opus-4' or use 'github_copilot:gpt-5.4'".to_string(),
        "default" | "placeholder" => format!("Invalid model name '{}'. Please specify a valid model like 'claude-opus-4', 'auto', or 'provider:model' format", model),
        _ if model.is_empty() => "Model name cannot be empty when stream=true".to_string(),
        _ => format!("Invalid model name '{}'. Please use 'auto', a model name like 'claude-opus-4', or 'provider:model' format like 'github_copilot:gpt-5.4'", model),
    }
}

fn normalize_streaming_model(model: &str) -> String {
    model
        .strip_prefix("anthropic:")
        .unwrap_or(model)
        .to_string()
}

fn streaming_chat_request(mut request: AnthropicRequest) -> ChatRequest {
    let normalized_model = normalize_streaming_model(&request.model);
    request.stream = Some(true);
    let mut chat_req = ChatRequest::from(request);
    chat_req.model = normalized_model;
    chat_req
}

// ─── Generic streaming proxy ─────────────────────────────────────────────────

/// Generic streaming proxy that works with any provider supporting streaming.
/// Used by OpenAI, Gemini, Codex, and auto endpoints.
async fn proxy_generic_stream(
    state: &web::Data<AppState>,
    mut request: ChatRequest,
    endpoint: &str,
) -> HttpResponse {
    tracing::debug!(
        endpoint = endpoint,
        model = %request.model,
        stream = ?request.stream,
        "Received streaming request"
    );

    let model = request.model.clone();

    // Handle "auto" model: use router to select best model, then stream
    if model == "auto" {
        let scenario = resolve_scenario(&state.overrides, endpoint).await;

        // Use router to determine the best model
        match state
            .router
            .select_best_model(&request, scenario.as_deref())
            .await
        {
            Ok((provider_name, selected_model)) => {
                tracing::info!(
                    provider = provider_name,
                    model = selected_model,
                    endpoint = endpoint,
                    scenario = ?scenario,
                    "Auto-selected model for streaming request"
                );

                // Update request with selected model (use provider:model format)
                request.model = if provider_name == endpoint {
                    selected_model
                } else {
                    format!("{}:{}", provider_name, selected_model)
                };

                // Recursively call with the concrete model
                return Box::pin(proxy_generic_stream(state, request, endpoint)).await;
            }
            Err(e) => {
                return HttpResponse::ServiceUnavailable().json(serde_json::json!({
                    "error": {
                        "message": format!("Failed to auto-select model for streaming: {}", e),
                        "type": "api_error"
                    }
                }));
            }
        }
    }

    // Parse provider:model format
    let (provider_name, model_name) = if model.contains(':') {
        let parts: Vec<&str> = model.split(':').collect();
        (parts[0].to_string(), parts[1].to_string())
    } else {
        // Use endpoint name as provider
        (endpoint.to_string(), model.clone())
    };

    let provider = match state.router.provider(&provider_name).await {
        Some(provider) => provider,
        None => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": {
                    "message": format!("Provider '{}' not found", provider_name),
                    "type": "invalid_request_error"
                }
            }));
        }
    };

    // Check if provider supports streaming
    if !provider.supports_streaming() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": {
                "message": format!("Provider '{}' does not support streaming", provider_name),
                "type": "invalid_request_error"
            }
        }));
    }

    // Update request model to just the model name (without provider prefix)
    request.model = model_name.clone();

    let start = std::time::Instant::now();

    // Start streaming request
    match provider.start_streaming_request(&request).await {
        Ok(response) => {
            let elapsed = start.elapsed().as_millis() as u64;
            state
                .stats
                .record_request(endpoint.to_string(), model_name, true, elapsed)
                .await;

            let byte_stream = response
                .bytes_stream()
                .map(|chunk| chunk.map_err(actix_web::error::ErrorBadGateway));

            HttpResponse::Ok()
                .insert_header((header::CONTENT_TYPE, "text/event-stream"))
                .insert_header((header::CACHE_CONTROL, "no-cache"))
                .insert_header(("Connection", "keep-alive"))
                .streaming(byte_stream)
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            state
                .stats
                .record_request(endpoint.to_string(), model_name, false, elapsed)
                .await;

            HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": {
                    "message": e.to_string(),
                    "type": "api_error"
                }
            }))
        }
    }
}

async fn proxy_anthropic_stream(
    state: &web::Data<AppState>,
    request: AnthropicRequest,
) -> HttpResponse {
    let model = request.model.clone();

    // Handle provider:model format - forward to generic streaming proxy
    if model.contains(':') {
        let chat_req = streaming_chat_request(request);
        return proxy_generic_stream(state, chat_req, "anthropic").await;
    }

    // Handle "auto" model: use router to select best model, then stream
    if model == "auto" {
        let chat_req = ChatRequest::from(request.clone());
        let scenario = resolve_scenario(&state.overrides, "anthropic").await;

        // Use router to determine the best model
        match state
            .router
            .select_best_model(&chat_req, scenario.as_deref())
            .await
        {
            Ok((provider_name, selected_model)) => {
                tracing::info!(
                    provider = provider_name,
                    model = selected_model,
                    scenario = ?scenario,
                    "Auto-selected model for Anthropic streaming request"
                );

                // If auto-selected a non-Anthropic provider, use generic streaming
                if provider_name != "anthropic" {
                    let mut new_req = chat_req;
                    new_req.model = format!("{}:{}", provider_name, selected_model);
                    return proxy_generic_stream(state, new_req, "anthropic").await;
                }

                // Create a new request with the selected model (Anthropic)
                let mut streaming_request = request;
                streaming_request.model = selected_model;

                // Recursively call with the concrete model
                return Box::pin(proxy_anthropic_stream(state, streaming_request)).await;
            }
            Err(e) => {
                return anthropic_error_response(
                    actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
                    "api_error",
                    format!("Failed to auto-select model for streaming: {}", e),
                );
            }
        }
    }

    // Validate explicit model name
    if !streaming_target_is_supported(&model) {
        return anthropic_error_response(
            actix_web::http::StatusCode::BAD_REQUEST,
            "invalid_request_error",
            streaming_target_error_message(&model),
        );
    }

    // Direct models route to anthropic provider
    let provider = match state.router.provider("anthropic").await {
        Some(provider) => provider,
        None => {
            return anthropic_error_response(
                actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
                "api_error",
                "Provider 'anthropic' not found".to_string(),
            )
        }
    };

    let chat_req = streaming_chat_request(request);
    let final_model = chat_req.model.clone();
    let start = std::time::Instant::now();

    match provider.start_streaming_request(&chat_req).await {
        Ok(response) => {
            let elapsed = start.elapsed().as_millis() as u64;
            state
                .stats
                .record_request("anthropic".to_string(), final_model, true, elapsed)
                .await;

            let byte_stream = response
                .bytes_stream()
                .map(|chunk| chunk.map_err(actix_web::error::ErrorBadGateway));

            HttpResponse::Ok()
                .insert_header((header::CONTENT_TYPE, "text/event-stream"))
                .insert_header((header::CACHE_CONTROL, "no-cache"))
                .insert_header(("Connection", "keep-alive"))
                .streaming(byte_stream)
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            state
                .stats
                .record_request("anthropic".to_string(), final_model, false, elapsed)
                .await;
            anthropic_error_response(
                actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
                "api_error",
                e.to_string(),
            )
        }
    }
}

async fn proxy_anthropic_non_stream(
    state: &web::Data<AppState>,
    request: AnthropicRequest,
) -> HttpResponse {
    let chat_req = ChatRequest::from(request);
    let scenario = resolve_scenario(&state.overrides, "anthropic").await;
    let start = std::time::Instant::now();
    let model = chat_req.model.clone();

    match state.router.route(&chat_req, scenario.as_deref()).await {
        Ok(resp) => {
            let elapsed = start.elapsed().as_millis() as u64;
            state
                .stats
                .record_request("anthropic".to_string(), model, true, elapsed)
                .await;
            HttpResponse::Ok().json(AnthropicResponse::from(resp))
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            state
                .stats
                .record_request("anthropic".to_string(), model, false, elapsed)
                .await;
            tracing::error!("Anthropic proxy error: {}", e);
            anthropic_error_response(
                actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
                "api_error",
                e.to_string(),
            )
        }
    }
}

/// `/v1/anthropic` — accepts Anthropic Messages API format, returns Anthropic format.
/// Used by Claude Code and Anthropic SDKs.
async fn anthropic_proxy(
    state: web::Data<AppState>,
    body: web::Json<serde_json::Value>,
) -> HttpResponse {
    let request: AnthropicRequest = match serde_json::from_value(body.into_inner()) {
        Ok(request) => request,
        Err(e) => {
            return anthropic_error_response(
                actix_web::http::StatusCode::BAD_REQUEST,
                "invalid_request_error",
                e.to_string(),
            )
        }
    };

    if request.stream == Some(true) {
        proxy_anthropic_stream(&state, request).await
    } else {
        proxy_anthropic_non_stream(&state, request).await
    }
}

/// `/v1/openai` — accepts OpenAI Chat Completions format, returns OpenAI format.
/// Used by OpenAI SDKs and most CLI tools.
async fn openai_proxy(
    state: web::Data<AppState>,
    req: web::Json<ChatRequest>,
) -> Result<HttpResponse> {
    let request = req.into_inner();

    if request.stream.unwrap_or(false) {
        Ok(proxy_generic_stream(&state, request, "openai").await)
    } else {
        route_endpoint(state, request, "openai").await
    }
}

/// `/v1/gemini` — accepts OpenAI-compatible format (Gemini supports this).
async fn gemini_proxy(
    state: web::Data<AppState>,
    req: web::Json<ChatRequest>,
) -> Result<HttpResponse> {
    let request = req.into_inner();

    if request.stream.unwrap_or(false) {
        Ok(proxy_generic_stream(&state, request, "gemini").await)
    } else {
        route_endpoint(state, request, "gemini").await
    }
}

/// `/v1/codex` — accepts OpenAI format; used by Codex CLI.
async fn codex_proxy(
    state: web::Data<AppState>,
    req: web::Json<ChatRequest>,
) -> Result<HttpResponse> {
    let request = req.into_inner();

    if request.stream.unwrap_or(false) {
        Ok(proxy_generic_stream(&state, request, "codex").await)
    } else {
        route_endpoint(state, request, "codex").await
    }
}

/// `/v1/auto` — accepts OpenAI format and lets the analyzer choose the best
/// scenario automatically.
async fn auto_route(
    state: web::Data<AppState>,
    req: web::Json<ChatRequest>,
) -> Result<HttpResponse> {
    let request = req.into_inner();

    if request.stream.unwrap_or(false) {
        Ok(proxy_generic_stream(&state, request, "auto").await)
    } else {
        route_endpoint(state, request, "auto").await
    }
}

/// Shared routing + stats helper used by every OpenAI-format endpoint.
async fn route_endpoint(
    state: web::Data<AppState>,
    req: ChatRequest,
    endpoint: &str,
) -> Result<HttpResponse> {
    let scenario = if endpoint == "auto" {
        None
    } else {
        resolve_scenario(&state.overrides, endpoint).await
    };

    let start = std::time::Instant::now();
    let model = req.model.clone();

    match state.router.route(&req, scenario.as_deref()).await {
        Ok(response) => {
            let elapsed = start.elapsed().as_millis() as u64;
            state
                .stats
                .record_request(endpoint.to_string(), model, true, elapsed)
                .await;
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            state
                .stats
                .record_request(endpoint.to_string(), model, false, elapsed)
                .await;
            tracing::error!("{} proxy error: {}", endpoint, e);
            Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": { "message": e.to_string(), "type": "api_error" }
            })))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AnthropicContent, ChatResponse, Choice, Usage};
    use crate::Config;
    use actix_web::{body, http::StatusCode};
    use serde_json::json;

    fn test_state(config: Config) -> web::Data<AppState> {
        let router = Arc::new(Router::new(
            RoutingEngine::new_with_config(config.clone()).unwrap(),
        ));
        web::Data::new(AppState {
            config: Arc::new(RwLock::new(config)),
            router,
            stats: Arc::new(StatsCollector::new()),
            overrides: Arc::new(RwLock::new(HashMap::new())),
            config_path: "test.toml".to_string(),
        })
    }

    #[actix_web::test]
    async fn anthropic_proxy_supports_auto_streaming_model() {
        let config = Config::from_string(
            r#"
[providers.anthropic]
type = "anthropic"
api_key = "test-key"
base_url = "http://127.0.0.1:9"

[scenarios.default]
models = [
    { provider = "anthropic", model = "claude-sonnet-4" }
]
is_default = true
"#,
        )
        .expect("config should parse");
        let state = test_state(config);

        let response = anthropic_proxy(
            state,
            web::Json(json!({
                "model": "auto",
                "messages": [{"role": "user", "content": "hello"}],
                "stream": true
            })),
        )
        .await;

        // Should attempt to connect (and fail with SERVICE_UNAVAILABLE since we don't have a real server)
        // but not reject the "auto" model name
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[actix_web::test]
    async fn anthropic_proxy_rejects_auth_streaming_model() {
        let config = Config::from_string(
            r#"
[providers.anthropic]
type = "anthropic"
api_key = "test-key"
base_url = "http://127.0.0.1:9"
"#,
        )
        .expect("config should parse");
        let state = test_state(config);

        let response = anthropic_proxy(
            state,
            web::Json(json!({
                "model": "auth",
                "messages": [{"role": "user", "content": "hello"}],
                "stream": true
            })),
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = body::to_bytes(response.into_body()).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["type"], json!("invalid_request_error"));
        assert!(json["error"]["message"].as_str().unwrap().contains("auth"));
        assert!(json["error"]["message"].as_str().unwrap().contains("typo"));
    }

    #[actix_web::test]
    async fn anthropic_proxy_attempts_direct_streaming_requests() {
        let config = Config::from_string(
            r#"
[providers.anthropic]
type = "anthropic"
api_key = "test-key"
base_url = "http://127.0.0.1:9"
"#,
        )
        .expect("config should parse");
        let state = test_state(config);

        let response = anthropic_proxy(
            state,
            web::Json(json!({
                "model": "anthropic:claude-sonnet-4-5",
                "messages": [{"role": "user", "content": "hello"}],
                "stream": true,
                "betas": ["fine-grained-tool-streaming-2025-05-14"]
            })),
        )
        .await;

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = body::to_bytes(response.into_body()).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["type"], json!("api_error"));
    }

    #[test]
    fn anthropic_response_uses_structured_content_blocks() {
        let response = ChatResponse {
            id: "msg_123".to_string(),
            model: "claude-sonnet-4-5".to_string(),
            choices: vec![Choice {
                index: 0,
                message: crate::models::ChatMessage {
                    role: "assistant".to_string(),
                    content: "hello".to_string(),
                },
                finish_reason: "stop".to_string(),
            }],
            usage: Usage {
                prompt_tokens: 1,
                completion_tokens: 2,
                total_tokens: 3,
            },
            anthropic_content: Some(vec![crate::models::AnthropicContentBlock {
                block_type: "tool_use".to_string(),
                text: None,
                id: Some("toolu_1".to_string()),
                name: Some("Read".to_string()),
                input: Some(json!({"file_path": "/tmp/x"})),
                tool_use_id: None,
                content: None,
                extra: serde_json::Map::new(),
            }]),
            anthropic_stop_sequence: None,
        };

        let anthropic = AnthropicResponse::from(response);
        assert_eq!(anthropic.content[0].block_type, "tool_use");
        assert_eq!(anthropic.content[0].name.as_deref(), Some("Read"));
    }

    #[test]
    fn anthropic_request_round_trip_keeps_tools() {
        let request: AnthropicRequest = serde_json::from_value(json!({
            "model": "claude-sonnet-4-5",
            "messages": [{
                "role": "user",
                "content": [{"type": "text", "text": "hello"}]
            }],
            "tools": [{"name": "Read", "input_schema": {"type": "object"}}],
            "tool_choice": {"type": "auto"}
        }))
        .unwrap();

        let chat = ChatRequest::from(request.clone());
        assert!(chat.requires_tools());
        let native = chat
            .anthropic
            .expect("native anthropic payload should be preserved");
        assert_eq!(native.tools, request.tools);
        assert_eq!(native.tool_choice, request.tool_choice);
        assert!(matches!(
            native.messages[0].content,
            AnthropicContent::Blocks(_)
        ));
        assert_eq!(request.messages[0].role, "user");
    }
}
