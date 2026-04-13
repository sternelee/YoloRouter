# YoloRouter CI/CD 快速参考

## 📋 工作流概览

### 发布工作流 (Release.yml)
**触发条件**: `git push vX.Y.Z`（标签）
**运行时间**: ~15-20 分钟
**输出**: GitHub Release + 多平台二进制文件

```
标签推送
  ↓
代码质量检查 (check)
  ├─ 运行测试
  ├─ 格式检查
  └─ Clippy 检查
  ↓
多平台编译 (build) - 仅在 check 通过时
  ├─ Linux x86_64
  ├─ macOS x86_64
  ├─ macOS ARM64
  └─ Windows x86_64
  ↓
创建 Release (release)
  ├─ 生成发布说明
  ├─ 上传二进制文件
  └─ 发布到 GitHub Releases
  ↓
发布文档 (publish-docs)
  └─ 部署到 GitHub Pages
```

### CI 工作流 (CI.yml)
**触发条件**: Push 到 main/develop，PR 提交
**运行时间**: ~10-15 分钟
**输出**: 检查结果

```
代码推送或 PR
  ↓
测试套件 (test) × 3 个平台
代码格式 (fmt)
Clippy 检查 (clippy)
安全审计 (security)
代码覆盖率 (coverage)
构建验证 (build) × 3 个平台
```

### 构建工作流 (Build.yml)
**触发条件**: Push 到 main/develop，PR 提交
**运行时间**: ~5-10 分钟
**输出**: 可下载的二进制工件

```
代码推送或 PR
  ↓
编译多平台二进制
  ├─ Linux x86_64
  ├─ macOS x86_64
  ├─ macOS ARM64
  └─ Windows x86_64
  ↓
上传工件 (保留 30 天)
```

## 🚀 发布新版本步骤

### 1️⃣ 更新代码

```bash
# 更新版本号
# 编辑 Cargo.toml: version = "0.2.0"

# 更新变更日志
# 编辑 CHANGELOG.md，添加新版本条目

# 提交更改
git add Cargo.toml CHANGELOG.md
git commit -m "Release v0.2.0"
git push origin main
```

### 2️⃣ 创建发布标签

```bash
# 本地创建标签
git tag v0.2.0

# 推送标签到 GitHub
git push origin v0.2.0
```

或通过 GitHub 网页：
1. https://github.com/sternelee/YoloRouter/releases/new
2. 输入标签: `v0.2.0`
3. 点击 "Create new tag"

### 3️⃣ 等待工作流完成

监控: https://github.com/sternelee/YoloRouter/actions

工作流完成后，GitHub Release 页面会自动更新所有二进制文件。

### 4️⃣ 验证发布

```bash
# 检查 GitHub Releases 页面
# https://github.com/sternelee/YoloRouter/releases

# 验证下载
# 下载每个二进制文件并检查 SHA256 校验和

# 测试安装
bash install.sh --version
```

### 5️⃣ 发布通知

- 在 GitHub Discussions 发布公告
- 分享到社交媒体
- 更新项目网站

## 📦 发布的文件

每个发布包含：

```
✓ yolo-router-linux-amd64         (Linux 64-bit)
✓ yolo-router-linux-amd64.sha256  (校验和)
✓ yolo-router-darwin-amd64        (macOS Intel)
✓ yolo-router-darwin-amd64.sha256
✓ yolo-router-darwin-arm64        (macOS Apple Silicon)
✓ yolo-router-darwin-arm64.sha256
✓ yolo-router-windows-amd64.exe   (Windows 64-bit)
✓ yolo-router-windows-amd64.sha256
✓ CHECKSUMS.sha256                (所有校验和汇总)
```

## 🔍 检查工作流状态

### GitHub Actions 页面
https://github.com/sternelee/YoloRouter/actions

### 特定工作流
- **Release**: https://github.com/sternelee/YoloRouter/actions/workflows/release.yml
- **CI**: https://github.com/sternelee/YoloRouter/actions/workflows/ci.yml
- **Build**: https://github.com/sternelee/YoloRouter/actions/workflows/build.yml

## 🐛 故障排除

### 测试失败

