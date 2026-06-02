# Memex

> The Memex you imagined in 1945 — finally built for the AI era.
> 让所有 AI 编辑器共享同一份"你和 AI 的全部历史"。

---

## 为什么

你现在每天在多个 AI 工具之间切换：Cursor、Claude Code、Codex、OpenCode、Aider、Continue、Cline……
每打开一个新 session，AI 都从零开始。**你和 AI 几万次对话的经验全部在浪费。**

Memex 是一个本地优先的"AI 记忆中枢"：
- 自动采集所有主流 AI CLI / 编辑器的会话历史
- 统一索引、全文检索、智能摘要
- 通过 MCP 协议暴露给任意 AI 编辑器，让每个新 session 都能 "想起" 你之前说过什么

**致敬 1945 年** — Vannevar Bush 在 *As We May Think* 中提出 Memex 概念：一台能记住一切、随时调取关联的人类记忆扩展机器。AI 时代终于让这个 80 年前的愿景成真。

---

## 特性

| 特性 | 说明 |
|---|---|
| 7 种 Adapter | Claude Code · Cursor · Codex · OpenCode · Aider · Continue · Cline |
| 本地优先 | 所有数据留在本地 `~/.memex/`，默认不上传任何会话内容 |
| 全文检索 | SQLite FTS5 + BM25 排序 + 时间衰减 + 中文 bigram |
| 智能摘要 | Ollama 本地 LLM 四级摘要（chunk → session → project → 日报） |
| MCP 协议 | 4 个工具：`search_memory` / `get_session` / `list_recent` / `stats` |
| 系统托盘 | macOS 透明圆角 popup，shadcn-vue UI，⌘⇧M 快捷键唤起 |
| Web Dashboard | `http://127.0.0.1:9999` 完整统计面板 |
| 隐私保护 | 自动脱敏 + 云端 opt-in + private session 过滤 |
| 实时监听 | 文件系统事件驱动，2 秒内自动入库 |

---

## 快速开始

### 方式 1：Homebrew Cask（macOS，最简单）

```bash
# 一次性 tap
brew tap shetengteng/memex
brew install --cask memex

# 之后升级
brew upgrade --cask memex
```

> Cask 安装会自动跑 `xattr -cr`（postflight 钩子），无需手动清除 quarantine。

### 方式 2：DMG 下载

```bash
# 1. 从 Releases 页下载 Memex_x.y.z_aarch64.dmg（M 系列）或 Memex_x.y.z_x64.dmg（Intel）
#    https://github.com/shetengteng/memex/releases

# 2. 双击 DMG 拖入 /Applications

# 3. 跑一键安装脚本（清除 quarantine、刷新 LaunchServices、启动）
curl -fsSL https://raw.githubusercontent.com/shetengteng/memex/main/scripts/install-macos.sh | bash
```

> **为什么需要这个脚本？** 当前版本使用 ad-hoc 签名（无 Apple Developer 账号），
> macOS Gatekeeper 会把从浏览器下载的 App 标记成 quarantine，
> 双击启动时弹出 "已损坏" / "未识别开发者" 错误。
> `xattr -cr Memex.app` 一次性清除 quarantine 标记即可正常使用。
>
> 也可以**手动执行**（与脚本等价）：
> ```bash
> xattr -cr /Applications/Memex.app
> open /Applications/Memex.app
> ```

### 方式 3：从源码构建

```bash
git clone https://github.com/shetengteng/memex.git
cd memex

# 构建 CLI + Daemon
cargo build --release

# 构建 Tauri Menubar App（需要 Node.js）
cd tauri-app && npm install && npx tauri build --bundles app

# 拷贝到 /Applications
cp -R target/release/bundle/macos/Memex.app /Applications/
```

### 首次运行

```bash
# 1. 启动 daemon（自动创建 ~/.memex/ 目录和 config.toml）
./target/release/memex-daemon

# 2. 手动采集一次
./target/release/memex ingest

# 3. 搜索你的 AI 历史
./target/release/memex search "如何优化数据库查询"

# 4. 查看统计
./target/release/memex stats

# 5. 安装 Menubar App
open target/release/bundle/macos/Memex.app
```

