# 🦞 Claw Code

A high-performance AI coding assistant with both Rust and Node.js implementations.

## Project Structure

```
ai-code/
├── rust/                    # Rust implementation (primary)
│   ├── crates/
│   │   ├── claw-cli/        # Main CLI binary
│   │   ├── api/             # API client + streaming
│   │   ├── runtime/         # Session, config, permissions, MCP
│   │   ├── tools/           # Tool implementations
│   │   ├── commands/        # Slash commands
│   │   ├── plugins/         # Plugin system
│   │   ├── bridge/          # Bridge functionality
│   │   ├── lsp/             # LSP support
│   │   └── server/          # Server components
│   └── Cargo.toml           # Workspace config
│
├── back/                    # Node.js/TypeScript implementation
│   └── src/
│       ├── main.tsx         # Entry point
│       ├── commands.ts      # Command handling
│       ├── tools.ts         # Tool definitions
│       ├── QueryEngine.ts   # Query engine
│       └── ...              # Other modules
│
└── assets/                  # Documentation assets
```

## Quick Start

### Rust Version

```bash
cd rust/
cargo build --release

# Run interactive REPL
./target/release/claw

# One-shot prompt
./target/release/claw prompt "explain this codebase"

# With specific model
./target/release/claw --model sonnet prompt "fix the bug"
```

### Node.js Version

```bash
cd back/
npm install
npm run dev
```

## Configuration

Set your API credentials:

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
# Or use a proxy
export ANTHROPIC_BASE_URL="https://your-proxy.com"
```

Or authenticate via OAuth:

```bash
claw login
```

## Features

| Feature | Rust | Node.js |
|---------|------|---------|
| API + streaming | ✅ | ✅ |
| OAuth login/logout | ✅ | ✅ |
| Interactive REPL | ✅ | ✅ |
| Tool system (bash, read, write, edit, grep, glob) | ✅ | ✅ |
| Web tools (search, fetch) | ✅ | ✅ |
| Sub-agent orchestration | ✅ | ✅ |
| Todo tracking | ✅ | ✅ |
| Notebook editing | ✅ | ✅ |
| CLAW.md / project memory | ✅ | ✅ |
| Config file hierarchy | ✅ | ✅ |
| Permission system | ✅ | ✅ |
| MCP server lifecycle | ✅ | ✅ |
| Session persistence + resume | ✅ | ✅ |
| Extended thinking | ✅ | ✅ |
| Cost tracking | ✅ | ✅ |
| Git integration | ✅ | ✅ |
| Markdown terminal rendering | ✅ | ✅ |
| Model aliases | ✅ | ✅ |
| Slash commands | ✅ | ✅ |
| Vim mode | ✅ | ✅ |

## Model Aliases

| Alias | Resolves To |
|-------|------------|
| `opus` | `claude-opus-4-6` |
| `sonnet` | `claude-sonnet-4-6` |
| `haiku` | `claude-haiku-4-5-20251213` |

## CLI Commands

```
claw [OPTIONS] [COMMAND]

Options:
  --model MODEL                    Set the model
  --dangerously-skip-permissions   Skip all permission checks
  --permission-mode MODE           Set permission mode
  --allowedTools TOOLS             Restrict enabled tools
  --output-format FORMAT           Output format (text/json)
  --version, -V                    Print version

Commands:
  prompt <text>      One-shot prompt
  login              OAuth authentication
  logout             Clear credentials
  init               Initialize project
  doctor             Check environment
```

## Slash Commands

| Command | Description |
|---------|-------------|
| `/help` | Show help |
| `/status` | Session status |
| `/cost` | Cost breakdown |
| `/compact` | Compact history |
| `/clear` | Clear conversation |
| `/model [name]` | Switch model |
| `/permissions` | Permission mode |
| `/config [section]` | Show config |
| `/memory` | CLAW.md contents |
| `/diff` | Git diff |
| `/export [path]` | Export conversation |
| `/session [id]` | Resume session |
| `/version` | Show version |

## Version

Current version: **0.1.0**

## License

MIT
