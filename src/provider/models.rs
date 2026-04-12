use crate::config::schema::ProviderConfig;

static HTTP_CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();

fn get_http_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to build HTTP client")
    })
}

pub async fn fetch_provider_models(cfg: &ProviderConfig) -> Result<Vec<String>, String> {
    match cfg.provider_type.as_str() {
        "anthropic" => Ok(vec![
            "claude-opus-4-5".to_string(),
            "claude-sonnet-4-5".to_string(),
            "claude-haiku-4-5".to_string(),
            "claude-opus-4".to_string(),
            "claude-sonnet-4".to_string(),
            "claude-3-5-sonnet-20241022".to_string(),
            "claude-3-5-haiku-20241022".to_string(),
            "claude-3-opus-20240229".to_string(),
        ]),
        "github_copilot" => Ok(vec![
            "gpt-4o".to_string(),
            "gpt-4o-mini".to_string(),
            "gpt-4".to_string(),
            "o1-preview".to_string(),
            "o1-mini".to_string(),
            "claude-3.5-sonnet".to_string(),
            "gemini-1.5-pro".to_string(),
        ]),
        "codex_oauth" => Ok(vec![
            "gpt-4o".to_string(),
            "gpt-4o-mini".to_string(),
            "o1".to_string(),
            "o1-mini".to_string(),
            "o3-mini".to_string(),
        ]),
        "gemini" => {
            let api_key = cfg.api_key.as_deref().unwrap_or("");
            if api_key.is_empty() {
                return Ok(vec![
                    "gemini-2.0-flash".to_string(),
                    "gemini-1.5-pro".to_string(),
                    "gemini-1.5-flash".to_string(),
                ]);
            }
            fetch_gemini_models(api_key).await
        }
        _ => {
            let base_url = cfg.base_url.as_deref().unwrap_or("https://api.openai.com/v1");
            let api_key = cfg.api_key.as_deref().unwrap_or("");
            if api_key.is_empty() {
                return Err("API key not configured for this provider".to_string());
            }
            fetch_openai_compatible_models(base_url, api_key).await
        }
    }
}

async fn fetch_openai_compatible_models(base_url: &str, api_key: &str) -> Result<Vec<String>, String> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let client = get_http_client();
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("API error: HTTP {}", resp.status()));
    }
    let json: serde_json::Value = resp.json().await
        .map_err(|e| format!("Parse error: {}", e))?;
    let models: Vec<String> = json["data"]
        .as_array()
        .ok_or("Unexpected response format: missing 'data' array")?
        .iter()
        .filter_map(|m| m["id"].as_str().map(str::to_string))
        .collect();
    if models.is_empty() {
        return Err("No models returned by API".to_string());
    }
    let mut sorted = models;
    sorted.sort();
    Ok(sorted)
}

async fn fetch_gemini_models(api_key: &str) -> Result<Vec<String>, String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={}",
        api_key
    );
    let client = get_http_client();
    let resp = client.get(&url).send().await
        .map_err(|e| format!("Network error: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("Gemini API error: HTTP {}", resp.status()));
    }
    let json: serde_json::Value = resp.json().await
        .map_err(|e| format!("Parse error: {}", e))?;
    let models: Vec<String> = json["models"]
        .as_array()
        .ok_or("Unexpected response format: missing 'models' array")?
        .iter()
        .filter_map(|m| {
            m["name"].as_str().map(|n| {
                n.trim_start_matches("models/").to_string()
            })
        })
        .filter(|name| name.starts_with("gemini"))
        .collect();
    let mut sorted = models;
    sorted.sort();
    Ok(sorted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_cfg(provider_type: &str, api_key: Option<&str>, base_url: Option<&str>) -> ProviderConfig {
        ProviderConfig {
            provider_type: provider_type.to_string(),
            api_key: api_key.map(str::to_string),
            auth_type: None,
            token: None,
            base_url: base_url.map(str::to_string),
            extra: HashMap::new(),
        }
    }

    #[test]
    fn test_anthropic_returns_hardcoded() {
        let cfg = make_cfg("anthropic", Some("sk-ant-test"), None);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let models = rt.block_on(fetch_provider_models(&cfg)).unwrap();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.contains("claude")));
    }

    #[test]
    fn test_github_copilot_returns_hardcoded() {
        let cfg = make_cfg("github_copilot", None, None);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let models = rt.block_on(fetch_provider_models(&cfg)).unwrap();
        assert!(!models.is_empty());
    }

    #[test]
    fn test_codex_oauth_returns_hardcoded() {
        let cfg = make_cfg("codex_oauth", None, None);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let models = rt.block_on(fetch_provider_models(&cfg)).unwrap();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.contains("gpt-4o")));
    }

    #[test]
    fn test_gemini_no_key_returns_hardcoded() {
        let cfg = make_cfg("gemini", None, None);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let models = rt.block_on(fetch_provider_models(&cfg)).unwrap();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.contains("gemini")));
    }

    #[test]
    fn test_openai_no_key_returns_error() {
        let cfg = make_cfg("openai", None, None);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(fetch_provider_models(&cfg));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("API key not configured"));
    }

    #[test]
    fn test_anthropic_has_expected_models() {
        let cfg = make_cfg("anthropic", Some("sk-ant-test"), None);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let models = rt.block_on(fetch_provider_models(&cfg)).unwrap();
        assert!(models.contains(&"claude-3-5-sonnet-20241022".to_string()));
        assert!(models.contains(&"claude-opus-4-5".to_string()));
        assert_eq!(models.len(), 8);
    }

    #[test]
    fn test_github_copilot_has_expected_models() {
        let cfg = make_cfg("github_copilot", None, None);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let models = rt.block_on(fetch_provider_models(&cfg)).unwrap();
        assert!(models.contains(&"gpt-4o".to_string()));
        assert!(models.contains(&"claude-3.5-sonnet".to_string()));
        assert_eq!(models.len(), 7);
    }
}
