#!/bin/bash

# YoloRouter Setup Helper
# Interactive configuration and verification tool

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

CONFIG_DIR="${CONFIG_DIR:-$HOME/.config/yolo-router}"
CONFIG_FILE="$CONFIG_DIR/config.toml"

print_info() { echo -e "${BLUE}ℹ${NC} $1"; }
print_success() { echo -e "${GREEN}✓${NC} $1"; }
print_error() { echo -e "${RED}✗${NC} $1"; }
print_warning() { echo -e "${YELLOW}⚠${NC} $1"; }
print_header() {
    echo ""
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
}

# Check if yolo-router is installed
check_installation() {
    if ! command -v yolo-router &> /dev/null; then
        print_error "yolo-router not found in PATH"
        echo ""
        echo "Please install YoloRouter first:"
        echo "  curl -sSL https://raw.githubusercontent.com/sternelee/YoloRouter/main/install.sh | bash"
        exit 1
    fi
    print_success "YoloRouter found: $(command -v yolo-router)"
}

# Test health endpoint
test_health() {
    print_info "Testing health endpoint..."
    if curl -s http://127.0.0.1:8989/health > /dev/null 2>&1; then
        print_success "Server is running"
        return 0
    else
        print_warning "Server is not running or not responding"
        return 1
    fi
}

# Setup environment variables
setup_env() {
    print_header "Setting Up Environment Variables"
    
    local env_file="$CONFIG_DIR/.env"
    
    echo "This will help you set up your API keys."
    echo ""
    
    # Anthropic
    read -p "Enter your Anthropic API Key (or press Enter to skip): " -r ANTHROPIC_KEY
    if [ -n "$ANTHROPIC_KEY" ]; then
        export ANTHROPIC_API_KEY="$ANTHROPIC_KEY"
        print_success "Anthropic API key set"
    fi
    
    # OpenAI
    read -p "Enter your OpenAI API Key (or press Enter to skip): " -r OPENAI_KEY
    if [ -n "$OPENAI_KEY" ]; then
        export OPENAI_API_KEY="$OPENAI_KEY"
        print_success "OpenAI API key set"
    fi
    
    # Gemini
    read -p "Enter your Gemini API Key (or press Enter to skip): " -r GEMINI_KEY
    if [ -n "$GEMINI_KEY" ]; then
        export GEMINI_API_KEY="$GEMINI_KEY"
        print_success "Gemini API key set"
    fi
    
    echo ""
    echo "Environment variables set for this session."
    echo ""
    echo "To make them persistent, add to your shell profile (~/.bashrc, ~/.zshrc, etc):"
    echo ""
    if [ -n "$ANTHROPIC_KEY" ]; then
        echo "export ANTHROPIC_API_KEY=\"$ANTHROPIC_KEY\""
    fi
    if [ -n "$OPENAI_KEY" ]; then
        echo "export OPENAI_API_KEY=\"$OPENAI_KEY\""
    fi
    if [ -n "$GEMINI_KEY" ]; then
        echo "export GEMINI_API_KEY=\"$GEMINI_KEY\""
    fi
    echo ""
}

# Edit configuration
edit_config() {
    print_header "Editing Configuration"
    
    if [ ! -f "$CONFIG_FILE" ]; then
        print_error "Configuration file not found at $CONFIG_FILE"
        print_info "Creating default configuration..."
        
        if [ ! -d "$CONFIG_DIR" ]; then
            mkdir -p "$CONFIG_DIR"
        fi
        
        # Create a basic config
        cat > "$CONFIG_FILE" << 'EOF'
[daemon]
port = 8989
log_level = "info"

[providers.anthropic]
type = "anthropic"
api_key = "${ANTHROPIC_API_KEY}"

[providers.openai]
type = "openai"
api_key = "${OPENAI_API_KEY}"

[providers.gemini]
type = "gemini"
api_key = "${GEMINI_API_KEY}"

[scenarios.default]
models = [
  { provider = "anthropic", model = "claude-opus", cost_tier = "high" },
  { provider = "openai", model = "gpt-4", cost_tier = "high" }
]

[routing]
fallback_enabled = true
timeout_ms = 30000
retry_count = 2
EOF
        print_success "Created default configuration at $CONFIG_FILE"
    fi
    
    echo "Opening $CONFIG_FILE with your default editor..."
    ${EDITOR:-nano} "$CONFIG_FILE"
    
    print_success "Configuration saved"
}

