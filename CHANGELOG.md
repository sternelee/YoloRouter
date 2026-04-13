# YoloRouter Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-04-13

### Added
- ✨ Initial release of YoloRouter
- 🔀 Multi-provider AI model routing (Anthropic, OpenAI, Gemini, GitHub Copilot, Codex, OpenAI-compatible services)
- 🛡️ Automatic fallback chain support for high availability
- ⚙️ Flexible TOML-based configuration with environment variable expansion
- 🧠 15-dimensional FastAnalyzer for intelligent model selection
- 📊 Real-time statistics and monitoring endpoints
- 🎯 Scenario-based routing configuration
- 💰 Cost optimization through intelligent model selection
- 🔐 Environment variable-based API key management
- 📁 Configuration directory at `~/.config/yolo-router`
- 🔧 Interactive TUI for provider authentication
- 🚀 HTTP REST API with multiple endpoint adaptations
- 📈 Performance monitoring (< 1ms analyzer, 1-3s request latency)
- ✅ Comprehensive test suite (54/54 tests passing)

### Documentation
- 📖 Complete user guide (USER_GUIDE.md)
- 📋 Installation guide (INSTALL.md)
- 📚 Project summary and architecture (PROJECT_SUMMARY.md)
- 🎯 Quick start guide (QUICK_START.md)
- 💾 Installation scripts (install.sh, uninstall.sh, yolo-setup.sh)
- 🔄 CI/CD workflow templates

### Features
- Support for 5+ AI providers out of the box
- 100+ models via OpenAI-compatible APIs (OpenRouter, Groq, DeepSeek, etc.)
- Automatic provider selection based on request analysis
- Health check endpoints
- Configuration validation
- Error recovery and fallback mechanisms
- Service management (systemd/launchd)

### Testing
- Unit tests for all modules
- Integration tests for full workflows
- Configuration parsing tests
- Provider factory tests
- Routing engine tests
- Analyzer tests

### Infrastructure
- GitHub Actions CI/CD workflow
- Automated release process
- Multi-platform binary builds (Linux, macOS, Windows)
- Checksum generation for binaries

## Future Releases

### [0.2.0] (Planned)
- [ ] Configuration hot-reload without restart
- [ ] Prometheus metrics export
- [ ] Kubernetes deployment manifests
- [ ] Docker image builds and publishing
- [ ] Package manager support (Homebrew, AUR, apt)
- [ ] Database persistence for statistics
- [ ] Advanced routing policies
- [ ] Request caching layer
- [ ] Rate limiting per provider
- [ ] Cost tracking and reporting

### [0.3.0] (Planned)
- [ ] Web UI dashboard
- [ ] Advanced analytics
- [ ] Custom routing plugins
- [ ] GraphQL API support
- [ ] Streaming response support
- [ ] Batch request processing
- [ ] Request/response transformations

## Notes

### Version 0.1.0 Release

This is the initial release of YoloRouter, featuring:
- ✅ Production-ready codebase
- ✅ Comprehensive documentation
- ✅ Full test coverage
- ✅ Multi-platform support
- ✅ Professional installation scripts
- ✅ CI/CD automation

The release is stable and ready for production deployment.

### Installation

```bash
# From GitHub release
bash install.sh

# Or with curl
curl -sSL https://raw.githubusercontent.com/sternelee/YoloRouter/main/install.sh | bash
```

### Breaking Changes
None (initial release)

### Deprecations
None (initial release)

### Security
- Audit of dependencies recommended
- No known security issues
- Regular dependency updates recommended

---

**Format inspired by**: [Keep a Changelog](https://keepachangelog.com/)
**Version format**: [Semantic Versioning](https://semver.org/)
