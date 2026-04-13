# YoloRouter Installation Guide

Welcome to YoloRouter! This guide will help you install and set up the intelligent AI model routing proxy.

## 📋 Table of Contents

- [System Requirements](#system-requirements)
- [Quick Installation](#quick-installation)
- [Detailed Installation](#detailed-installation)
- [Configuration](#configuration)
- [Running YoloRouter](#running-yoloprouter)
- [Verification](#verification)
- [Troubleshooting](#troubleshooting)
- [Uninstallation](#uninstallation)

## 🔧 System Requirements

### Minimum

- **OS**: macOS, Linux, or other Unix-like systems
- **Rust**: 1.70 or later
- **Cargo**: Latest stable version
- **RAM**: 256 MB minimum
- **Disk**: 50 MB for binary + 10 MB for configuration

### Recommended

- **Rust**: 1.80+
- **RAM**: 512 MB or more
- **Disk**: SSD recommended for faster startup

### Install Rust (if not already installed)

```bash
# macOS / Linux
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Then follow the on-screen instructions to add Cargo to your PATH
source $HOME/.cargo/env
```

## 🚀 Quick Installation

### Option 1: One-Command Installation (Recommended)

```bash
# Clone and install in one command
git clone https://github.com/sternelee/YoloRouter.git
cd YoloRouter
bash install.sh
```

### Option 2: Using curl

```bash
curl -sSL https://raw.githubusercontent.com/sternelee/YoloRouter/main/install.sh | bash
```

That's it! The script will:
1. ✅ Check prerequisites (Git, Rust)
2. ✅ Clone the repository
3. ✅ Build the release binary
4. ✅ Install to `/usr/local/bin/yolo-router`
5. ✅ Create config directory at `~/.config/yolo-router`
6. ✅ Optionally set up systemd (Linux) or launchd (macOS) service

## 🔨 Detailed Installation

### Step 1: Clone Repository

```bash
git clone https://github.com/sternelee/YoloRouter.git
cd YoloRouter
```

### Step 2: Build from Source

```bash
# Release build (optimized, ~10-20 seconds)
cargo build --release

# Or debug build (faster compilation, but slower runtime)
cargo build

# Binary location:
# target/release/yolo-router (release)
# target/debug/yolo-router   (debug)
```

### Step 3: Install Binary

```bash
# Copy to system directory
sudo cp target/release/yolo-router /usr/local/bin/

# Or use the install script
bash install.sh
```

### Step 4: Verify Installation

```bash
yolo-router --version  # Should print version info
which yolo-router      # Should show /usr/local/bin/yolo-router
```

## ⚙️ Configuration

### Create Configuration Directory

```bash
mkdir -p ~/.config/yolo-router
cd ~/.config/yolo-router
```

### Copy Example Configuration

```bash
# From the cloned repository
cp /path/to/YoloRouter/config.example.toml ~/.config/yolo-router/config.toml

# Or create manually
nano ~/.config/yolo-router/config.toml
```

### Basic Configuration Template

```toml
[daemon]
port = 8989
log_level = "info"

# Anthropic Claude
[providers.anthropic]
type = "anthropic"
api_key = "${ANTHROPIC_API_KEY}"

# OpenAI
[providers.openai]
type = "openai"
api_key = "${OPENAI_API_KEY}"

# Google Gemini
[providers.gemini]
type = "gemini"
api_key = "${GEMINI_API_KEY}"

# Default scenario
[scenarios.default]
models = [
  { provider = "anthropic", model = "claude-opus", cost_tier = "high" },
  { provider = "openai", model = "gpt-4", cost_tier = "high" },
  { provider = "anthropic", model = "claude-sonnet", cost_tier = "medium" }
]
is_default = true

# Routing configuration
[routing]
fallback_enabled = true
timeout_ms = 30000
retry_count = 2
```

### Set Environment Variables

```bash
# Add to your shell profile (~/.bashrc, ~/.zshrc, ~/.fish, etc.)

# Anthropic
export ANTHROPIC_API_KEY="sk-ant-..."

# OpenAI
export OPENAI_API_KEY="sk-..."

# Google Gemini
export GEMINI_API_KEY="..."

# Reload shell
source ~/.bashrc  # or ~/.zshrc, etc.
```

**Get API Keys:**
- **Anthropic**: https://console.anthropic.com
- **OpenAI**: https://platform.openai.com
- **Google Gemini**: https://makersuite.google.com/app/apikey

### Interactive Configuration Setup

Use the helper script:

```bash
bash yolo-setup.sh env    # Set environment variables
bash yolo-setup.sh edit   # Edit configuration file
```

## 🏃 Running YoloRouter

### Option 1: Manual Start (Development)

```bash
yolo-router --config ~/.config/yolo-router/config.toml

# Or set environment variable
export YOLO_CONFIG=~/.config/yolo-router/config.toml
yolo-router
```

### Option 2: Using Setup Script (Recommended)

```bash
bash yolo-setup.sh start
```

### Option 3: Systemd Service (Linux)

If you installed the systemd service:

```bash
# Start
sudo systemctl start yolo-router

# Enable on boot
sudo systemctl enable yolo-router

# Check status
sudo systemctl status yolo-router

# View logs
sudo journalctl -u yolo-router -f
```

### Option 4: Launchd Service (macOS)

If you installed the launchd service:

```bash
# Load
launchctl load ~/Library/LaunchAgents/com.yoloprouter.daemon.plist

# Unload
launchctl unload ~/Library/LaunchAgents/com.yoloprouter.daemon.plist

# View logs
log stream --predicate 'process == "yolo-router"'
```

## ✅ Verification

### 1. Health Check

```bash
curl http://127.0.0.1:8989/health
# Expected: {"status":"ok"}
```

### 2. Check Configuration

```bash
curl http://127.0.0.1:8989/config | jq .
```

### 3. View Statistics

```bash
curl http://127.0.0.1:8989/stats | jq .
```

### 4. Test API Endpoint

```bash
curl -X POST http://127.0.0.1:8989/v1/auto \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-opus",
    "messages": [{"role": "user", "content": "Hello!"}],
    "max_tokens": 100
  }' | jq .
```

### 5. Using Setup Script

```bash
bash yolo-setup.sh test   # Test all endpoints
bash yolo-setup.sh api    # Test with sample request
```

## 🐛 Troubleshooting

### Issue: "Command not found: yolo-router"

**Solution**: Ensure the binary is in your PATH

```bash
# Check installation
which yolo-router

# If not found, manually add to PATH
export PATH="/usr/local/bin:$PATH"

# Or install manually
sudo cp target/release/yolo-router /usr/local/bin/
chmod +x /usr/local/bin/yolo-router
```

### Issue: "Connection refused" when accessing API

**Solution**: Start the server first

```bash
# In one terminal
yolo-router --config ~/.config/yolo-router/config.toml

# In another terminal
curl http://127.0.0.1:8989/health
```

### Issue: "Port 8989 already in use"

**Solution**: Change the port in config

```toml
[daemon]
port = 8990  # Change to different port
```

### Issue: "API key not found" or "Unauthorized"

**Solution**: Verify environment variables

```bash
# Check if variables are set
echo $ANTHROPIC_API_KEY
echo $OPENAI_API_KEY

# If empty, set them
export ANTHROPIC_API_KEY="your-key-here"

# Add to shell profile to make persistent
nano ~/.bashrc
```

### Issue: "Configuration not found"

**Solution**: Create or specify config file

```bash
# Create default config
mkdir -p ~/.config/yolo-router
cp config.example.toml ~/.config/yolo-router/config.toml

# Or specify with flag
yolo-router --config /path/to/config.toml

# Or with environment variable
export YOLO_CONFIG=/path/to/config.toml
yolo-router
```

### Issue: Build fails with "Rust version too old"

**Solution**: Update Rust

```bash
rustup update
rustc --version  # Should be 1.70+
```

### Issue: "jq: command not found" in tests

**Solution**: Install jq (optional, for JSON parsing)

```bash
# macOS
brew install jq

# Ubuntu/Debian
sudo apt-get install jq

# Or use without jq
curl http://127.0.0.1:8989/stats
```

## 🗑️ Uninstallation

### Using Uninstall Script

```bash
bash uninstall.sh

# Or
/usr/local/bin/yolo-router uninstall
```

### Manual Uninstallation

```bash
# Remove binary
sudo rm /usr/local/bin/yolo-router

# Remove systemd service (Linux)
sudo rm /etc/systemd/system/yolo-router.service
sudo systemctl daemon-reload

# Remove launchd service (macOS)
launchctl unload ~/Library/LaunchAgents/com.yoloprouter.daemon.plist
rm ~/Library/LaunchAgents/com.yoloprouter.daemon.plist

# Remove configuration (optional)
rm -rf ~/.config/yolo-router
```

## 📚 Next Steps

1. **Configure Providers**: Add your API keys to `~/.config/yolo-router/config.toml`
2. **Start Server**: Run `yolo-router --config ~/.config/yolo-router/config.toml`
3. **Test Integration**: Visit http://127.0.0.1:8989/stats
4. **Read Documentation**: Check `USER_GUIDE.md` for detailed usage
5. **Set Up IDE Integration**: See `CLAUDE_CODE_SETUP.md` for Claude Code integration

## 🆘 Getting Help

- **GitHub Issues**: https://github.com/sternelee/YoloRouter/issues
- **Documentation**: 
  - [USER_GUIDE.md](USER_GUIDE.md) - Complete user guide
  - [PROJECT_SUMMARY.md](PROJECT_SUMMARY.md) - Architecture details
  - [QUICK_START.md](QUICK_START.md) - Quick reference

## 📝 Summary

```bash
# TL;DR - Complete installation
git clone https://github.com/sternelee/YoloRouter.git
cd YoloRouter
bash install.sh

# Set API keys
export ANTHROPIC_API_KEY="your-key"

# Start server
yolo-router --config ~/.config/yolo-router/config.toml

# Test
curl http://127.0.0.1:8989/health
```

---

**Version**: 0.1.0  
**Last Updated**: April 2026  
**Status**: ✅ Production Ready
