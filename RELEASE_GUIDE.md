# YoloRouter Release Guide

This document describes how to release a new version of YoloRouter.

## Release Process

### 1. Prepare the Release

#### Update Version Number

Edit `Cargo.toml` and update the version:

```toml
[package]
name = "yolo-router"
version = "0.2.0"  # Update this
```

#### Update CHANGELOG

Add your changes to `CHANGELOG.md`:

```markdown
## [0.2.0] - 2026-04-20

### Added
- New feature 1
- New feature 2

### Fixed
- Bug fix 1
- Bug fix 2

### Changed
- Breaking change 1
```

#### Run Pre-release Checks

```bash
# Run all tests
cargo test --lib --release

# Check format
cargo fmt --check

# Run clippy
cargo clippy --all-targets --release -- -D warnings

# Build release binary
cargo build --release

# Verify installation works
bash install.sh --help
```

### 2. Create GitHub Release

The release process is **fully automated** via GitHub Actions.

#### Step 1: Create a Git Tag

```bash
# Create and push tag
git tag v0.2.0
git push origin v0.2.0
```

Or create the tag through GitHub UI:
- Go to: https://github.com/sternelee/YoloRouter/releases/new
- Enter tag: `v0.2.0`
- Click "Create new tag"

#### Step 2: GitHub Actions Runs Automatically

When you push a tag starting with `v` (e.g., `v0.2.0`), the Release workflow automatically:

1. **Code Quality Check** (`check` job)
   - Runs all tests
   - Checks code format
   - Runs clippy linter
   - If any check fails, the workflow stops

2. **Build Binaries** (`build` job) - only if check passes
   - Builds for Linux x86_64
   - Builds for macOS x86_64
   - Builds for macOS ARM64
   - Builds for Windows x86_64
   - Generates SHA256 checksums for each binary

3. **Create Release** (`release` job) - only if build succeeds
   - Downloads all built artifacts
   - Generates comprehensive release notes
   - Creates GitHub Release with all binaries
   - Includes SHA256 checksums
   - Publishes to GitHub Releases page

4. **Publish Documentation** (`publish-docs` job)
   - Builds documentation site
   - Deploys to GitHub Pages

### 3. Verify Release

After the GitHub Actions workflow completes:

```bash
# 1. Check GitHub Releases page
# https://github.com/sternelee/YoloRouter/releases

# 2. Verify downloads work
# Download each binary and verify checksums

# 3. Test installation from release
curl -sSL https://github.com/sternelee/YoloRouter/releases/download/v0.2.0/install.sh | bash

# 4. Verify binary works
yolo-router --version
```

### 4. Publish Announcement

Once verified:

1. **Create Release Announcement**
   - Share on GitHub Discussions
   - Post to social media
   - Update project website
   - Notify users

2. **Example Announcement**:
   ```
   🚀 YoloRouter v0.2.0 is now available!
   
   Key improvements:
   • Configuration hot-reload
   • Prometheus metrics export
   • Performance improvements
   
   Install: bash install.sh
   Docs: https://github.com/sternelee/YoloRouter
   ```

## Workflow Files

### `.github/workflows/release.yml`
- **Triggered by**: `git push vX.Y.Z` (tags)
- **Jobs**:
  - `check`: Code quality validation
  - `build`: Multi-platform binary builds
  - `release`: GitHub Release creation
  - `publish-docs`: Documentation deployment
- **Duration**: ~10-20 minutes
- **Outputs**: GitHub Release with binaries and checksums

### `.github/workflows/ci.yml`
- **Triggered by**: Push to main/develop, Pull Requests
- **Jobs**:
  - `test`: Run test suite on multiple OSes
  - `fmt`: Check code formatting
  - `clippy`: Lint checks
  - `security`: Security audit
  - `coverage`: Code coverage report
  - `build`: Build verification
- **Duration**: ~5-10 minutes
- **Outputs**: Workflow results on PR/commit

### `.github/workflows/build.yml`
- **Triggered by**: Push to main/develop, Pull Requests
- **Jobs**: Builds binaries on each commit (for testing purposes)
- **Duration**: ~5-10 minutes
- **Outputs**: Artifact downloads in Actions tab

## Release Checklist

Before releasing, ensure:

- [ ] All tests pass locally: `cargo test --lib --release`
- [ ] Code format is correct: `cargo fmt --check`
- [ ] No clippy warnings: `cargo clippy --release -- -D warnings`
- [ ] CHANGELOG.md is updated
- [ ] Cargo.toml version is updated
- [ ] Version bump is committed to main
- [ ] All commit messages are clear and meaningful
- [ ] Documentation is up to date
- [ ] Installation scripts work: `bash install.sh`
- [ ] Binary works: `yolo-router --version`

