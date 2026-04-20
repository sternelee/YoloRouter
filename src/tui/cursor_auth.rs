use std::path::PathBuf;
use std::process::Stdio;

/// Return possible paths to the Cursor CLI config file that holds the auth token.
pub fn cursor_token_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".cursor").join("cli-config.json"));
    }
    if let Some(config) = dirs::config_dir() {
        paths.push(config.join("cursor").join("cli-config.json"));
    }
    paths
}

/// Check whether the user is already authenticated with Cursor.
pub async fn check_cursor_auth() -> Result<bool, String> {
    for path in cursor_token_paths() {
        if path.exists() {
            let content = tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
            let json: serde_json::Value = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;
            if json.get("token").or(json.get("accessToken")).is_some() {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Run `cursor-agent login` interactively (inherits stdin/stdout/stderr).
pub async fn run_cursor_login(agent_path: &str) -> Result<(), String> {
    let mut child = tokio::process::Command::new(agent_path)
        .arg("login")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| {
            format!(
                "Failed to spawn '{} login': {}. Is cursor-agent installed?",
                agent_path, e
            )
        })?;

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Failed to wait for cursor-agent login: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "cursor-agent login exited with code {:?}",
            status.code()
        ))
    }
}

/// Cursor authentication flow.
///
/// Returns `Ok(true)` if the user is authenticated (either already or after login).
pub async fn run_cursor_device_flow(agent_path: Option<String>) -> Result<bool, String> {
    let agent = agent_path.unwrap_or_else(|| {
        std::env::var("CURSOR_AGENT_EXECUTABLE").unwrap_or_else(|_| "cursor-agent".to_string())
    });

    if check_cursor_auth().await? {
        println!("Cursor is already authenticated.");
        return Ok(true);
    }

    println!("Starting Cursor authentication...");
    println!("This will open your browser to log in to Cursor.");
    println!();

    run_cursor_login(&agent).await?;

    if check_cursor_auth().await? {
        println!("Cursor authentication successful!");
        Ok(true)
    } else {
        Err("Authentication verification failed. Please try again.".to_string())
    }
}
