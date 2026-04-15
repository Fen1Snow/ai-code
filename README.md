# AI-Code 🦞

[![Rust](https://img.shields.io/badge/Rust-1.75+-orange?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Version](https://img.shields.io/badge/Version-0.3.0-green.svg)](rust/CHANGELOG.md)

**AI-Code** 是一个基于 [Claw-Code](https://github.com/instructkr/claw-code) 开发的 AI 编程助手，专为软件开发者设计。支持多种 AI API 提供商，提供强大的代码理解、编辑和执行能力。

> 🦞 **AI-Code** - Your AI-Powered Software Engineering Assistant

---

## ✨ 特性

- 🔧 **多 API 支持** - OpenAI、Anthropic、Ollama 等多种 API 提供商
- 📁 **文件操作** - 读取、写入、编辑文件，支持搜索和替换
- 🔍 **代码搜索** - Glob 模式搜索、正则表达式搜索
- 🌐 **网络能力** - 网页搜索、网页内容获取
- 🖥️ **命令执行** - 执行 Shell 命令、PowerShell 命令
- 🤖 **子代理** - 启动专门的子代理任务
- 📓 **Jupyter 支持** - 编辑 Jupyter notebooks
- 🔌 **插件系统** - 支持自定义插件和钩子

---

## 🚀 快速开始

### 1. 环境要求

- **Rust** 1.75+ (用于编译)
- **Linux** / **macOS** / **Windows**

### 2. 编译项目

```bash
cd rust
cargo build --release
```

编译后的二进制文件位于: `rust/target/release/claw`

### 3. 配置 API

设置环境变量:

```bash
# OpenAI API
export OPENAI_API_KEY="your-api-key"
export OPENAI_BASE_URL="https://api.openai.com/v1"  # 可选
export OPENAI_MODEL="gpt-4o"  # 可选

# 或 Anthropic API
export ANTHROPIC_API_KEY="your-api-key"
export ANTHROPIC_MODEL="claude-opus-4-6"  # 可选
```

### 4. 运行

**使用启动脚本 (Linux):**

```bash
cd rust
chmod +x run_claw.sh
./run_claw.sh
```

**直接运行二进制:**

```bash
cd rust
./target/release/claw
```

**单次提问模式:**

```bash
./run_claw.sh "请帮我分析这个项目的结构"
```

---

## 📖 使用指南

### 交互式 REPL 命令

| 命令 | 说明 |
|------|------|
| `/help` | 显示帮助信息 |
| `/status` | 显示会话状态 |
| `/model [name]` | 查看或切换模型 |
| `/permissions [mode]` | 查看或切换权限模式 |
| `/clear --confirm` | 清除当前会话 |
| `/cost` | 显示 token 使用量 |
| `/config [section]` | 查看配置 |
| `/memory` | 查看加载的指令文件 |
| `/diff` | 显示 git diff |
| `/version` | 显示版本信息 |
| `/exit` | 退出 REPL |

### 权限模式

- `read-only` - 只读模式，只能读取和搜索
- `workspace-write` - 工作区写入模式，可以编辑文件
- `danger-full-access` - 完全访问模式，无限制 (默认)

### CLI 参数

```bash
claw [OPTIONS] [PROMPT]

OPTIONS:
  --model MODEL              指定模型
  --output-format FORMAT     输出格式 (text/json)
  --permission-mode MODE     权限模式
  --allowedTools TOOLS       限制可用工具
  --version, -V              显示版本
  --help                     显示帮助

EXAMPLES:
  claw                              # 启动交互式 REPL
  claw "分析这个项目"                # 单次提问
  claw --model opus "解释代码"       # 使用指定模型
  claw --output-format json "问题"   # JSON 输出
```

---

## 🛠️ 可用工具

| 工具 | 说明 |
|------|------|
| `bash` | 执行 Shell 命令 |
| `read_file` | 读取文件内容 |
| `write_file` | 写入文件 |
| `edit_file` | 编辑文件 (搜索/替换) |
| `glob_search` | Glob 模式搜索文件 |
| `grep_search` | 正则表达式搜索内容 |
| `list_directory` | 列出目录内容 |
| `WebFetch` | 获取网页内容 |
| `WebSearch` | 网页搜索 |
| `TodoWrite` | 任务列表管理 |
| `Agent` | 启动子代理 |
| `Skill` | 加载本地技能 |
| `NotebookEdit` | 编辑 Jupyter Notebook |
| `Config` | 配置管理 |
| `REPL` | 代码执行 |
| `PowerShell` | 执行 PowerShell 命令 |

---

## 📁 项目结构

```
rust/
├── crates/
│   ├── api/           # API 客户端
│   ├── bridge/        # 跨语言桥接
│   ├── claw-cli/      # CLI 入口
│   ├── commands/      # 命令处理
│   ├── plugins/       # 插件系统
│   ├── runtime/       # 运行时
│   ├── server/        # 服务器
│   └── tools/         # 工具实现
├── Cargo.toml         # Workspace 配置
├── CHANGELOG.md       # 变更日志
└── run_claw.sh        # Linux 启动脚本
```

---

## 🔧 开发

### 运行测试

```bash
cd rust
cargo test
```

### 代码检查

```bash
cd rust
cargo clippy
```

### 构建发布版本

```bash
cd rust
cargo build --release
```

---

## 📝 变更日志

查看 [CHANGELOG.md](rust/CHANGELOG.md) 了解版本历史。

### v0.3.0 (2026-04-16)
- 添加 `run_claw.sh` Linux 一键启动脚本
- 完善文档和 README
- 项目更名为 AI-Code

### v0.2.0 (2026-04-16)
- 修复多个测试问题
- 添加 `list_directory` 和 `image_editor` 工具
- 改进 API 选择器逻辑

### v0.1.0 (2026-04-06)
- 初始发布

---

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

---

## 📄 许可证

本项目基于 MIT 许可证开源。

---

## 🙏 致谢

本项目基于 [Claw-Code](https://github.com/instructkr/claw-code) 开发，感谢原作者的贡献。