## Troubleshooting Release Issues

### Build Fails

Check the GitHub Actions log:
1. Go to: https://github.com/sternelee/YoloRouter/actions
2. Click the failed release workflow
3. Expand the failing job
4. Look for error messages
5. Common issues:
   - Test failures: Fix and push new commit
   - Format issues: Run `cargo fmt` and push
   - Clippy warnings: Fix and push

### Release Already Published

To fix/re-release:

```bash
# Delete the tag locally and remotely
git tag -d v0.2.0
git push origin --delete v0.2.0

# Delete the GitHub Release (via UI)
# https://github.com/sternelee/YoloRouter/releases

# Create the tag again
git tag v0.2.0
git push origin v0.2.0
```

### Binary Download Issues

1. Check GitHub Actions logs for build errors
2. Verify binary was uploaded as artifact
3. Check file permissions and naming
4. Manually upload via GitHub UI if needed

## Automating Releases

### Option 1: Automated Version Bumping

Create a script `scripts/release.sh`:

```bash
#!/bin/bash
set -e

# Get version
VERSION=$1
if [ -z "$VERSION" ]; then
  echo "Usage: ./scripts/release.sh <version>"
  exit 1
fi

# Update Cargo.toml
sed -i "s/version = \".*\"/version = \"$VERSION\"/" Cargo.toml

# Update CHANGELOG (you need to add entry manually first)

# Commit
git add Cargo.toml CHANGELOG.md
git commit -m "Release v$VERSION"

# Tag and push
git tag "v$VERSION"
git push origin main
git push origin "v$VERSION"

echo "Release v$VERSION initiated!"
```

Usage:
```bash
chmod +x scripts/release.sh
./scripts/release.sh 0.2.0
```

### Option 2: Release Labels

Use GitHub labels to manage release:
1. Label PR with `release-patch`, `release-minor`, or `release-major`
2. Set up automation to bump version and create tag automatically

## Release Versioning

YoloRouter follows [Semantic Versioning](https://semver.org/):

- **MAJOR** (0→1): Breaking changes, API changes
- **MINOR** (0.1→0.2): New features, backward compatible
- **PATCH** (0.1.0→0.1.1): Bug fixes, backward compatible

Examples:
- Feature release: `0.1.0` → `0.2.0` (minor bump)
- Bug fix: `0.1.0` → `0.1.1` (patch bump)
- Breaking change: `0.1.0` → `1.0.0` (major bump)

## Release Notes Template

The CI automatically generates release notes, but you can customize by editing the release on GitHub:

```markdown
## 🎉 YoloRouter v0.2.0

### ✨ What's New

- Feature 1 description
- Feature 2 description
- Feature 3 description

### 🐛 Bug Fixes

- Fixed issue #123
- Fixed issue #456

### 📊 Performance

- Improved analyzer speed by 30%
- Reduced memory usage by 15%

### 📚 Documentation

- Updated user guide
- Added configuration examples

### 🙏 Thanks

Special thanks to:
- @contributor1
- @contributor2

### 📥 Installation

```bash
bash install.sh
```

or

```bash
curl -sSL https://raw.githubusercontent.com/sternelee/YoloRouter/main/install.sh | bash
```

### 📋 Full Changelog

See [CHANGELOG.md](CHANGELOG.md) for the full list of changes.

### 🔗 Links

- [Installation Guide](INSTALL.md)
- [User Guide](USER_GUIDE.md)
- [GitHub Issues](https://github.com/sternelee/YoloRouter/issues)
```

## GitHub Pages Documentation

Documentation is automatically published to GitHub Pages on each release:

- **Build from**: Root documentation files (README.md, INSTALL.md, etc.)
- **Deploy to**: `https://sternelee.github.io/YoloRouter/`
- **Trigger**: Automatic on `release` workflow completion

To enable/configure:
1. Go to: Settings → Pages
2. Set source to "GitHub Actions"
3. Done!

## Status Badges

Add these to README.md to show CI status:

```markdown
![CI Status](https://github.com/sternelee/YoloRouter/workflows/CI/badge.svg)
![Build Status](https://github.com/sternelee/YoloRouter/workflows/Build/badge.svg)
![Release Status](https://github.com/sternelee/YoloRouter/workflows/Release/badge.svg)
```

## Contact & Support

For release-related questions:
- GitHub Issues: https://github.com/sternelee/YoloRouter/issues
- GitHub Discussions: https://github.com/sternelee/YoloRouter/discussions

---

**Last Updated**: April 2026
**Release Strategy**: Semantic Versioning + GitHub Actions Automation
