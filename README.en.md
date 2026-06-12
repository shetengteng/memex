# Memex

> The Memex you imagined in 1945 — finally built for the AI era.
> One unified history for you and every AI editor you use.

**中文 README**: [README.md](./README.md)

---

## Why

You switch between many AI tools every day: Cursor, Claude Code, Codex, OpenCode, Aider, Continue, Cline… Every new session starts from zero. **Tens of thousands of conversations of context get thrown away over and over.**

Memex is a **local-first** "AI memory hub":

- Auto-collects sessions from every mainstream AI CLI / editor
- Unified indexing, full-text search, intelligent summarization
- Exposes your entire history through the **MCP protocol** so every new session can "remember" what you said before
- Desktop app + system tray + embedded HTTP API in a single process — zero ops

**Inspired by 1945** — Vannevar Bush coined "Memex" in *As We May Think*: a machine that augments human memory by recording everything and following associations on demand. The AI era finally makes that 80-year-old vision real.

---

## Features

| Feature | Description |
|---|---|
| **7 Adapters** | Claude Code · Cursor · Codex · OpenCode · Aider · Continue · Cline |
| **Local-first** | Everything stays in `~/.memex/`; nothing is uploaded by default |
| **Full-text search** | SQLite FTS5 + BM25 ranking + time decay + CJK bigram |
| **Smart summaries** | Local Ollama LLM, 4-tier (chunk → session → project → daily / weekly) |
| **MCP protocol** | 4 tools: `search_memory` / `get_session` / `list_recent` / `stats` |
| **Desktop app** | 1100×720 main window, 5 tabs (Today / Library / Insights / Connect / Settings) |
| **System tray** | Minimal 360×520 popup, 5 latest sessions + jumppad, ⌘⇧M to toggle |
| **Embedded daemon** | HTTP API lives and dies with the app; default port 9999, auto-fallback to 10000–10009 if busy |
| **Notification center** | Weekly report, reflection pending, ingestion failures — supports mark read/unread/delete/clear |
| **Privacy** | Auto-redaction before write + private sessions hidden from MCP (both togglable in Settings) |
| **Live watcher** | File-system event driven, ingested in under 2 seconds |

---

## Quick Start

> **Status**: v1.0.0 — daemon fully embedded, port fallback, CLI auto-retry, notification center, privacy switches, resilient wipe-all, shadcn UI everywhere. DMG and Homebrew Cask will be enabled once the first GitHub Release ships. Until then prefer **Option 1: build from source**.

### Option 1: Build from source (recommended)

```bash
git clone https://github.com/shetengteng/memex.git
cd memex

# Build + deploy to /Applications + clear quarantine + launch
bash scripts/upgrade-local.sh --skip-backup
```

The script will automatically:

1. `cargo build --release` — build CLI + daemon
2. `npx tauri build --bundles app` — build the Tauri desktop app
3. Replace `/Applications/Memex.app`
4. `xattr -cr` — clear Gatekeeper quarantine
5. Re-register LaunchServices + launch the new version

When it's done, click the Memex (M) icon in your menubar.

### Option 2: DMG download (after v1.0)

```bash
# 1. Download the matching DMG from the Releases page
#    https://github.com/shetengteng/memex/releases

# 2. Drag into /Applications

# 3. Run the one-shot installer (clears quarantine, refreshes LaunchServices, launches)
curl -fsSL https://raw.githubusercontent.com/shetengteng/memex/main/scripts/install-macos.sh | bash
```

> **Why the script?** This build uses ad-hoc signing (no Apple Developer account). macOS Gatekeeper marks browser-downloaded apps as quarantined, so double-clicking shows "damaged / unidentified developer". `xattr -cr Memex.app` clears the flag once and you're done.

### Option 3: Homebrew Cask (after v1.0)

```bash
brew tap shetengteng/memex
brew install --cask memex
```

---

## Desktop App Tour

When Memex is running, the menubar shows an (M) icon. Left-click for the tray popup, right-click for the main window or quit. ⌘⇧M toggles the main window from anywhere.

| Page | Route | Purpose |
|---|---|---|
| **Today** | `/today` | Active sessions today + Command Palette (⌘K) for cross-project search |
| **Library** | `/library` | All sessions / projects / topics, filter + detail drawer |
| **Insights** | `/insights` | LLM-generated weekly/monthly reports, stats, topic graphs |
| **Connect** | `/connect` | One-click inject MCP + SKILL into Cursor / Claude Code / Codex / OpenCode |
| **Settings** | `/settings` | LLM providers, notification toggles, privacy, backup/restore, daemon status |
| **Tray** | `/tray-popup` | 360×520 popup, 5 latest sessions + jumppad |

Every page hosts a **notification bell**: weekly-report-done, reflection-pending, ingestion-failed surface here with detail / mark-read / delete / clear-all controls.

---

## Architecture

