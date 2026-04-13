#!/bin/bash

# YoloRouter Installation Script
# Installs YoloRouter system-wide on macOS, Linux, and other Unix-like systems

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
REPO_URL="${REPO_URL:-https://github.com/sternelee/YoloRouter.git}"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
CONFIG_DIR="${CONFIG_DIR:-$HOME/.config/yolo-router}"
VERSION="${VERSION:-latest}"

# Detect OS
OS=""
ARCH=""
detect_os() {
    case "$(uname -s)" in
        Darwin*)
            OS="darwin"
            ;;
        Linux*)
            OS="linux"
            ;;
        *)
            OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
            ;;
    esac

    case "$(uname -m)" in
        x86_64)
            ARCH="x86_64"
            ;;
        aarch64)
            ARCH="aarch64"
            ;;
        arm64)
            ARCH="aarch64"
            ;;
        *)
            ARCH="$(uname -m)"
            ;;
    esac
}

# Print colored output
print_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

print_header() {
    echo ""
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
}

# Check prerequisites
check_prerequisites() {
    print_header "Checking Prerequisites"

    local missing_tools=()

    if ! command -v git &> /dev/null; then
        missing_tools+=("git")
    else
        print_success "git found"
    fi

    if ! command -v cargo &> /dev/null; then
        missing_tools+=("cargo")
    else
        local rust_version=$(rustc --version 2>/dev/null | grep -oP '\d+\.\d+' | head -1)
        print_success "Rust/Cargo found (version: $rust_version)"
    fi

    if [ ${#missing_tools[@]} -gt 0 ]; then
        print_error "Missing required tools: ${missing_tools[@]}"
        echo ""
        echo "Installation instructions:"
        for tool in "${missing_tools[@]}"; do
            case "$tool" in
                git)
                    echo "  $tool: https://git-scm.com/download"
                    ;;
                cargo)
                    echo "  $tool: https://rustup.rs/ (curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh)"
                    ;;
            esac
        done
        exit 1
    fi
}

# Clone or update repository
setup_repository() {
    print_header "Setting Up Repository"

    local temp_dir=$(mktemp -d)
    print_info "Cloning YoloRouter from $REPO_URL into $temp_dir"

    if git clone --depth 1 "$REPO_URL" "$temp_dir/yolo-router"; then
        print_success "Repository cloned successfully"
        cd "$temp_dir/yolo-router"
    else
        print_error "Failed to clone repository"
        exit 1
    fi
}

# Build from source
build_from_source() {
    print_header "Building YoloRouter from Source"

    print_info "Building release binary..."
    if cargo build --release 2>&1 | grep -E "Compiling|Finished"; then
        print_success "Build completed successfully"
        BINARY_PATH="$PWD/target/release/yolo-router"
    else
        print_error "Build failed"
        exit 1
    fi
}

# Install binary
install_binary() {
    print_header "Installing Binary"

    if [ ! -f "$BINARY_PATH" ]; then
        print_error "Binary not found at $BINARY_PATH"
        exit 1
    fi

    # Check if we need sudo
    if [ ! -w "$INSTALL_DIR" ]; then
        print_info "Installing to $INSTALL_DIR requires elevated privileges"
        if ! sudo cp "$BINARY_PATH" "$INSTALL_DIR/yolo-router"; then
            print_error "Failed to copy binary to $INSTALL_DIR"
            exit 1
        fi
        if ! sudo chmod +x "$INSTALL_DIR/yolo-router"; then
            print_error "Failed to make binary executable"
            exit 1
        fi
    else
        if ! cp "$BINARY_PATH" "$INSTALL_DIR/yolo-router"; then
            print_error "Failed to copy binary to $INSTALL_DIR"
            exit 1
        fi
        if ! chmod +x "$INSTALL_DIR/yolo-router"; then
            print_error "Failed to make binary executable"
            exit 1
        fi
    fi

    print_success "Binary installed to $INSTALL_DIR/yolo-router"
}

# Setup configuration directory
setup_config() {
    print_header "Setting Up Configuration"

    # Create config directory
    mkdir -p "$CONFIG_DIR"
    print_success "Created config directory: $CONFIG_DIR"

    # Copy example config if it doesn't exist
    if [ -f "config.example.toml" ] && [ ! -f "$CONFIG_DIR/config.toml" ]; then
        cp config.example.toml "$CONFIG_DIR/config.toml"
        print_success "Copied example configuration to $CONFIG_DIR/config.toml"
        print_warning "Please edit $CONFIG_DIR/config.toml with your API keys"
    fi

    # Create providers.json directory (for auth storage)
    mkdir -p "$CONFIG_DIR/providers"
    print_success "Created auth directory: $CONFIG_DIR/providers"
}

