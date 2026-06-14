# Memex

> The Memex you imagined in 1945 — finally built for the AI era.
> 让所有 AI 编辑器共享同一份"你和 AI 的全部历史"。

**English README**: [README.en.md](./README.en.md)

---

## 为什么

你每天在多个 AI 工具之间切换：Cursor、Claude Code、Codex、OpenCode、Aider、Continue、Cline……每打开一个新 session，AI 都从零开始。**你和 AI 几万次对话的经验在被一遍遍丢弃。**

Memex 是一个**本地优先**的"AI 记忆中枢"：

- 自动采集所有主流 AI CLI / 编辑器的会话历史
- 统一索引、全文检索、智能摘要
- 通过 **MCP 协议**暴露给任意 AI 编辑器，让每个新 session 都能"想起"你之前说过什么
- 桌面应用 + 系统托盘 + 内嵌 HTTP API，单进程零运维

**致敬 1945 年** — Vannevar Bush 在 *As We May Think* 中提出 Memex 概念：一台能记住一切、随时调取关联的人类记忆扩展机器。AI 时代终于让这个 80 年前的愿景成真。

---

## 特性

| 特性 | 说明 |
|---|---|
| **7 种 Adapter** | Claude Code · Cursor · Codex · OpenCode · Aider · Continue · Cline |
| **本地优先** | 所有数据留在本地 `~/.memex/`，默认不上传任何会话内容 |
| **全文检索** | SQLite FTS5 + BM25 排序 + 时间衰减 + 中文 bigram |
| **智能摘要** | Ollama 本地 LLM 四级摘要（chunk → session → project → 日报 / 周报） |
| **MCP 协议** | 4 个工具：`search_memory` / `get_session` / `list_recent` / `stats` |
| **桌面应用** | 1100×720 主窗口，五大页（今天 / 资料库 / 洞察 / 连接 / 设置） |
| **系统托盘** | 极简 360×520 popup，最近 5 条会话 + 主窗口跳板，⌘⇧M 切换 |
| **内嵌 Daemon** | HTTP API 与主程序同生共死，默认 9999 端口，被占用自动 fallback 到 10000-10009 |
| **通知中心** | 周报 / 反思待处理 / 采集失败实时入口，支持已读/未读/单条删/清空 |
| **隐私保护** | 入库前自动脱敏 + 私有会话不暴露给 MCP（两个开关均可在设置中切换） |
| **实时监听** | 文件系统事件驱动，2 秒内自动入库 |

---

## 快速开始

