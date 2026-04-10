use yolo_router::{Config, server, utils, tui::TuiManager};
use yolo_router::tui::github_auth::run_github_device_flow;
use yolo_router::tui::codex_auth::run_codex_device_flow;
use std::env;
use std::path::PathBuf;

#[actix_web::main]
async fn main() -> yolo_router::Result<()> {
    utils::init_logger();

    let args: Vec<String> = env::args().collect();
    let tui_mode = args.contains(&"--tui".to_string());

    // --auth <provider> subcommand
    if let Some(provider) = args.windows(2).find(|w| w[0] == "--auth").map(|w| w[1].as_str()) {
        return run_auth(provider).await;
    }

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

/// Handle `yolo-router --auth <provider>` subcommands.
async fn run_auth(provider: &str) -> yolo_router::Result<()> {
    match provider {
        "github" | "github_copilot" => {
            println!("Starting GitHub Copilot OAuth device flow...");
            match run_github_device_flow(None).await {
                Ok(Some(token)) => {
                    let token_path = github_token_path();
                    if let Some(parent) = token_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = std::fs::write(&token_path, &token);
                    println!("✅ GitHub token saved to {}", token_path.display());
                    println!("   Add to config.toml:");
                    println!("   [providers.github_copilot]");
                    println!("   type = \"github_copilot\"");
                    println!("   token = \"{}...\"", &token[..token.len().min(8)]);
                }
                Ok(None) => println!("Authentication cancelled."),
                Err(e) => eprintln!("Auth error: {e}"),
            }
        }
        "codex" | "codex_oauth" | "chatgpt" => {
            println!("Starting ChatGPT / Codex OAuth device flow...");
            let token_path = codex_token_path();
            match run_codex_device_flow(Some(token_path.clone())).await {
                Ok(Some((access_token, _refresh_token))) => {
                    println!("✅ Codex tokens saved to {}", token_path.display());
                    println!("   Add to config.toml:");
                    println!("   [providers.codex_oauth]");
                    println!("   type = \"codex_oauth\"");
                    println!("   # (tokens auto-loaded from {})", token_path.display());
                    let masked = if access_token.len() > 8 {
                        format!("{}...", &access_token[..8])
                    } else {
                        "****".to_string()
                    };
                    println!("   # access_token: {masked}");
                }
                Ok(None) => println!("Authentication cancelled."),
                Err(e) => eprintln!("Auth error: {e}"),
            }
        }
        other => {
            eprintln!("Unknown auth provider: {other}");
            eprintln!("Supported: github, codex, anthropic, openai, gemini");
            eprintln!();
            eprintln!("For API key providers, set the key in your config.toml or as environment variable:");
            eprintln!("  ANTHROPIC_API_KEY, OPENAI_API_KEY, GEMINI_API_KEY");
        }
    }
    Ok(())
}

fn github_token_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("yolo-router")
        .join("github_token")
}

fn codex_token_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("yolo-router")
        .join("codex_oauth.json")
}


