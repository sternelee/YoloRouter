#!/bin/bash

# YoloRouter Uninstall Script
# Removes YoloRouter and optionally its configuration

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
CONFIG_DIR="${CONFIG_DIR:-$HOME/.config/yolo-router}"

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

# Main uninstall flow
main() {
    print_header "YoloRouter Uninstallation"

    # Check if installed
    if [ ! -f "$INSTALL_DIR/yolo-router" ]; then
        print_warning "YoloRouter binary not found at $INSTALL_DIR/yolo-router"
        echo "It may have already been uninstalled."
        exit 0
    fi

    print_info "This will uninstall YoloRouter"
    echo ""
    echo "Installed at: $INSTALL_DIR/yolo-router"
    echo "Config at:    $CONFIG_DIR"
    echo ""

    # Confirmation
    read -p "Do you want to continue? (y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        print_info "Uninstallation cancelled"
        exit 0
    fi

    # Stop systemd service if running
    if systemctl is-active --quiet yolo-router 2>/dev/null; then
        print_info "Stopping yolo-router service..."
        if sudo systemctl stop yolo-router 2>/dev/null; then
            print_success "Service stopped"
        fi
    fi

    # Disable systemd service
    if [ -f "/etc/systemd/system/yolo-router.service" ]; then
        read -p "Remove systemd service? (y/n) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            if sudo rm -f "/etc/systemd/system/yolo-router.service"; then
                sudo systemctl daemon-reload
                print_success "Systemd service removed"
            fi
        fi
    fi

    # Unload launchd service (macOS)
    if [ -f "$HOME/Library/LaunchAgents/com.yoloprouter.daemon.plist" ]; then
        read -p "Unload launchd service? (y/n) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            if launchctl unload "$HOME/Library/LaunchAgents/com.yoloprouter.daemon.plist" 2>/dev/null; then
                rm -f "$HOME/Library/LaunchAgents/com.yoloprouter.daemon.plist"
                print_success "Launchd service unloaded and removed"
            fi
        fi
    fi

    # Remove binary
    if [ -w "$INSTALL_DIR" ]; then
        rm -f "$INSTALL_DIR/yolo-router"
    else
        sudo rm -f "$INSTALL_DIR/yolo-router"
    fi
    print_success "Binary removed from $INSTALL_DIR/yolo-router"

    # Remove configuration
    read -p "Remove configuration directory? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        if [ -d "$CONFIG_DIR" ]; then
            rm -rf "$CONFIG_DIR"
            print_success "Configuration directory removed"
        fi
    else
        print_warning "Configuration preserved at $CONFIG_DIR"
    fi

    print_header "Uninstallation Complete"
    print_success "YoloRouter has been uninstalled"
}

# Run main
main "$@"
