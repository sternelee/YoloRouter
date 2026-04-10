use yolo_router::{Config, server, utils, tui::TuiManager};
use std::env;

#[actix_web::main]
async fn main() -> yolo_router::Result<()> {
    utils::init_logger();

    let args: Vec<String> = env::args().collect();
    let tui_mode = args.contains(&"--tui".to_string());

    // --config <path> flag (takes priority over YOLO_CONFIG env var)
    let config_path = args
        .windows(2)
        .find(|w| w[0] == "--config")
        .map(|w| w[1].clone())
        .unwrap_or_else(|| env::var("YOLO_CONFIG").unwrap_or_else(|_| "config.toml".to_string()));

    let config = match Config::from_file(&config_path) {
        Ok(cfg) => {
            if let Err(e) = cfg.validate() {
                eprintln!("Configuration validation error: {}", e);
                return Err(e);
            }
            cfg
        }
        Err(e) => {
            if !tui_mode {
                eprintln!("Failed to load config from {}: {}", config_path, e);
                eprintln!("Using default configuration");
            }
            Config {
                daemon: None,
                providers: None,
                scenarios: None,
                routing: None,
            }
        }
    };

    if tui_mode {
        let manager = TuiManager::new();
        manager.run(config, config_path).await;
        return Ok(());
    }

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