```
┌──────────────────────────────────────────────────────┐
│  AI editors (Cursor / Claude Code / Codex / ...)     │
└────────────┬────────────────────────────┬────────────┘
             │ MCP (stdio)                │ writes session files
             ▼                            ▼
┌──────────────────┐      ┌──────────────────────────┐
│  memex-cli mcp   │ HTTP │  Memex.app (Tauri 2)     │
│  (stdio bridge)  │─────▶│  ├─ embedded daemon       │
│                  │      │  ├─ watcher (notify)      │
└───────┬──────────┘      │  ├─ scheduler (reports)   │
        │  HTTP             │  ├─ auto ingest          │
        ▼                  │  ├─ HTTP API :9999        │
                           │  └─ Tray / Main Window    │
                           └──────────┬────────────────┘
                                      │
                                      ▼
┌──────────────────────────────────────────────────────┐
│              memex-core                              │
│  ├─ collector (7 adapters)                           │
│  ├─ processor (normalize · chunk · redact · meta)    │
│  ├─ storage   (Markdown source-of-truth + SQLite FTS5)│
│  ├─ retriever (BM25 + recency)                       │
│  └─ llm       (Ollama / Anthropic provider)          │
└──────────────────────────────────────────────────────┘
```

**Embedded daemon**: Since v0.3.x, the standalone `memex-daemon` process has been merged into `Memex.app`. The daemon starts and exits with the app. CLI / MCP discover its port through `~/.memex/daemon.lock`.

**Port strategy**: Listens on `127.0.0.1:9999` by default; if busy, auto-falls back within `10000-10009`, using `SO_REUSEADDR` to dodge TIME_WAIT on restart. CLI clients automatically re-read the lock file and retry on a new port if a transport error occurs.

---

## Tech Stack

| Layer | Tech |
|---|---|
| Core | Rust + SQLite (FTS5 / WAL) + blake3 |
| CLI | clap + ureq |
| Daemon | axum + tokio + notify + chrono |
| MCP | hand-rolled stdio JSON-RPC |
| Desktop app | Tauri 2 + Vue 3 + TypeScript + Vue Router 4 + shadcn-vue + reka-ui |
| LLM | Ollama (local) / Anthropic (optional cloud) |
| Build | Cargo workspace + Vite |

---

## CLI Commands

```
memex-cli ingest [--adapter <name>]      # manual collection
memex-cli search <query> [--json]        # full-text search
memex-cli sessions [--recent N]          # list sessions
memex-cli session <id>                   # session detail
memex-cli stats                          # statistics
memex-cli config show / set <key> <val>  # config management
memex-cli backup <path>                  # archive backup
memex-cli restore <path>                 # restore from archive
memex-cli rebuild-index                  # rebuild FTS index from markdown
memex-cli doctor                         # health check
memex-cli setup cursor | claude-code     # one-shot inject MCP + SKILL
memex-cli hooks <ide>                    # inspect/enable IDE hooks
memex-cli mcp                            # enter MCP stdio mode
memex-cli daemon status                  # daemon health check
```

Every command talks to the embedded daemon over HTTP. The daemon is the single data entry point — no path bypasses it to touch SQLite directly.

---

## MCP SKILL

| File | For |
|---|---|
| [`SKILL.md`](SKILL.md) | Generic SKILL (4 MCP tools + CLI cheat sheet) |
| [`app/skills/cursor/SKILL.md`](app/skills/cursor/SKILL.md) | Cursor-specific |
| [`app/skills/claude-code/SKILL.md`](app/skills/claude-code/SKILL.md) | Claude Code-specific |
| [`app/skills/codex/SKILL.md`](app/skills/codex/SKILL.md) | Codex-specific |
| [`app/skills/opencode/SKILL.md`](app/skills/opencode/SKILL.md) | OpenCode-specific |

> Usage: in Memex desktop app → **Connect → IDE Integrations**, install/uninstall MCP + SKILL into the 4 IDEs in one click. After installing, open a new session in that IDE and call `search_memory("xxx")` to query all your history.

---

## Privacy Model

Memex defaults to the strictest privacy posture:

- **Auto-redact before write** — API keys / emails / tokens / card numbers are matched and replaced by placeholders in `processor::redact` before any row hits SQLite. Rules live in `~/.memex/redactions.yaml` and are user-extensible.
- **Private session filter** — sessions marked `private: true` in `~/.memex/privacy.yaml` are excluded from every MCP tool response.
- **Both toggles are live** — Settings → Privacy lets you turn auto-redact and skip-private-from-mcp on/off independently; takes effect immediately, no restart needed.
- **Cloud LLM is opt-in** — Ollama (local) is the default. Anthropic / OpenAI must be explicitly enabled in config.
- **No telemetry** — Memex does not report usage data to any external service.

---

## Design Docs

Browse online: **<https://shetengteng.github.io/memex/>** (auto-published via GitHub Pages, includes UI screenshots)

Source lives in [`design/`](design/). Preview locally:

```bash
open docs/index.html
```

---

## License

MIT
