use yolo_router::{Config, server, utils};
use std::env;

#[actix_web::main]
async fn main() -> yolo_router::Result<()> {
    utils::init_logger();
    
    let config_path = env::var("YOLO_CONFIG").unwrap_or_else(|_| "config.toml".to_string());
    
    let config = match Config::from_file(&config_path) {
        Ok(cfg) => {
            if let Err(e) = cfg.validate() {
                eprintln!("Configuration validation error: {}", e);
                return Err(e);
            }
            cfg
        }
        Err(e) => {
            eprintln!("Failed to load config from {}: {}", config_path, e);
            eprintln!("Using default configuration");
            Config {
                daemon: None,
                providers: None,
                scenarios: None,
                routing: None,
            }
        }
    };

    let daemon_config = config.daemon();
    
    tracing::info!("Starting YoloRouter daemon");
    tracing::info!("Config file: {}", config_path);
    tracing::info!("Listening on 127.0.0.1:{}", daemon_config.port);
    tracing::info!("Log level: {}", daemon_config.log_level);
    tracing::info!("Providers configured: {:?}", config.providers().keys().collect::<Vec<_>>());
    tracing::info!("Scenarios configured: {:?}", config.scenarios().keys().collect::<Vec<_>>());

    server::start_server(daemon_config.port, config).await?;

    Ok(())
}
