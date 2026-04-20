use crate::config::schema::ProviderConfig;
use crate::provider::codex_oauth::{CodexOAuthProvider, CodexQuotaInfo, CodexQuotaWindow};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, OnceLock};

static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
static MODEL_CACHE: OnceLock<Mutex<HashMap<u64, CachedModels>>> = OnceLock::new();

#[derive(Clone)]
struct CachedModels {
    models: Vec<String>,
    fetched_at_ms: i64,
}

fn get_http_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to build HTTP client")
    })
}

fn model_cache() -> &'static Mutex<HashMap<u64, CachedModels>> {
    MODEL_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn provider_models_ttl_ms() -> i64 {
    5 * 60 * 1000
}

fn provider_models_cache_key(cfg: &ProviderConfig) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    cfg.provider_type.hash(&mut hasher);
    cfg.base_url.hash(&mut hasher);
    cfg.api_key.hash(&mut hasher);
    cfg.token.hash(&mut hasher);
    hasher.finish()
}

fn cached_models_for(cfg: &ProviderConfig, now_ms: i64) -> Option<Vec<String>> {
    let cache = model_cache().lock().ok()?;
    let entry = cache.get(&provider_models_cache_key(cfg))?;
    // Cache is valid while elapsed < TTL (exclusive upper bound).
    // An entry fetched at time T is usable for calls where now < T + ttl.
    if now_ms.saturating_sub(entry.fetched_at_ms) < provider_models_ttl_ms() {
        Some(entry.models.clone())
    } else {
        None
    }
}

fn store_cached_models(cfg: &ProviderConfig, models: Vec<String>, now_ms: i64) {
    if let Ok(mut cache) = model_cache().lock() {
        cache.insert(
            provider_models_cache_key(cfg),
            CachedModels {
                models,
                fetched_at_ms: now_ms,
            },
        );
    }
}

