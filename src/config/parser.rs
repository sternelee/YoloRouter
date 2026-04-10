use crate::{error::YoloRouterError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use toml;

use super::schema::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub daemon: Option<DaemonConfig>,
    pub providers: Option<HashMap<String, ProviderConfig>>,
    pub scenarios: Option<HashMap<String, ScenarioConfig>>,
    pub routing: Option<RoutingConfig>,
}

fn expand_env_var(value: &str) -> String {
    if value.starts_with("${") && value.ends_with("}") {
        let env_var = &value[2..value.len() - 1];
        std::env::var(env_var).unwrap_or_else(|_| value.to_string())
    } else {
        value.to_string()
    }
}

impl Config {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let content = fs::read_to_string(path)
            .map_err(|e| YoloRouterError::ConfigError(format!("Failed to read config file: {}", e)))?;
        Self::from_string(&content)
    }

    pub fn from_string(content: &str) -> Result<Self> {
        toml::from_str(content)
            .map_err(|e| YoloRouterError::TomlError(e))
    }

    pub fn to_string(&self) -> Result<String> {
        toml::to_string_pretty(self)
            .map_err(|e| YoloRouterError::ConfigError(format!("Failed to serialize config: {}", e)))
    }

    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let content = self.to_string()?;
        fs::write(path, content)
            .map_err(|e| YoloRouterError::IoError(e))
    }

    pub fn daemon(&self) -> DaemonConfig {
        self.daemon
            .clone()
            .unwrap_or(DaemonConfig {
                port: 8080,
                log_level: "info".to_string(),
            })
    }

    pub fn providers(&self) -> HashMap<String, ProviderConfig> {
        self.providers.clone().unwrap_or_default()
    }

    pub fn scenarios(&self) -> HashMap<String, ScenarioConfig> {
        self.scenarios.clone().unwrap_or_default()
    }

    pub fn routing(&self) -> RoutingConfig {
        self.routing
            .clone()
            .unwrap_or(RoutingConfig {
                fallback_enabled: true,
                timeout_ms: 30000,
                retry_count: 2,
                confidence_threshold: 0.6,
            })
    }

    pub fn get_provider(&self, name: &str) -> Result<ProviderConfig> {
        let mut provider = self.providers()
            .get(name)
            .cloned()
            .ok_or_else(|| YoloRouterError::ConfigError(format!("Provider '{}' not found", name)))?;

        // Expand environment variables in api_key and token
        if let Some(ref api_key) = provider.api_key {
            provider.api_key = Some(expand_env_var(api_key));
        }
        if let Some(ref token) = provider.token {
            provider.token = Some(expand_env_var(token));
        }

        Ok(provider)
    }

    pub fn get_scenario(&self, name: &str) -> Result<ScenarioConfig> {
        self.scenarios()
            .get(name)
            .cloned()
            .ok_or_else(|| YoloRouterError::ConfigError(format!("Scenario '{}' not found", name)))
    }

    pub fn validate(&self) -> Result<()> {
        // Validate that referenced providers exist
        for (scenario_name, scenario) in self.scenarios() {
            for model in &scenario.models {
                if !self.providers().contains_key(&model.provider) {
                    return Err(YoloRouterError::ConfigError(
                        format!("Scenario '{}' references non-existent provider '{}'", scenario_name, model.provider)
                    ));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_string() {
        let toml_str = r#"
[daemon]
port = 8080
log_level = "debug"

[providers.anthropic]
type = "anthropic"
api_key = "test-key"
"#;
        let config = Config::from_string(toml_str).unwrap();
        assert_eq!(config.daemon().port, 8080);
        assert_eq!(config.daemon().log_level, "debug");
        assert!(config.providers().contains_key("anthropic"));
    }

    #[test]
    fn test_env_var_expansion() {
        unsafe {
            std::env::set_var("TEST_API_KEY", "secret-123");
        }
        let toml_str = r#"
[daemon]
port = 8080

[providers.test]
type = "test"
api_key = "${TEST_API_KEY}"
"#;
        let config = Config::from_string(toml_str).unwrap();
        let provider = config.get_provider("test").unwrap();
        assert_eq!(provider.api_key, Some("secret-123".to_string()));
    }

    #[test]
    fn test_config_validation() {
        let toml_str = r#"
[providers.anthropic]
type = "anthropic"
api_key = "test"

[scenarios.test]
models = [
    { provider = "nonexistent", model = "test" }
]
"#;
        let config = Config::from_string(toml_str).unwrap();
        assert!(config.validate().is_err());
    }
}
