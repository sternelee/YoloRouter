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
        let content = fs::read_to_string(path).map_err(|e| {
            YoloRouterError::ConfigError(format!("Failed to read config file: {}", e))
        })?;
        Self::from_string(&content)
    }

    pub fn from_string(content: &str) -> Result<Self> {
        toml::from_str(content).map_err(YoloRouterError::TomlError)
    }

    pub fn to_string(&self) -> Result<String> {
        toml::to_string_pretty(self)
            .map_err(|e| YoloRouterError::ConfigError(format!("Failed to serialize config: {}", e)))
    }

    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let content = self.to_string()?;
        fs::write(path, content).map_err(YoloRouterError::IoError)
    }

    pub fn daemon(&self) -> DaemonConfig {
        self.daemon.clone().unwrap_or(DaemonConfig {
            port: 8989,
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
        self.routing.clone().unwrap_or(RoutingConfig {
            fallback_enabled: true,
            timeout_ms: 30000,
            retry_count: 2,
            confidence_threshold: 0.6,
        })
    }

    pub fn get_provider(&self, name: &str) -> Result<ProviderConfig> {
        let mut provider = self.providers().get(name).cloned().ok_or_else(|| {
            YoloRouterError::ConfigError(format!("Provider '{}' not found", name))
        })?;

        // Expand environment variables in api_key and token
        if let Some(ref api_key) = provider.api_key {
            provider.api_key = Some(expand_env_var(api_key));
        }
        if let Some(ref token) = provider.token {
            provider.token = Some(expand_env_var(token));
        }
        if let Some(ref base_url) = provider.base_url {
            provider.base_url = Some(expand_env_var(base_url));
        }

        Ok(provider)
    }

    pub fn get_scenario(&self, name: &str) -> Result<ScenarioConfig> {
        self.scenarios()
            .get(name)
            .cloned()
            .ok_or_else(|| YoloRouterError::ConfigError(format!("Scenario '{}' not found", name)))
    }

    /// Append a model entry to an existing scenario's model list.
    /// Returns early if the exact same (provider, model, cost_tier) already exists.
    pub fn add_model_to_scenario(
        &mut self,
        scenario_name: &str,
        provider: &str,
        model: &str,
        cost_tier: &str,
    ) -> Result<()> {
        let scenarios = self
            .scenarios
            .get_or_insert_with(std::collections::HashMap::new);
        let scenario = scenarios.get_mut(scenario_name).ok_or_else(|| {
            YoloRouterError::ConfigError(format!("Scenario '{}' not found", scenario_name))
        })?;
        
        // Check if this exact model already exists in the scenario
        let already_exists = scenario.models.iter().any(|m| {
            m.provider == provider && m.model == model && m.cost_tier.as_deref() == Some(cost_tier)
        });
        
        if already_exists {
            return Err(YoloRouterError::ConfigError(format!(
                "Model '{}' from provider '{}' with cost tier '{}' already exists in scenario '{}'",
                model, provider, cost_tier, scenario_name
            )));
        }
        
        scenario.models.push(ModelConfig {
            provider: provider.to_string(),
            model: model.to_string(),
            cost_tier: Some(cost_tier.to_string()),
            capabilities: None,
            fallback_to: None,
        });
        Ok(())
    }

    /// Create a new scenario with one initial model entry. Errors if already exists.
    pub fn add_scenario(
        &mut self,
        scenario_name: &str,
        provider: &str,
        model: &str,
        cost_tier: &str,
    ) -> Result<()> {
        let scenarios = self
            .scenarios
            .get_or_insert_with(std::collections::HashMap::new);
        if scenarios.contains_key(scenario_name) {
            return Err(YoloRouterError::ConfigError(format!(
                "Scenario '{}' already exists",
                scenario_name
            )));
        }
        scenarios.insert(
            scenario_name.to_string(),
            ScenarioConfig {
                models: vec![ModelConfig {
                    provider: provider.to_string(),
                    model: model.to_string(),
                    cost_tier: Some(cost_tier.to_string()),
                    capabilities: None,
                    fallback_to: None,
                }],
                default_tier: None,
                match_task_types: vec![],
                match_languages: vec![],
                priority: 0,
                is_default: false,
            },
        );
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        // Validate that referenced providers exist
        for (scenario_name, scenario) in self.scenarios() {
            for model in &scenario.models {
                if !self.providers().contains_key(&model.provider) {
                    return Err(YoloRouterError::ConfigError(format!(
                        "Scenario '{}' references non-existent provider '{}'",
                        scenario_name, model.provider
                    )));
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
port = 8989
log_level = "debug"

[providers.anthropic]
type = "anthropic"
api_key = "test-key"
"#;
        let config = Config::from_string(toml_str).unwrap();
        assert_eq!(config.daemon().port, 8989);
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
port = 8989

[providers.test]
type = "test"
api_key = "${TEST_API_KEY}"
base_url = "${TEST_BASE_URL}"
"#;
        unsafe {
            std::env::set_var("TEST_BASE_URL", "https://example.com/v1");
        }
        let config = Config::from_string(toml_str).unwrap();
        let provider = config.get_provider("test").unwrap();
        assert_eq!(provider.api_key, Some("secret-123".to_string()));
        assert_eq!(
            provider.base_url,
            Some("https://example.com/v1".to_string())
        );
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

#[cfg(test)]
mod mutation_tests {
    use super::*;

    fn base_config() -> Config {
        Config::from_string(
            r#"
[daemon]
port = 8989

[providers.openai]
type = "openai"
api_key = "sk-test"

[scenarios.coding]
models = [
    { provider = "openai", model = "gpt-4", cost_tier = "high" }
]

[routing]
fallback_enabled = true
"#,
        )
        .unwrap()
    }

    #[test]
    fn test_add_model_to_existing_scenario() {
        let mut cfg = base_config();
        cfg.add_model_to_scenario("coding", "openai", "gpt-4o", "medium")
            .unwrap();
        let scenarios = cfg.scenarios();
        let models = &scenarios["coding"].models;
        assert_eq!(models.len(), 2);
        assert_eq!(models[1].model, "gpt-4o");
        assert_eq!(models[1].provider, "openai");
        assert_eq!(models[1].cost_tier.as_deref(), Some("medium"));
    }

    #[test]
    fn test_add_model_rejects_duplicate_in_scenario() {
        let mut cfg = base_config();
        // Try to add the same model that already exists in 'coding' scenario
        let result = cfg.add_model_to_scenario("coding", "openai", "gpt-4", "high");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
        
        // Verify the models list is unchanged
        let scenarios = cfg.scenarios();
        let models = &scenarios["coding"].models;
        assert_eq!(models.len(), 1);
    }

    #[test]
    fn test_add_different_model_same_provider_allowed() {
        let mut cfg = base_config();
        // Add a different model from the same provider (should succeed)
        cfg.add_model_to_scenario("coding", "openai", "gpt-4o", "medium")
            .unwrap();
        let scenarios = cfg.scenarios();
        let models = &scenarios["coding"].models;
        assert_eq!(models.len(), 2);
    }

    #[test]
    fn test_add_same_model_different_cost_tier_allowed() {
        let mut cfg = base_config();
        // Add the same model but with different cost tier (should succeed)
        cfg.add_model_to_scenario("coding", "openai", "gpt-4", "low")
            .unwrap();
        let scenarios = cfg.scenarios();
        let models = &scenarios["coding"].models;
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].cost_tier.as_deref(), Some("high"));
        assert_eq!(models[1].cost_tier.as_deref(), Some("low"));
    }

    #[test]
    fn test_add_model_to_missing_scenario_errors() {
        let mut cfg = base_config();
        let result = cfg.add_model_to_scenario("nonexistent", "openai", "gpt-4o", "low");
        assert!(result.is_err());
    }

    #[test]
    fn test_add_scenario_creates_new() {
        let mut cfg = base_config();
        cfg.add_scenario("budget", "openai", "gpt-3.5-turbo", "low")
            .unwrap();
        let scenarios = cfg.scenarios();
        assert!(scenarios.contains_key("budget"));
        assert_eq!(scenarios["budget"].models.len(), 1);
        assert_eq!(scenarios["budget"].models[0].model, "gpt-3.5-turbo");
        assert_eq!(
            scenarios["budget"].models[0].cost_tier.as_deref(),
            Some("low")
        );
    }

    #[test]
    fn test_add_scenario_duplicate_errors() {
        let mut cfg = base_config();
        let result = cfg.add_scenario("coding", "openai", "gpt-4o", "high");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_round_trips_after_mutation() {
        let mut cfg = base_config();
        cfg.add_model_to_scenario("coding", "openai", "gpt-4o", "medium")
            .unwrap();
        let toml_str = cfg.to_string().unwrap();
        let reloaded = Config::from_string(&toml_str).unwrap();
        assert_eq!(reloaded.scenarios()["coding"].models.len(), 2);
    }
}