pub fn static_provider_models(provider_type: &str) -> Option<Vec<String>> {
    match provider_type {
        "anthropic" => Some(vec![
            "claude-opus-4.6".to_string(),
            "claude-opus-4.5".to_string(),
            "claude-sonnet-4.6".to_string(),
            "claude-sonnet-4.5".to_string(),
            "claude-haiku-4.5".to_string(),
            "claude-opus-4-5".to_string(),
            "claude-sonnet-4-5".to_string(),
            "claude-haiku-4-5".to_string(),
            "claude-opus-4".to_string(),
            "claude-sonnet-4".to_string(),
            "claude-3-5-sonnet-20241022".to_string(),
            "claude-3-5-haiku-20241022".to_string(),
            "claude-3-opus-20240229".to_string(),
            "claude-opus".to_string(),
            "claude-sonnet".to_string(),
            "claude-haiku".to_string(),
        ]),
        "github_copilot" | "github" => Some(vec![
            "gpt-5.4".to_string(),
            "gpt-5.4-mini".to_string(),
            "gpt-5.3-codex".to_string(),
            "gpt-5.2-codex".to_string(),
            "gpt-5.2".to_string(),
            "gpt-5.1".to_string(),
            "gpt-5-mini".to_string(),
            "gpt-4.1".to_string(),
            "gpt-4o".to_string(),
            "gpt-4-turbo".to_string(),
            "claude-sonnet-4.6".to_string(),
            "claude-sonnet-4.5".to_string(),
            "claude-haiku-4.5".to_string(),
            "claude-opus-4.6".to_string(),
            "claude-opus-4.5".to_string(),
            "claude-sonnet-4".to_string(),
        ]),
        "codex_oauth" => Some(vec![
            "gpt-5.4".to_string(),
            "gpt-5.4-mini".to_string(),
            "gpt-5.3-codex".to_string(),
            "gpt-5.3-codex-spark".to_string(),
            "gpt-5.2".to_string(),
            "gpt-5.2-codex".to_string(),
            "gpt-5.1".to_string(),
            "gpt-5-mini".to_string(),
            "gpt-4.1".to_string(),
            "gpt-4o".to_string(),
            "gpt-4o-mini".to_string(),
            "gpt-4-turbo".to_string(),
            "gpt-4".to_string(),
            "o1".to_string(),
            "o1-preview".to_string(),
            "o1-mini".to_string(),
            "gpt-3.5-turbo".to_string(),
        ]),
        "openai" | "codex" => Some(vec![
            "gpt-5.4".to_string(),
            "gpt-5.4-mini".to_string(),
            "gpt-5.3-codex".to_string(),
            "gpt-5.3-codex-spark".to_string(),
            "gpt-5.2".to_string(),
            "gpt-5.2-codex".to_string(),
            "gpt-5.1".to_string(),
            "gpt-5-mini".to_string(),
            "gpt-4.1".to_string(),
            "gpt-4o".to_string(),
            "gpt-4o-mini".to_string(),
            "gpt-4-turbo".to_string(),
            "gpt-4".to_string(),
            "o1".to_string(),
            "o1-preview".to_string(),
            "o1-mini".to_string(),
            "gpt-3.5-turbo".to_string(),
        ]),
        "gemini" => Some(vec![
            "gemini-2.0-flash".to_string(),
            "gemini-1.5-pro".to_string(),
            "gemini-1.5-flash".to_string(),
            "gemini-pro".to_string(),
            "gemini-pro-vision".to_string(),
        ]),
        "cursor" => Some(vec![
            "auto".to_string(),
            "composer-1.5".to_string(),
            "composer-1".to_string(),
            "opus-4.6-thinking".to_string(),
            "opus-4.6".to_string(),
            "sonnet-4.6".to_string(),
            "sonnet-4.6-thinking".to_string(),
            "opus-4.5".to_string(),
            "opus-4.5-thinking".to_string(),
            "sonnet-4.5".to_string(),
            "sonnet-4.5-thinking".to_string(),
            "gpt-5.4-high".to_string(),
            "gpt-5.4-high-fast".to_string(),
            "gpt-5.4-xhigh".to_string(),
            "gpt-5.4-xhigh-fast".to_string(),
            "gpt-5.4-medium".to_string(),
            "gpt-5.4-medium-fast".to_string(),
            "gpt-5.3-codex".to_string(),
            "gpt-5.3-codex-fast".to_string(),
            "gpt-5.3-codex-low".to_string(),
            "gpt-5.3-codex-low-fast".to_string(),
            "gpt-5.3-codex-high".to_string(),
            "gpt-5.3-codex-high-fast".to_string(),
            "gpt-5.3-codex-xhigh".to_string(),
            "gpt-5.3-codex-xhigh-fast".to_string(),
            "gpt-5.3-codex-spark-preview".to_string(),
            "gpt-5.2".to_string(),
            "gpt-5.2-high".to_string(),
            "gpt-5.2-codex".to_string(),
            "gpt-5.2-codex-fast".to_string(),
            "gpt-5.2-codex-low".to_string(),
            "gpt-5.2-codex-low-fast".to_string(),
            "gpt-5.2-codex-high".to_string(),
            "gpt-5.2-codex-high-fast".to_string(),
            "gpt-5.2-codex-xhigh".to_string(),
            "gpt-5.2-codex-xhigh-fast".to_string(),
            "gpt-5.1-codex-max".to_string(),
            "gpt-5.1-codex-max-high".to_string(),
            "gpt-5.1-codex-mini".to_string(),
            "gpt-5.1-high".to_string(),
            "gemini-3.1-pro".to_string(),
            "gemini-3-pro".to_string(),
            "gemini-3-flash".to_string(),
            "grok".to_string(),
            "kimi-k2.5".to_string(),
        ]),
        _ => None,
    }
}

