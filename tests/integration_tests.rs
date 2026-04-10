#[cfg(test)]
mod integration_tests {
    use yolo_router::{Config, ChatRequest, ChatMessage};

    #[test]
    fn test_config_round_trip() {
        let toml_str = r#"
[daemon]
port = 8080
log_level = "info"

[providers.anthropic]
type = "anthropic"
api_key = "sk-ant-test"

[providers.openai]
type = "openai"
api_key = "sk-test"

[scenarios.coding]
models = [
    { provider = "anthropic", model = "claude-opus", cost_tier = "high" },
    { provider = "openai", model = "gpt-4", cost_tier = "high" }
]
default_tier = "high"

[routing]
fallback_enabled = true
timeout_ms = 30000
retry_count = 2
"#;

        let config = Config::from_string(toml_str)
            .expect("Failed to parse config");
        
        assert_eq!(config.daemon().port, 8080);
        assert!(config.providers().contains_key("anthropic"));
        assert!(config.providers().contains_key("openai"));
        assert!(config.scenarios().contains_key("coding"));
        
        let config_str = config.to_string()
            .expect("Failed to serialize config");
        assert!(!config_str.is_empty());
    }

    #[test]
    fn test_chat_request_creation() {
        let request = ChatRequest {
            model: "claude-opus".to_string(),
            messages: vec![
                ChatMessage {
                    role: "user".to_string(),
                    content: "Hello!".to_string(),
                }
            ],
            max_tokens: Some(1000),
            temperature: Some(0.7),
            top_p: None,
        };

        assert_eq!(request.model, "claude-opus");
        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.messages[0].role, "user");
        assert_eq!(request.max_tokens, Some(1000));
    }

    #[test]
    fn test_multi_provider_config() {
        let toml_str = r#"
[daemon]
port = 8080

[providers.anthropic]
type = "anthropic"
api_key = "sk-ant-1"

[providers.openai]
type = "openai"
api_key = "sk-openai-1"

[providers.gemini]
type = "gemini"
api_key = "sk-gemini-1"

[providers.github]
type = "github"
token = "sk-github-1"

[scenarios.test]
models = [
    { provider = "anthropic", model = "claude-opus", cost_tier = "high" },
    { provider = "openai", model = "gpt-4", cost_tier = "high" },
    { provider = "gemini", model = "gemini-pro", cost_tier = "medium" },
    { provider = "github", model = "codex", cost_tier = "low" }
]

[routing]
fallback_enabled = true
"#;

        let config = Config::from_string(toml_str)
            .expect("Failed to parse multi-provider config");
        
        assert_eq!(config.providers().len(), 4);
        assert!(config.providers().contains_key("anthropic"));
        assert!(config.providers().contains_key("openai"));
        assert!(config.providers().contains_key("gemini"));
        assert!(config.providers().contains_key("github"));
    }

    #[test]
    fn test_scenario_validation() {
        let toml_str = r#"
[daemon]
port = 8080

[providers.anthropic]
type = "anthropic"
api_key = "sk-ant-1"

[scenarios.test]
models = [
    { provider = "anthropic", model = "claude-opus", cost_tier = "high" },
    { provider = "nonexistent", model = "bad-model", cost_tier = "high" }
]

[routing]
fallback_enabled = true
"#;

        let config = Config::from_string(toml_str)
            .expect("Failed to parse config");
        
        // Validation should fail because 'nonexistent' provider doesn't exist
        let result = config.validate();
        assert!(result.is_err(), "Should detect missing provider");
    }

    #[test]
    fn test_routing_config_defaults() {
        let toml_str = r#"
[daemon]
port = 8080

[providers.anthropic]
type = "anthropic"
api_key = "sk-test"

[routing]
fallback_enabled = true
"#;

        let config = Config::from_string(toml_str)
            .expect("Failed to parse config");
        
        let routing = config.routing();
        assert_eq!(routing.fallback_enabled, true);
        assert_eq!(routing.timeout_ms, 30000);
        // retry_count has no default, so it's 0
        assert_eq!(routing.retry_count, 0);
    }

    #[test]
    fn test_daemon_config_validation() {
        let toml_str = r#"
[daemon]
port = 8080
log_level = "debug"

[providers.anthropic]
type = "anthropic"
api_key = "sk-test"

[routing]
fallback_enabled = true
"#;

        let config = Config::from_string(toml_str)
            .expect("Failed to parse config");
        
        let daemon = config.daemon();
        assert_eq!(daemon.port, 8080);
        assert_eq!(daemon.log_level, "debug");
    }

    #[test]
    fn test_complex_scenario_chain() {
        let toml_str = r#"
[daemon]
port = 8080

[providers.anthropic]
type = "anthropic"
api_key = "sk-ant"

[providers.openai]
type = "openai"
api_key = "sk-openai"

[providers.gemini]
type = "gemini"
api_key = "sk-gemini"

[scenarios.production_coding]
models = [
    { provider = "anthropic", model = "claude-opus", cost_tier = "high" },
    { provider = "openai", model = "gpt-4", cost_tier = "high" },
    { provider = "anthropic", model = "claude-sonnet", cost_tier = "medium" },
    { provider = "openai", model = "gpt-3.5-turbo", cost_tier = "low" },
    { provider = "gemini", model = "gemini-pro", cost_tier = "low" }
]
default_tier = "high"

[scenarios.budget_mode]
models = [
    { provider = "openai", model = "gpt-3.5-turbo", cost_tier = "low" },
    { provider = "gemini", model = "gemini-pro", cost_tier = "low" }
]
default_tier = "low"

[routing]
fallback_enabled = true
timeout_ms = 60000
retry_count = 3
"#;

        let config = Config::from_string(toml_str)
            .expect("Failed to parse config");
        
        assert!(config.validate().is_ok());
        assert_eq!(config.scenarios().len(), 2);
        
        let scenarios = config.scenarios();
        let prod_scenario = scenarios.get("production_coding")
            .expect("Missing production_coding scenario");
        assert_eq!(prod_scenario.models.len(), 5);
        
        let budget_scenario = scenarios.get("budget_mode")
            .expect("Missing budget_mode scenario");
        assert_eq!(budget_scenario.models.len(), 2);
    }
}