```bash
# 本地重现
cargo test --lib --release

# 查看详细信息
cargo test --lib --release -- --nocapture

# 修复并推送
git add .
git commit -m "Fix failing tests"
git push origin main
```

### 构建失败

```bash
# 本地构建
cargo build --release

# 查看错误
cargo build --release --verbose

# 清理并重试
cargo clean
cargo build --release
```

### 格式问题

```bash
# 自动修复格式
cargo fmt

# 检查 clippy 问题
cargo clippy --release -- -D warnings

# 推送修复
git add .
git commit -m "Fix code format and clippy warnings"
git push origin main
```

## ✅ 发布前检查清单

- [ ] 所有测试通过: `cargo test --lib --release`
- [ ] 代码格式正确: `cargo fmt --check`
- [ ] 无 clippy 警告: `cargo clippy --release -- -D warnings`
- [ ] CHANGELOG.md 已更新
- [ ] Cargo.toml 版本已更新
- [ ] 安装脚本可用: `bash install.sh`
- [ ] 二进制可用: `./target/release/yolo-router --version`
- [ ] 文档已更新
- [ ] 提交信息清晰

## 📊 工作流详解

### Release.yml 详解

#### check 作业
```yaml
- 运行所有单元测试
- 检查代码格式 (cargo fmt)
- 运行 clippy 检查
- 如果任何检查失败，工作流停止
```

#### build 作业
```yaml
- 为每个平台编译发布二进制
- 剥离二进制文件 (Linux/macOS)
- 生成 SHA256 校验和
- 上传工件
- 仅在 check 通过时运行
```

#### release 作业
```yaml
- 下载所有构建的工件
- 生成发布说明
- 创建 GitHub Release
- 上传二进制文件
- 仅在 build 通过时运行
```

#### publish-docs 作业
```yaml
- 构建文档
- 部署到 GitHub Pages
- 仅在 release 通过时运行
```

### CI.yml 详解

#### test 作业
```yaml
- 在 Ubuntu、macOS、Windows 上运行完整测试
- 运行库测试、文档测试、集成测试
- 缓存 cargo 注册表和索引
```

#### fmt 作业
```yaml
- 检查所有代码格式是否符合 rustfmt 标准
- 如果有格式问题则失败
```

#### clippy 作业
```yaml
- 运行高级 Rust linter
- 检查代码质量和潜在错误
```

#### security 作业
```yaml
- 运行 cargo-audit 检查已知安全漏洞
- 审查依赖项
```

#### coverage 作业
```yaml
- 运行 cargo-tarpaulin 生成代码覆盖率
- 上传到 Codecov
```

#### build 作业
```yaml
- 验证调试版本构建
- 验证发布版本构建
- 在多个平台上构建
```

## 🔄 回滚发布

如果需要撤销发布：

```bash
# 删除本地标签
git tag -d v0.2.0

# 删除远程标签
git push origin --delete v0.2.0

# 删除 GitHub Release (网页 UI)
# https://github.com/sternelee/YoloRouter/releases

# 重新创建
git tag v0.2.0
git push origin v0.2.0
```

## 📈 监控构建

### 构建统计
```bash
# 查看所有工作流运行
# https://github.com/sternelee/YoloRouter/actions

# 查看特定发布的日志
# https://github.com/sternelee/YoloRouter/actions/workflows/release.yml
```

### 失败通知
- GitHub 自动发送电子邮件通知
- 检查 Actions 选项卡获取详细信息

## 🎯 最佳实践

1. **频繁提交**: 每个功能一次提交
2. **清晰的提交信息**: `feat: 添加新功能` 或 `fix: 修复 #123`
3. **完整测试**: 在推送前本地运行测试
4. **文档同步**: 每个功能都更新 CHANGELOG.md
5. **语义版本**: 遵循 Semantic Versioning
6. **发布前检查**: 使用发布前检查清单

## 📞 获取帮助

- **GitHub Issues**: https://github.com/sternelee/YoloRouter/issues
- **GitHub Discussions**: https://github.com/sternelee/YoloRouter/discussions
- **本文件**: 查看更详细的说明

---

**版本**: 1.0  
**最后更新**: 2026 年 4 月  
**维护者**: YoloRouter 团队
