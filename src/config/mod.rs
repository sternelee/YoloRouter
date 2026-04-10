mod parser;
pub use parser::Config;

pub mod schema {
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct DaemonConfig {
        pub port: u16,
        #[serde(default = "default_log_level")]
        pub log_level: String,
    }

    fn default_log_level() -> String {
        "info".to_string()
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ProviderConfig {
        #[serde(rename = "type")]
        pub provider_type: String,
        pub api_key: Option<String>,
        pub auth_type: Option<String>,
        pub token: Option<String>,
        pub base_url: Option<String>,
        #[serde(flatten)]
        pub extra: HashMap<String, toml::Value>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ModelConfig {
        pub provider: String,
        pub model: String,
        #[serde(default)]
        pub cost_tier: Option<String>,
        #[serde(default)]
        pub capabilities: Option<Vec<String>>,
        #[serde(default)]
        pub fallback_to: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ScenarioConfig {
        pub models: Vec<ModelConfig>,
        #[serde(default)]
        pub default_tier: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RoutingConfig {
        #[serde(default)]
        pub fallback_enabled: bool,
        #[serde(default = "default_timeout")]
        pub timeout_ms: u64,
        #[serde(default)]
        pub retry_count: u32,
    }

    fn default_timeout() -> u64 {
        30000
    }
}