# Start server
start_server() {
    print_header "Starting YoloRouter Server"
    
    if test_health; then
        print_warning "Server is already running"
        read -p "Do you want to restart it? (y/n) " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            return 0
        fi
    fi
    
    if [ ! -f "$CONFIG_FILE" ]; then
        print_error "Configuration file not found at $CONFIG_FILE"
        print_info "Run 'yolo-setup edit' to create one"
        exit 1
    fi
    
    print_info "Starting YoloRouter..."
    print_info "Server will run on http://127.0.0.1:8989"
    echo ""
    echo "Press Ctrl+C to stop the server"
    echo ""
    
    export YOLO_CONFIG="$CONFIG_FILE"
    yolo-router --config "$CONFIG_FILE"
}

# Test server
test_server() {
    print_header "Testing YoloRouter Server"
    
    if ! test_health; then
        print_error "Server is not responding"
        echo ""
        echo "Start the server with: yolo-setup start"
        exit 1
    fi
    
    echo ""
    print_info "Testing configuration endpoint..."
    if curl -s http://127.0.0.1:8989/config | jq . > /dev/null 2>&1; then
        print_success "Config endpoint OK"
        curl -s http://127.0.0.1:8989/config | jq '.daemon'
    else
        print_warning "Config endpoint returned invalid JSON"
    fi
    
    echo ""
    print_info "Testing stats endpoint..."
    if curl -s http://127.0.0.1:8989/stats | jq . > /dev/null 2>&1; then
        print_success "Stats endpoint OK"
        curl -s http://127.0.0.1:8989/stats | jq .
    else
        print_warning "Stats endpoint returned invalid JSON"
    fi
}

# Test API endpoint
test_api() {
    print_header "Testing API Endpoint"
    
    if ! test_health; then
        print_error "Server is not running"
        echo "Start with: yolo-setup start"
        exit 1
    fi
    
    echo "Testing /v1/auto endpoint with a simple request..."
    echo ""
    
    local response=$(curl -s -X POST http://127.0.0.1:8989/v1/auto \
        -H "Content-Type: application/json" \
        -d '{
            "model": "claude-opus",
            "messages": [{"role": "user", "content": "Say hello!"}],
            "max_tokens": 100
        }')
    
    if echo "$response" | jq . > /dev/null 2>&1; then
        print_success "API request successful"
        echo "$response" | jq .
    else
        print_error "API request failed"
        echo "Response: $response"
    fi
}

# Show usage
show_usage() {
    print_header "YoloRouter Setup Helper Usage"
    
    echo "Commands:"
    echo ""
    echo "  ${CYAN}yolo-setup install${NC}"
    echo "    Install YoloRouter (requires curl)"
    echo ""
    echo "  ${CYAN}yolo-setup check${NC}"
    echo "    Verify installation"
    echo ""
    echo "  ${CYAN}yolo-setup env${NC}"
    echo "    Set up environment variables (API keys)"
    echo ""
    echo "  ${CYAN}yolo-setup edit${NC}"
    echo "    Edit configuration file"
    echo ""
    echo "  ${CYAN}yolo-setup start${NC}"
    echo "    Start YoloRouter daemon"
    echo ""
    echo "  ${CYAN}yolo-setup test${NC}"
    echo "    Test server endpoints"
    echo ""
    echo "  ${CYAN}yolo-setup api${NC}"
    echo "    Test API with a sample request"
    echo ""
    echo "  ${CYAN}yolo-setup logs${NC}"
    echo "    View server logs"
    echo ""
    echo "Configuration:"
    echo "  Config directory: $CONFIG_DIR"
    echo "  Config file:      $CONFIG_FILE"
    echo ""
}

# Main
case "${1:-help}" in
    check)
        check_installation
        print_success "YoloRouter is installed and ready"
        ;;
    env)
        setup_env
        ;;
    edit)
        edit_config
        ;;
    start)
        check_installation
        start_server
        ;;
    test)
        check_installation
        test_server
        ;;
    api)
        check_installation
        test_api
        ;;
    logs)
        check_installation
        if [ -f "$CONFIG_DIR/yolo-router.log" ]; then
            tail -f "$CONFIG_DIR/yolo-router.log"
        else
            print_warning "No log file found at $CONFIG_DIR/yolo-router.log"
            print_info "Start the server to generate logs"
        fi
        ;;
    help|--help|-h|"")
        show_usage
        ;;
    *)
        print_error "Unknown command: $1"
        show_usage
        exit 1
        ;;
esac
