# 🚀 YoloRouter 完整项目指南

欢迎！本文件为你快速导航整个 YoloRouter 项目。

## 📚 你在这里

**YoloRouter** 是一个智能 AI 模型路由代理，已完全配置好发布流程和安装脚本。

## 🎯 快速导航

### 👥 对于用户

1. **快速开始**: 阅读 `QUICK_INSTALL.txt`（5分钟）
2. **详细安装**: 阅读 `INSTALL.md`（15分钟）
3. **使用教程**: 阅读 `USER_GUIDE.md`（30分钟）

**一键安装**:
```bash
bash install.sh
```

### 🛠️ 对于开发者/维护者

1. **发布流程**: 阅读 `RELEASE_GUIDE.md`（20分钟）
2. **CI/CD 参考**: 阅读 `CI_CD_GUIDE.md`（10分钟）
3. **版本记录**: 查看 `CHANGELOG.md`

**发布新版本**:
```bash
git tag v0.2.0
git push origin v0.2.0
# GitHub Actions 自动处理其余部分！
```

### 🏗️ 对于架构师/技术负责人

1. **项目概览**: 阅读 `README.md`
2. **技术细节**: 阅读 `PROJECT_SUMMARY.md`
3. **CI/CD 设置**: 查看 `.github/workflows/`

## 📁 项目结构

```
YoloRouter/
├── 📦 安装脚本
│   ├── install.sh              ← 用户运行这个
│   ├── uninstall.sh            ← 卸载脚本
│   ├── yolo-setup.sh           ← 配置助手
│   └── QUICK_INSTALL.txt       ← 快速参考
│
├── 📖 安装文档
│   ├── INSTALL.md              ← 详细安装指南
│   ├── INSTALLATION.md         ← 包索引
│   ├── RELEASE.md              ← 发布信息
│   └── 00-START-HERE.md        ← 你在这里！
│
├── 🚀 CI/CD 工作流
│   ├── .github/workflows/
│   │   ├── release.yml         ← 发布自动化
│   │   ├── ci.yml              ← 持续集成
│   │   ├── build.yml           ← 开发构建
│   │   └── validate.yml        ← 工作流验证
│   ├── CI_CD_GUIDE.md          ← 快速参考
│   ├── RELEASE_GUIDE.md        ← 详细指南
│   └── CHANGELOG.md            ← 版本历史
│
├── 📚 项目文档
│   ├── README.md               ← 项目概览
│   ├── USER_GUIDE.md           ← 用户指南
│   ├── PROJECT_SUMMARY.md      ← 技术细节
│   └── AGENTS.md               ← 开发者参考
│
└── 💾 源代码
    ├── src/                    ← Rust 源代码
    ├── tests/                  ← 测试套件
    ├── Cargo.toml              ← 项目配置
    └── Cargo.lock              ← 依赖锁定
```

## 🎓 文档说明

| 文件 | 对象 | 时间 | 用途 |
|------|------|------|------|
| **QUICK_INSTALL.txt** | 终端用户 | 5分钟 | 快速安装 |
| **INSTALL.md** | 安装者 | 15分钟 | 详细步骤 |
| **USER_GUIDE.md** | 使用者 | 30分钟 | 功能说明 |
| **RELEASE_GUIDE.md** | 维护者 | 20分钟 | 如何发布 |
| **CI_CD_GUIDE.md** | 运维 | 10分钟 | 工作流参考 |
| **CHANGELOG.md** | 所有人 | 5分钟 | 版本历史 |
| **PROJECT_SUMMARY.md** | 架构师 | 30分钟 | 技术架构 |
| **README.md** | 所有人 | 10分钟 | 项目介绍 |

## 🚀 5分钟快速开始

### 第1步：安装

```bash
# 选项 A：直接运行
bash install.sh

# 选项 B：通过 curl 运行
curl -sSL https://raw.githubusercontent.com/sternelee/YoloRouter/main/install.sh | bash
```

### 第2步：配置 API 密钥

```bash
bash yolo-setup.sh env
# 按提示输入：
# - Anthropic API Key (可选)
# - OpenAI API Key (可选)
# - Gemini API Key (可选)
```

### 第3步：启动服务器

```bash
bash yolo-setup.sh start
```

### 第4步：测试

```bash
curl http://127.0.0.1:8989/health
# 应该返回: {"status":"ok"}
```

完成！🎉

## 📊 关键数字

- **安装脚本**: 3 个（852 行代码）
- **文档**: 10 个（~2,200 行）
- **CI/CD 工作流**: 4 个（625 行）
- **测试**: 54 个全部通过
- **平台支持**: Linux, macOS, Windows

## 🔍 常见问题

**Q: 安装需要多长时间？**  
A: 2-5 分钟（取决于 Rust 是否已安装）

**Q: 我可以使用哪些 AI 提供商？**  
A: Anthropic, OpenAI, Google Gemini, GitHub Copilot, 及 100+ OpenAI 兼容服务

**Q: 如何卸载？**  
A: `bash uninstall.sh`

**Q: 发布新版本需要多少步骤？**  
A: 2 步：`git tag v0.2.0` + `git push origin v0.2.0`（其余自动处理）

**Q: 我在安装过程中遇到了错误，怎么办？**  
A: 查看 `INSTALL.md` 中的故障排除部分（11 个常见问题的解决方案）

## 🎯 推荐路径

### 👨‍💻 我是开发者，想为项目贡献

1. 阅读 `README.md`（项目概览）
2. 阅读 `PROJECT_SUMMARY.md`（架构详解）
3. 阅读 `AGENTS.md`（开发者指南）
4. 查看源代码 `src/`
5. 运行测试: `cargo test --lib`

### 🚀 我想要发布新版本

1. 阅读 `RELEASE_GUIDE.md`（5 步流程）
2. 更新 `Cargo.toml` 版本号
3. 更新 `CHANGELOG.md`
4. `git tag v0.2.0`
5. `git push origin v0.2.0`
6. GitHub Actions 自动处理其余部分

### 📦 我想要安装 YoloRouter

1. 阅读 `QUICK_INSTALL.txt`（快速方式）
2. 运行 `bash install.sh`
3. 运行 `bash yolo-setup.sh env`
4. 运行 `bash yolo-setup.sh start`
5. 阅读 `USER_GUIDE.md` 学习如何使用

### 🔧 我是 DevOps/SRE，想要配置 CI/CD

1. 阅读 `CI_CD_GUIDE.md`（快速参考）
2. 查看 `.github/workflows/` 中的工作流
3. 根据需要自定义工作流
4. 检查 GitHub 仓库权限设置

## 📞 获取帮助

- **GitHub Issues**: https://github.com/sternelee/YoloRouter/issues
- **GitHub Discussions**: https://github.com/sternelee/YoloRouter/discussions
- **详细故障排除**: 查看 `INSTALL.md` 的故障排除部分

## ✅ 状态

- ✅ 所有 54 个测试通过
- ✅ 生产就绪
- ✅ 多平台支持
- ✅ 完整文档
- ✅ 自动化 CI/CD
- ✅ 安装脚本已验证

## 🎉 开始使用

```bash
# 现在就试试！
bash install.sh
```

或

```bash
curl -sSL https://raw.githubusercontent.com/sternelee/YoloRouter/main/install.sh | bash
```

---

**项目状态**: 🟢 生产就绪  
**最后更新**: 2026 年 4 月  
**版本**: 0.1.0

👉 **下一步**: 根据你的角色选择上面的推荐路径