### MCP 接入

```bash
# 为 Cursor 配置 MCP
./target/release/memex setup cursor

# 为 Claude Code 配置 MCP
./target/release/memex setup claude-code
```

---

## 架构

```
┌──────────────────────────────────────────────────────┐
│  AI 编辑器 (Cursor / Claude Code / Codex / ...)      │
└────────────┬────────────────────────────┬────────────┘
             │ MCP (stdio)               │ 写入 session 文件
             ▼                           ▼
┌─────────────────┐      ┌──────────────────────────┐
│   memex mcp     │      │  memex-daemon            │
│   (stdio 模式)   │      │  ├─ watcher (notify)     │
│                 │      │  ├─ auto ingest           │
│                 │      │  └─ HTTP API :9999        │
└───────┬─────────┘      └──────────┬───────────────┘
        │                           │
        ▼                           ▼
┌──────────────────────────────────────────────────────┐
│              memex-core                              │
│  ├─ collector (7 adapters)                           │
│  ├─ processor (normalize · chunk · redact · meta)    │
│  ├─ storage   (Markdown 真源 + SQLite FTS5)          │
│  ├─ retriever (BM25 + recency)                       │
│  └─ llm       (Ollama / Anthropic provider)          │
└──────────────────────────────────────────────────────┘
```

---

## 技术栈

| 层 | 技术 |
|---|---|
| Core | Rust + SQLite (FTS5 / WAL) + blake3 |
| CLI | clap |
| Daemon | axum + tokio + notify |
| MCP | 手写 stdio JSON-RPC |
| Menubar | Tauri 2 + Vue 3 + TypeScript + shadcn-vue |
| LLM | Ollama (本地) / Anthropic (可选云端) |
| 构建 | Cargo workspace + Vite |

---

## CLI 命令

```
memex ingest [--adapter <name>]     # 手动采集
memex search <query> [--json]       # 全文检索
memex sessions [--recent N]          # 列出会话
memex session <id>                   # 查看会话详情
memex stats                          # 统计信息
memex config show / set <key> <val>  # 配置管理
memex backup <path>                  # 备份
memex rebuild-index                  # 从 Markdown 重建索引
memex doctor                         # 健康检查
memex daemon start / stop / status   # Daemon 管理
memex setup cursor / claude-code     # MCP 配置
memex mcp                            # 进入 MCP 模式
```

---

## 设计文档

在线浏览：**https://shetengteng.github.io/memex/**（GitHub Pages 自动发布）

本地源文件在 [`design/`](design/) 目录：

| 文档 | 内容 |
|---|---|
| `20260531-03-*最终设计文档.md` | v4 架构、模块边界、数据模型 |
| `20260531-12-*技术栈.md` | 技术选型 + 代码复用来源 |
| `20260602-01-*功能点开发清单-100%.md` | 功能模块视角的全集 checklist（v1.0 已 100%） |
| `20260531-09-*交互原型-v3.html` | 单 popup 聚焦版原型 |

本地预览 docs site：

```bash
pip install markdown pygments
python3 scripts/build-docs.py
open site/index.html
```

---

## MCP SKILL

- [`SKILL.md`](SKILL.md) — 通用 SKILL（4 个 MCP 工具 + CLI 速查）
- [`skills/cursor/SKILL.md`](skills/cursor/SKILL.md) — Cursor 专属
- [`skills/claude-code/SKILL.md`](skills/claude-code/SKILL.md) — Claude Code 专属
- [`skills/codex/SKILL.md`](skills/codex/SKILL.md) — Codex 专属
- [`skills/opencode/SKILL.md`](skills/opencode/SKILL.md) — OpenCode 专属

> 用法：在 Memex popup → Settings → IDE Integrations 一键安装/卸载 MCP + SKILL 到 4 个 IDE。

---

## License

MIT