> **当前状态**：v1.0.3 — daemon 完全内嵌、端口 fallback、CLI 自动重试、通知中心、隐私开关、清空数据韧性、UI 全部 shadcn 化。DMG 已可在 [Releases](https://github.com/shetengteng/memex/releases) 下载；从源码构建仍推荐用于开发与定制。

### 方式 1：从源码构建（推荐）

```bash
git clone https://github.com/shetengteng/memex.git
cd memex

# 一键部署到 /Applications + 清 quarantine + 启动
bash scripts/upgrade-local.sh --skip-backup
```

脚本会自动：

1. `cargo build --release` 编译 CLI + Daemon
2. `npx tauri build --bundles app` 编译 Tauri 桌面应用
3. 替换 `/Applications/Memex.app`
4. `xattr -cr` 清除 Gatekeeper quarantine
5. 重新注册 LaunchServices + 启动新版

完成后，菜单栏点击 Memex (M) 图标即可使用。

### 方式 2：DMG 下载（v1.0.3 已可用）

```bash
# 1. 从 Releases 页下载对应架构 DMG
#    https://github.com/shetengteng/memex/releases

# 2. 双击 DMG 拖入 /Applications

# 3. 跑一键安装脚本（清 quarantine、刷新 LaunchServices、启动）
curl -fsSL https://raw.githubusercontent.com/shetengteng/memex/main/scripts/install-macos.sh | bash
```

> **为什么需要这个脚本？** 当前版本使用 ad-hoc 签名（无 Apple Developer 账号），macOS Gatekeeper 会把从浏览器下载的 App 标记成 quarantine，双击启动时弹出"已损坏 / 未识别开发者"错误。`xattr -cr Memex.app` 一次性清除即可正常使用。

---

## 桌面应用一览

启动 Memex 后，菜单栏会出现 (M) 图标，左键弹出 Tray Popup，右键打开主窗口或退出。⌘⇧M 全局快捷键可在任意位置切换主窗口。

| 页面 | 路径 | 作用 |
|---|---|---|
| **今天 (Today)** | `/today` | 当日活跃会话 + 命令面板（⌘K）跨项目搜索 |
| **资料库 (Library)** | `/library` | 全部会话 / 项目 / 主题，支持过滤、详情抽屉 |
| **洞察 (Insights)** | `/insights` | LLM 生成的周报 / 月报 / 数据统计 / 主题图谱 |
| **连接 (Connect)** | `/connect` | IDE 集成 (Cursor/Claude Code/Codex/OpenCode) 一键注入 MCP + SKILL |
| **设置 (Settings)** | `/settings` | LLM 提供商、通知开关、隐私、数据备份/恢复、Daemon 状态 |
| **托盘 (Tray)** | `/tray-popup` | 360×520，最近 5 条会话 + 跳板 |

每页都集成了**通知铃铛**：周报生成完成、反思待处理、采集失败会实时浮现，支持详情 / 标记已读 / 单条删除 / 一键清空。

---

## 架构

```
┌──────────────────────────────────────────────────────┐
│  AI 编辑器 (Cursor / Claude Code / Codex / ...)       │
└────────────┬────────────────────────────┬────────────┘
             │ MCP (stdio)                │ 写入 session 文件
             ▼                            ▼
┌──────────────────┐      ┌──────────────────────────┐
│  memex-cli mcp   │ HTTP │  Memex.app (Tauri 2)     │
│  (stdio 桥接)    │─────▶│  ├─ 内嵌 daemon          │
│                  │      │  ├─ watcher (notify)     │
└───────┬──────────┘      │  ├─ scheduler (周报/反思) │
        │  HTTP             │  ├─ auto ingest          │
        ▼                  │  ├─ HTTP API :9999       │
                           │  └─ Tray / Main Window   │
                           └──────────┬────────────────┘
                                      │
                                      ▼
┌──────────────────────────────────────────────────────┐
│              memex-core                              │
│  ├─ collector (7 adapters)                           │
│  ├─ processor (normalize · chunk · redact · meta)    │
│  ├─ storage   (Markdown 真源 + SQLite FTS5)          │
│  ├─ retriever (BM25 + recency)                       │
│  └─ llm       (Ollama / Anthropic provider)          │
└──────────────────────────────────────────────────────┘
```

**Daemon 内嵌**：v0.3.x 起，原来的独立 `memex-daemon` 进程已合并进 `Memex.app`，app 启动则自动拉起，退出则一并关闭。CLI / MCP 通过 `~/.memex/daemon.lock` 自动发现端口。

**端口策略**：默认监听 `127.0.0.1:9999`，被占用时自动在 `10000-10009` 段内 fallback，使用 `SO_REUSEADDR` 避免重启时 TIME_WAIT 阻塞。CLI 客户端遇到传输错误时会自动重读 lock 文件并以新端口重试。

---

## 技术栈

| 层 | 技术 |
|---|---|
| Core | Rust + SQLite (FTS5 / WAL) + blake3 |
| CLI | clap + ureq |
| Daemon | axum + tokio + notify + chrono |
| MCP | 手写 stdio JSON-RPC |
| Desktop App | Tauri 2 + Vue 3 + TypeScript + Vue Router 4 + shadcn-vue + reka-ui |
| LLM | Ollama (本地) / Anthropic (可选云端) |
| 构建 | Cargo workspace + Vite |

---

## CLI 命令

```
memex-cli ingest [--adapter <name>]      # 手动采集
memex-cli search <query> [--json]        # 全文检索
memex-cli sessions [--recent N]          # 列出会话
memex-cli session <id>                   # 查看会话详情
memex-cli stats                          # 统计信息
memex-cli config show / set <key> <val>  # 配置管理
memex-cli backup <path>                  # 备份归档
memex-cli restore <path>                 # 从归档恢复
memex-cli rebuild-index                  # 从 Markdown 重建 FTS 索引
memex-cli doctor                         # 健康检查
memex-cli setup cursor | claude-code     # 一键注入 MCP + SKILL
memex-cli hooks <ide>                    # 查看/启用某个 IDE 的 hooks
memex-cli mcp                            # 进入 MCP stdio 模式
memex-cli daemon status                  # Daemon 健康检查
```

所有命令都通过 HTTP 与内嵌 daemon 通信，daemon 是唯一数据入口，不再有"绕开 daemon 直连 SQLite"的路径。

---

## MCP + Skill：在你的 IDE 里直接用自然语言查记忆

Memex 暴露 **6 个 MCP 工具**给 Claude Code / Cursor / Codex / OpenCode。**安装一次后**，你不需要记任何命令——直接用自然语言问，IDE 会自动路由到对应的工具。

### 一键安装

在 Memex 桌面应用 → **连接 → IDE 集成** 勾选你的 IDE，会自动写入对应的 MCP server config + Skill 到：

- Claude Code → `~/.claude.json`
- Cursor → `~/.cursor/mcp.json` + Project Rules
- Codex → `~/.codex/config.toml`
- OpenCode → `~/.config/opencode/mcp.json`

新开一个 IDE session 即可生效。

### 6 个 MCP 工具

| 工具 | 用途 | 典型场景 |
|---|---|---|
| `search_memory` | 跨 adapter / 项目 / 时间窗口的全文检索 | 关键词秒级命中 |
| `get_session` | 拿单条会话完整正文（含 mark 高亮） | "把那次讨论调出来" |
| `list_recent` | 列最近 N 条 session | "最近一周我都聊了啥" |
| `get_project_context` | 当前 `cwd` 项目的工作记忆摘要 | "继续做之前的功能" |
| `list_sessions_by_range` | ISO 日期区间筛选 | 周报 / 月报 |
| `stats` | 总览（session 数 / message 数 / chunk 数） | "我的库现在多大" |

### 自然语言 demo（开箱即用，零参数记忆负担）

| 你在 IDE 里说 | Memex 自动做的事 |
|---|---|
| "我上周和 Cursor 在 zoom-docs 项目讨论了什么？" | `search_memory(query="zoom-docs", adapter="cursor", since_days=7)` |
| "找一下我们之前定的 retry 策略" | `search_memory(query="retry strategy", limit=8)` |
| "把 `abc12` 这条 session 的全文拉出来" | `get_session(session_id="abc12")` |
| "最近 10 个 Claude Code session" | `list_recent(limit=10, adapter="claude_code")` |
| "继续之前在这个项目做的事" | `get_project_context()` |
| "把 6 月 1 号到 6 月 7 号的所有会话列出来" | `list_sessions_by_range(after="2026-06-01", before="2026-06-07")` |
| "我索引了多少条对话？" | `stats()` |

> **核心体验**：你只管用人类的话提问，IDE 的 LLM 会替你挑工具 + 填参数。**没有命令要背，没有 SQL 要写。**

### 完整 SKILL 文档

| 文件 | 适用 |
|---|---|
| [`SKILL.md`](SKILL.md) | 通用 SKILL（6 个 MCP 工具 + CLI 速查） |
| [`app/skills/cursor/SKILL.md`](app/skills/cursor/SKILL.md) | Cursor 专属 |
| [`app/skills/claude-code/SKILL.md`](app/skills/claude-code/SKILL.md) | Claude Code 专属 |
| [`app/skills/codex/SKILL.md`](app/skills/codex/SKILL.md) | Codex 专属 |
| [`app/skills/opencode/SKILL.md`](app/skills/opencode/SKILL.md) | OpenCode 专属 |

---

## 隐私模型

Memex 默认采用最严格的隐私策略：

- **入库前自动脱敏**：API Key / 邮箱 / Token / 信用卡号等模式由 `processor::redact` 在写入数据库前替换为占位符，规则可在 `~/.memex/redactions.yaml` 中扩展
- **私有会话过滤**：被 `~/.memex/privacy.yaml` 标记为 `private: true` 的会话不会通过 MCP 暴露给 IDE
- **两个开关均可关闭**：设置 → 隐私 中可分别关停 auto-redact 和 skip-private-from-mcp（关掉后会立即生效，无需重启）
- **云端 LLM 严格 opt-in**：默认全本地 Ollama，使用 Anthropic / OpenAI 必须显式在配置中开启
- **没有遥测**：Memex 不向任何外部服务上报使用数据

---

## 设计文档

在线浏览：**<https://shetengteng.github.io/memex/>**（GitHub Pages 自动发布，含各页面截图）

本地源文件在 [`design/`](design/) 目录。本地预览 docs site：

```bash
# 静态站点直接用浏览器打开
open docs/index.html
```

---

## License

MIT
