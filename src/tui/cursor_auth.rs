use std::process::Stdio;

/// Run `cursor-agent status` to check whether the user is authenticated.
///
/// Returns `true` if stdout contains "Login successful" or "Logged in".
pub async fn check_cursor_auth(agent_path: &str) -> Result<bool, String> {
    let output = tokio::process::Command::new(agent_path)
        .args(["status"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| {
            format!(
                "Failed to spawn '{} status': {}. Is cursor-agent installed?",
                agent_path, e
            )
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{} {}", stdout, stderr);

    let is_authed = combined.contains("Login successful")
        || combined.contains("Logged in");

    Ok(is_authed)
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

    if check_cursor_auth(&agent).await? {
        println!("Cursor is already authenticated.");
        return Ok(true);
    }

    println!("Starting Cursor authentication...");
    println!("This will open your browser to log in to Cursor.");
    println!();

    run_cursor_login(&agent).await?;

    if check_cursor_auth(&agent).await? {
        println!("Cursor authentication successful!");
        Ok(true)
    } else {
        Err("Authentication verification failed. Please try again.".to_string())
    }
}