pub async fn fetch_provider_models(cfg: &ProviderConfig) -> Result<Vec<String>, String> {
    let now_ms = now_epoch_ms();
    if let Some(models) = cached_models_for(cfg, now_ms) {
        return Ok(models);
    }

    let models = match cfg.provider_type.as_str() {
        "anthropic" | "github_copilot" | "github" | "codex_oauth" | "cursor" => {
            Ok(static_provider_models(&cfg.provider_type).unwrap_or_default())
        }
        "gemini" => {
            let api_key = cfg.api_key.as_deref().unwrap_or("");
            if api_key.is_empty() {
                return Ok(static_provider_models("gemini").unwrap_or_default());
            }
            fetch_gemini_models(api_key)
                .await
                .or_else(|_| Ok(static_provider_models("gemini").unwrap_or_default()))
        }
        _ => {
            let base_url = cfg
                .base_url
                .as_deref()
                .unwrap_or("https://api.openai.com/v1");
            let api_key = cfg.api_key.as_deref().unwrap_or("");
            if api_key.is_empty() {
                if let Some(models) = static_provider_models(&cfg.provider_type) {
                    return Ok(models);
                }
                return Err("API key not configured for this provider".to_string());
            }
            match fetch_openai_compatible_models(base_url, api_key).await {
                Ok(models) => Ok(models),
                Err(error) => {
                    if let Some(models) = static_provider_models(&cfg.provider_type) {
                        Ok(models)
                    } else {
                        Err(error)
                    }
                }
            }
        }
    }?;

    store_cached_models(cfg, models.clone(), now_ms);
    Ok(models)
}

pub async fn fetch_provider_quota(cfg: &ProviderConfig) -> Result<CodexQuotaInfo, String> {
    match cfg.provider_type.as_str() {
        "codex_oauth" => build_codex_oauth_provider(cfg)
            .fetch_usage(codex_oauth_account_id(cfg).as_deref())
            .await
            .map_err(|e| e.to_string()),
        _ => Err("Quota lookup is only supported for codex_oauth providers".to_string()),
    }
}

pub fn is_codex_oauth_provider(cfg: &ProviderConfig) -> bool {
    cfg.provider_type == "codex_oauth"
}

pub fn codex_quota_ttl_ms() -> i64 {
    5 * 60 * 1000
}

pub fn now_epoch_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub fn should_refresh_quota(last_queried_at_ms: Option<i64>, now_ms: i64, ttl_ms: i64) -> bool {
    match last_queried_at_ms {
        Some(last) => now_ms.saturating_sub(last) >= ttl_ms,
        None => true,
    }
}

pub fn codex_window_label(window_seconds: Option<i64>) -> String {
    match window_seconds {
        Some(18_000) => "5h window".to_string(),
        Some(604_800) => "7d window".to_string(),
        Some(secs) if secs % 86_400 == 0 => format!("{}d window", secs / 86_400),
        Some(secs) if secs % 3_600 == 0 => format!("{}h window", secs / 3_600),
        Some(secs) if secs >= 60 => format!("{}m window", secs / 60),
        Some(secs) if secs > 0 => format!("{}s window", secs),
        _ => "Usage window".to_string(),
    }
}

pub fn format_reset_at(reset_at: Option<i64>, now_ms: i64) -> String {
    match reset_at {
        Some(ts) => {
            let reset_ms = if ts > 1_000_000_000_000 {
                ts
            } else {
                ts * 1000
            };
            let delta_secs = (reset_ms.saturating_sub(now_ms) / 1000).max(0);
            if delta_secs >= 86_400 {
                format!("in {}d", delta_secs / 86_400)
            } else if delta_secs >= 3_600 {
                format!("in {}h", delta_secs / 3_600)
            } else if delta_secs >= 60 {
                format!("in {}m", delta_secs / 60)
            } else {
                format!("in {}s", delta_secs)
            }
        }
        None => "unknown".to_string(),
    }
}

