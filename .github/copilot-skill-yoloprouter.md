# YoloRouter Copilot Skill

This skill helps users configure YoloRouter through interactive dialogue with Copilot. YoloRouter is an AI model routing proxy that intelligently routes requests between multiple providers (Anthropic, OpenAI, Google Gemini, GitHub Codex, etc.).

## What You Can Help Users Do

1. **Set up providers**: Guide users in configuring API credentials for different AI providers
2. **Create scenarios**: Help define intelligent routing scenarios based on task types
3. **Configure fallback chains**: Set up redundancy chains to ensure reliability
4. **Understand configuration**: Explain the TOML configuration structure
5. **Troubleshoot routing**: Help debug why requests are being routed to specific providers

## Key Concepts

- **Provider**: An AI service (Anthropic, OpenAI, Gemini, GitHub Codex) configured with credentials
- **Scenario**: A named routing configuration that maps multiple providers/models for specific task types
- **Model Chain**: An ordered list of model configurations to try in sequence (with fallback)
- **Fallback**: Automatic retry mechanism when a model request fails

## Configuration Structure

YoloRouter uses TOML format with these main sections:

```toml
[daemon]
port = 8080
log_level = "info"

[providers.{name}]
type = "{anthropic|openai|gemini|github|codex|generic}"
api_key = "${ENVIRONMENT_VAR}"  # Or direct key
# provider-specific fields follow

[scenarios.{scenario_name}]
models = [
  { provider = "name", model = "model-id", cost_tier = "low|medium|high" },
  # ... more models in fallback order
]
default_tier = "high"

[routing]
fallback_enabled = true
timeout_ms = 30000
retry_count = 2
```

## Interactive Configuration Guide

When a user asks to configure YoloRouter, follow this process:

### Step 1: Understand Their Setup
Ask about:
- Which AI providers they use or want to use
- Current API keys/tokens available
- Expected usage patterns (heavy coding work? analysis? general tasks?)
- Reliability needs (can they tolerate slow responses? Need redundancy?)

### Step 2: Help Gather Credentials
For each provider:
- **Anthropic**: Ask for Claude API key from console.anthropic.com
- **OpenAI**: Ask for API key from platform.openai.com
- **Google Gemini**: Ask for API key from makersuite.google.com
- **GitHub Codex**: Ask for GitHub token (from github.com/settings/tokens)
- **Custom**: Ask for endpoint URL and auth details

Guide them to store in environment variables (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, etc.)

### Step 3: Define Scenarios
Help them identify use cases and create scenarios:
- **high_quality_coding**: Use expensive, capable models (Opus, GPT-4)
- **quick_analysis**: Use faster, cheaper models (Haiku, GPT-3.5)
- **code_review**: Route to code-aware models
- **general_chat**: Route to any available model

### Step 4: Create Fallback Chains
For each scenario, help order models by:
- Cost (expensive first for important tasks, cheap first for cost-sensitive)
- Capability (capable first for complex tasks)
- Availability (preferred provider first)

Example:
```toml
[scenarios.high_quality_coding]
models = [
  { provider = "anthropic", model = "claude-opus", cost_tier = "high" },
  { provider = "openai", model = "gpt-4", cost_tier = "high" },
  { provider = "openai", model = "gpt-3.5-turbo", cost_tier = "medium" },
]
default_tier = "high"
```

### Step 5: Output Configuration
Generate the complete TOML file based on their inputs. Offer to:
- Validate the configuration syntax
- Explain any complex parts
- Help test endpoints once deployed

## Common Patterns

### Cost-Conscious Setup
```toml
[scenarios.budget_conscious]
models = [
  { provider = "openai", model = "gpt-3.5-turbo", cost_tier = "low" },
  { provider = "anthropic", model = "claude-haiku", cost_tier = "low" },
]
default_tier = "low"
```

### High-Quality with Fallback
```toml
[scenarios.production_coding]
models = [
  { provider = "anthropic", model = "claude-opus", cost_tier = "high" },
  { provider = "openai", model = "gpt-4", cost_tier = "high" },
  { provider = "anthropic", model = "claude-sonnet", cost_tier = "medium" },
]
default_tier = "high"
```

### Multi-Task Setup
```toml
[scenarios.coding]
# Complex coding tasks - use best models
models = [...]

[scenarios.analysis]
# Data analysis - use analytical models
models = [...]

[scenarios.general]
# Default fallback - use cheapest available
models = [...]
```

## Endpoints and Usage

Once configured, YoloRouter runs on specified port and provides:

- `POST /v1/anthropic` - Route to Anthropic provider
- `POST /v1/openai` - Route to OpenAI provider
- `POST /v1/gemini` - Route to Google Gemini
- `POST /v1/codex` - Route to GitHub Codex
- `POST /v1/auto` - Auto-detect best provider based on request
- `GET /health` - Health check
- `GET /config` - View current configuration
- `GET /stats` - View request statistics

## Helper Commands

When users ask about configuration, suggest:

1. **View example config**: Ask them to check `config.example.toml` in the repo
2. **Validate TOML syntax**: Recommend using `toml-cli` or online TOML validators
3. **Test configuration**: Help them start the server and test endpoints with curl/Postman
4. **Debug routing**: Check logs and use `/stats` endpoint to see which providers are being used
5. **Adjust scenarios**: Help modify `[scenarios]` section to match usage patterns better

## Environment Variable Support

YoloRouter supports `${VAR_NAME}` syntax in TOML for secure credential management:

```toml
[providers.anthropic]
api_key = "${ANTHROPIC_API_KEY}"  # Reads from environment
```

Guide users to set environment variables before starting the daemon:
```bash
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-..."
yolo-router --config config.toml
```

## Troubleshooting Help

When users report issues:

1. **"My requests go to wrong provider"**: Check scenario definitions and fallback_enabled setting
2. **"Getting auth errors"**: Verify API keys are correct and set in environment/config
3. **"Requests are slow"**: Check timeout_ms setting, provider availability, and /stats for patterns
4. **"Fallback not working"**: Verify models exist in provider, check retry_count and fallback_enabled
5. **"Can't connect to provider"**: Check base_url, auth credentials, and network connectivity

## When to Escalate to Real Support

- Actual API provider outages or errors (check their status pages)
- Advanced routing logic requiring code changes
- Performance optimization beyond config tuning
- Integration with external systems beyond HTTP API

