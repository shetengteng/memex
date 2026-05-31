# Memex MCP Skill

Local-first cross-LLM session memory hub. Search, retrieve, and browse AI conversation history from Claude Code, Cursor, Codex, and OpenCode — all indexed locally with FTS5.

## Setup

```bash
# One-time: install and configure
cargo install --path crates/memex-cli
memex setup cursor       # or: memex setup claude-code

# Ingest sessions
memex ingest
```

## MCP Tools

| Tool | Description | Required Params |
|------|-------------|-----------------|
| `search_memory` | FTS5 search across all sessions | `query` (string) |
| `get_session` | Get full session by ID/prefix | `session_id` (string) |
| `list_recent` | List recent sessions | — |
| `stats` | Show index statistics | — |

### search_memory

```json
{ "query": "authentication middleware", "limit": 5, "adapter": "cursor", "project": "my-app" }
```

Returns matching chunks with snippet, session ID, adapter, project, and timestamp.

### get_session

```json
{ "session_id": "abc123" }
```

Returns session metadata and all messages (role + content).

### list_recent

```json
{ "limit": 10 }
```

Returns recent sessions with ID, adapter, project, timestamp, and message count.

### stats

Returns total sessions, messages, and chunks in the index.

## CLI Commands

| Command | Description |
|---------|-------------|
| `memex ingest [--adapter X]` | Ingest sessions from AI tool history |
| `memex search <query> [--limit N] [--adapter X] [--project X]` | Search indexed sessions |
| `memex sessions [--recent N]` | List sessions |
| `memex session <id>` | Show a specific session |
| `memex stats` | Show statistics |
| `memex config show` | Show configuration |
| `memex config set <key> <value>` | Set configuration value |
| `memex setup <target>` | Configure MCP for cursor/claude-code |
| `memex doctor` | Run diagnostics |
| `memex backup <path>` | Export data archive |
| `memex rebuild-index` | Rebuild SQLite from Markdown |
| `memex mcp` | Start MCP server (stdio) |

## Configuration Keys

| Key | Type | Description |
|-----|------|-------------|
| `adapters.claude_code` | bool | Enable Claude Code adapter |
| `adapters.cursor` | bool | Enable Cursor adapter |
| `adapters.codex` | bool | Enable Codex adapter |
| `adapters.opencode` | bool | Enable OpenCode adapter |
| `llm.ollama_enabled` | bool | Enable local LLM summarization |
| `llm.ollama_url` | string | Ollama server URL |
| `llm.ollama_model` | string | Ollama model name |
| `privacy.redaction_enabled` | bool | Enable PII redaction |
| `privacy.skip_private_sessions` | bool | Skip private sessions |
| `data_dir` | string | Data storage directory |

## Custom Redaction Rules

Create `~/.memex/redactions.yaml`:

```yaml
rules:
  - pattern: "INTERNAL-\\d+"
    label: internal_id
  - pattern: "corp\\.example\\.com"
    label: internal_domain
```

## Data Model

- **Sessions**: collected from AI tool history directories
- **Messages**: individual turns (user/assistant/system/tool)
- **Chunks**: FTS5-indexed segments split from messages
- Markdown files are the source of truth; SQLite is a reconstructible index

## Trigger Phrases

Use when the user asks: search memory, find session, what did I discuss, previous conversation, session history, memex, recall, cross-LLM search, AI conversation history.