pub fn codex_quota_rows(quota: &CodexQuotaInfo, now_ms: i64) -> Vec<(String, String, String)> {
    [
        quota.rate_limit.primary_window.as_ref(),
        quota.rate_limit.secondary_window.as_ref(),
    ]
    .into_iter()
    .flatten()
    .map(|window| codex_quota_row(window, now_ms))
    .collect()
}

fn codex_quota_row(window: &CodexQuotaWindow, now_ms: i64) -> (String, String, String) {
    (
        codex_window_label(window.limit_window_seconds),
        window
            .used_percent
            .map(|used| format!("{used:.1}% used"))
            .unwrap_or_else(|| "usage unknown".to_string()),
        format_reset_at(window.reset_at, now_ms),
    )
}

fn codex_oauth_account_id(cfg: &ProviderConfig) -> Option<String> {
    cfg.extra
        .get("account_id")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
}

fn codex_oauth_token_path(cfg: &ProviderConfig) -> Option<std::path::PathBuf> {
    cfg.extra
        .get("token_path")
        .and_then(|value| value.as_str())
        .map(std::path::PathBuf::from)
        .or_else(|| dirs::config_dir().map(|dir| dir.join("yolo-router").join("codex_oauth.json")))
}

fn codex_oauth_refresh_token(cfg: &ProviderConfig) -> Option<String> {
    cfg.extra
        .get("refresh_token")
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn build_codex_oauth_provider(cfg: &ProviderConfig) -> CodexOAuthProvider {
    let token_path = codex_oauth_token_path(cfg);
    if let Some(access_token) = cfg.api_key.clone().or(cfg.token.clone()) {
        CodexOAuthProvider::with_access_token(
            access_token,
            codex_oauth_refresh_token(cfg),
            token_path,
        )
    } else {
        CodexOAuthProvider::new(token_path)
    }
}

async fn fetch_openai_compatible_models(
    base_url: &str,
    api_key: &str,
) -> Result<Vec<String>, String> {
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
    let json: serde_json::Value = resp
        .json()
        .await
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
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("Gemini API error: HTTP {}", resp.status()));
    }
    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;
    let models: Vec<String> = json["models"]
        .as_array()
        .ok_or("Unexpected response format: missing 'models' array")?
        .iter()
        .filter_map(|m| {
            m["name"]
                .as_str()
                .map(|n| n.trim_start_matches("models/").to_string())
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
    use crate::provider::codex_oauth::CodexQuotaRateLimit;
    use std::collections::HashMap;

    fn make_cfg(
        provider_type: &str,
        api_key: Option<&str>,
        base_url: Option<&str>,
    ) -> ProviderConfig {
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
        assert_eq!(models, static_provider_models("anthropic").unwrap());
    }

    #[test]
    fn test_github_copilot_returns_hardcoded() {
        let cfg = make_cfg("github_copilot", None, None);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let models = rt.block_on(fetch_provider_models(&cfg)).unwrap();
        assert!(!models.is_empty());
        assert_eq!(models, static_provider_models("github_copilot").unwrap());
    }

    #[test]
    fn test_codex_oauth_returns_hardcoded() {
        let cfg = make_cfg("codex_oauth", None, None);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let models = rt.block_on(fetch_provider_models(&cfg)).unwrap();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.contains("gpt-5.4")));
        assert_eq!(models, static_provider_models("codex_oauth").unwrap());
    }

    #[test]
    fn test_gemini_no_key_returns_hardcoded() {
        let cfg = make_cfg("gemini", None, None);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let models = rt.block_on(fetch_provider_models(&cfg)).unwrap();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.contains("gemini")));
        assert_eq!(models, static_provider_models("gemini").unwrap());
    }

    #[test]
    fn test_openai_no_key_returns_error() {
        let cfg = make_cfg("openai", None, None);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let models = rt.block_on(fetch_provider_models(&cfg)).unwrap();
        assert_eq!(models, static_provider_models("openai").unwrap());
    }

    #[test]
    fn test_anthropic_has_expected_models() {
        let cfg = make_cfg("anthropic", Some("sk-ant-test"), None);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let models = rt.block_on(fetch_provider_models(&cfg)).unwrap();
        assert!(models.contains(&"claude-3-5-sonnet-20241022".to_string()));
        assert!(models.contains(&"claude-opus-4-5".to_string()));
        assert!(models.len() >= 8);
    }

    #[test]
    fn test_github_copilot_has_expected_models() {
        let cfg = make_cfg("github_copilot", None, None);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let models = rt.block_on(fetch_provider_models(&cfg)).unwrap();
        assert!(models.contains(&"gpt-5.4".to_string()));
        assert!(models.contains(&"gpt-5.3-codex".to_string()));
        assert!(models.contains(&"claude-sonnet-4.6".to_string()));
        assert!(models.contains(&"gpt-5.2-codex".to_string()));
        assert!(models.contains(&"claude-opus-4.5".to_string()));
        assert!(models.len() >= 14);
    }

    #[test]
    fn test_static_catalog_openai_and_codex_stay_aligned() {
        let openai = static_provider_models("openai").unwrap();
        let codex = static_provider_models("codex").unwrap();
        assert_eq!(openai, codex);
        assert!(openai.contains(&"gpt-5.3-codex-spark".to_string()));
    }

    #[test]
    fn test_provider_models_cache_key_changes_with_credentials() {
        let cfg_a = make_cfg("openai", Some("key-a"), Some("https://api.openai.com/v1"));
        let cfg_b = make_cfg("openai", Some("key-b"), Some("https://api.openai.com/v1"));
        assert_ne!(
            provider_models_cache_key(&cfg_a),
            provider_models_cache_key(&cfg_b)
        );
    }

    #[test]
    fn test_cached_models_respect_ttl() {
        let cfg = make_cfg("openai", Some("key-a"), Some("https://api.openai.com/v1"));
        let now_ms = 1_000_000;
        store_cached_models(&cfg, vec!["cached-model".to_string()], now_ms);

        assert_eq!(
            cached_models_for(&cfg, now_ms + provider_models_ttl_ms() - 1),
            Some(vec!["cached-model".to_string()])
        );
        assert_eq!(
            cached_models_for(&cfg, now_ms + provider_models_ttl_ms()),
            None
        );
    }

    #[test]
    fn test_should_refresh_quota_when_missing_or_stale() {
        assert!(should_refresh_quota(None, 1_000, codex_quota_ttl_ms()));
        assert!(!should_refresh_quota(Some(4_000), 10_000, 10_000));
        assert!(should_refresh_quota(Some(0), 10_000, 10_000));
    }

    #[test]
    fn test_codex_window_label_formats_known_windows() {
        assert_eq!(codex_window_label(Some(18_000)), "5h window");
        assert_eq!(codex_window_label(Some(604_800)), "7d window");
        assert_eq!(codex_window_label(Some(7_200)), "2h window");
    }

    #[test]
    fn test_codex_quota_rows_formats_usage_and_reset() {
        let quota = CodexQuotaInfo {
            rate_limit: CodexQuotaRateLimit {
                primary_window: Some(CodexQuotaWindow {
                    used_percent: Some(42.5),
                    limit_window_seconds: Some(18_000),
                    reset_at: Some(7_200),
                }),
                secondary_window: None,
            },
            queried_at_ms: 0,
        };

        let rows = codex_quota_rows(&quota, 0);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "5h window");
        assert_eq!(rows[0].1, "42.5% used");
        assert_eq!(rows[0].2, "in 2h");
    }

    #[test]
    fn test_is_codex_oauth_provider() {
        assert!(is_codex_oauth_provider(&make_cfg(
            "codex_oauth",
            None,
            None
        )));
        assert!(!is_codex_oauth_provider(&make_cfg("openai", None, None)));
    }
}