# Create systemd service (Linux only)
setup_systemd_service() {
    if [ "$OS" != "linux" ]; then
        return
    fi

    print_header "Setting Up Systemd Service (Optional)"

    local service_file="/etc/systemd/system/yolo-router.service"

    if [ -f "$service_file" ]; then
        print_warning "Systemd service already exists at $service_file"
        read -p "Do you want to update it? (y/n) " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            return
        fi
    fi

    read -p "Install systemd service? (y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        print_info "Skipping systemd service installation"
        return
    fi

    local service_content="[Unit]
Description=YoloRouter - AI Model Routing Proxy
After=network.target

[Service]
Type=simple
User=$USER
WorkingDirectory=$CONFIG_DIR
Environment=\"PATH=/usr/local/bin:/usr/bin\"
EnvironmentFile=-$CONFIG_DIR/.env
ExecStart=$INSTALL_DIR/yolo-router --config $CONFIG_DIR/config.toml
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
"

    if echo "$service_content" | sudo tee "$service_file" > /dev/null; then
        sudo systemctl daemon-reload
        print_success "Systemd service installed"
        echo ""
        echo "To start YoloRouter:"
        echo "  sudo systemctl start yolo-router"
        echo ""
        echo "To enable on boot:"
        echo "  sudo systemctl enable yolo-router"
        echo ""
        echo "To check status:"
        echo "  sudo systemctl status yolo-router"
    else
        print_error "Failed to create systemd service"
    fi
}

# Create launchd service (macOS only)
setup_launchd_service() {
    if [ "$OS" != "darwin" ]; then
        return
    fi

    print_header "Setting Up Launchd Service (Optional - macOS)"

    local plist_dir="$HOME/Library/LaunchAgents"
    local plist_file="$plist_dir/com.yoloprouter.daemon.plist"

    mkdir -p "$plist_dir"

    read -p "Install launchd service? (y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        print_info "Skipping launchd service installation"
        return
    fi

    local plist_content="<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">
<plist version=\"1.0\">
<dict>
    <key>Label</key>
    <string>com.yoloprouter.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>$INSTALL_DIR/yolo-router</string>
        <string>--config</string>
        <string>$CONFIG_DIR/config.toml</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>StandardOutPath</key>
    <string>$CONFIG_DIR/yolo-router.log</string>
    <key>StandardErrorPath</key>
    <string>$CONFIG_DIR/yolo-router-error.log</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
    </dict>
</dict>
</plist>
"

    if echo "$plist_content" > "$plist_file"; then
        print_success "Launchd service installed at $plist_file"
        echo ""
        echo "To load the service:"
        echo "  launchctl load $plist_file"
        echo ""
        echo "To unload the service:"
        echo "  launchctl unload $plist_file"
    else
        print_error "Failed to create launchd service"
    fi
}

# Print usage instructions
print_usage() {
    print_header "Installation Complete!"
    echo ""
    echo "YoloRouter has been successfully installed."
    echo ""
    echo -e "${BLUE}Quick Start:${NC}"
    echo ""
    echo "1. Configure your API keys:"
    echo "   nano $CONFIG_DIR/config.toml"
    echo ""
    echo "2. Set environment variables:"
    echo "   export ANTHROPIC_API_KEY=\"your-api-key\""
    echo "   export OPENAI_API_KEY=\"your-api-key\""
    echo ""
    echo "3. Start YoloRouter:"
    echo "   yolo-router --config $CONFIG_DIR/config.toml"
    echo ""
    echo "4. Test the server:"
    echo "   curl http://127.0.0.1:8989/health"
    echo ""
    echo -e "${BLUE}Interactive Configuration:${NC}"
    echo ""
    echo "   yolo-router --tui                    # TUI config editor"
    echo "   yolo-router --auth anthropic         # Authenticate provider"
    echo ""
    echo -e "${BLUE}API Endpoints:${NC}"
    echo ""
    echo "   POST   http://127.0.0.1:8989/v1/auto                # Auto-routing (15D analyzer)"
    echo "   POST   http://127.0.0.1:8989/v1/anthropic          # Anthropic endpoint"
    echo "   POST   http://127.0.0.1:8989/v1/openai             # OpenAI endpoint"
    echo "   GET    http://127.0.0.1:8989/health                # Health check"
    echo "   GET    http://127.0.0.1:8989/stats                 # Statistics"
    echo "   GET    http://127.0.0.1:8989/config                # Current config"
    echo ""
    echo -e "${BLUE}Documentation:${NC}"
    echo ""
    echo "   User Guide:         https://github.com/sternelee/YoloRouter/blob/main/USER_GUIDE.md"
    echo "   Project Summary:    https://github.com/sternelee/YoloRouter/blob/main/PROJECT_SUMMARY.md"
    echo "   Configuration:      $CONFIG_DIR/config.toml"
    echo ""
    echo -e "${BLUE}Uninstall:${NC}"
    echo ""
    echo "   rm -f $INSTALL_DIR/yolo-router"
    echo "   rm -rf $CONFIG_DIR"
    echo ""
}

# Cleanup
cleanup() {
    if [ -d "$temp_dir" ]; then
        print_info "Cleaning up temporary files..."
        rm -rf "$temp_dir"
    fi
}

trap cleanup EXIT

# Main installation flow
main() {
    print_header "YoloRouter Installation"
    echo "OS: $OS ($ARCH)"
    echo "Install Directory: $INSTALL_DIR"
    echo "Config Directory: $CONFIG_DIR"
    echo ""

    # Check if already installed
    if command -v yolo-router &> /dev/null; then
        print_warning "yolo-router is already installed at $(which yolo-router)"
        read -p "Do you want to reinstall? (y/n) " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 0
        fi
    fi

    detect_os
    check_prerequisites
    setup_repository
    build_from_source
    install_binary
    setup_config
    setup_systemd_service
    setup_launchd_service
    print_usage
}

# Run main
main "$@"
