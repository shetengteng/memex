---
name: memex
description: 本地优先的跨 LLM 会话记忆中枢——搜索、检索、浏览 Claude Code / Cursor / Codex / OpenCode 等 IDE 的历史对话，本地 FTS5 全文索引。
client: claude-code
---

# Memex (Claude Code Skill)

> Claude Code 客户端专属 SKILL。通用工具说明与 CLI 命令请见仓库根目录的 `SKILL.md`。

## 适用场景（Claude Code 内）

当你在 Claude Code 中遇到以下情境时，调用 Memex MCP：

- 多 session 切换后想找回**之前会话**里讨论过的方案
- 跨 `~/.claude/projects/` 多个 project 聚合搜索
- 需要把"以往结论 + 当前问题"一起做技术决策
- 写 PR description 时，把过往讨论里的 tradeoff 拉出来作为引用

## 一次性启用（已通过 setup 自动完成则跳过）

```bash
memex-cli setup claude-code     # 写入 ~/.claude.json
memex-cli ingest                # 拉一遍历史
```

`setup claude-code` 会向 `~/.claude.json`（Claude Code CLI 实际读取的配置文件）注入：

```json
{
  "mcpServers": {
    "memex": { "command": "<absolute-path>/memex-cli", "args": ["mcp"] }
  }
}
```

注入后**重启 Claude Code**，工具以 `mcp__memex__*` 命名空间出现在工具列表（当前 6 个工具）。

## 工具调用样例（Claude Code 内）

Claude Code 的 MCP 工具调用统一前缀为 `mcp__memex__`：

| 用户原话 | 应调用 | 关键参数 |
|---|---|---|
| "我和 Claude Code 在 zoom-docs 项目聊过什么" | `mcp__memex__search_memory` | `query="zoom-docs"`, `adapter="claude_code"` |
| "上次讨论的 retry 策略最终定的是哪个？" | `mcp__memex__search_memory` | `query="retry strategy"`, `since_days=14` |
| "把 abc12 这个 session 的全文拿给我" | `mcp__memex__get_session` | `session_id="abc12"` |
| "最近 10 个 Claude Code session" | `mcp__memex__list_recent` | `limit=10` |
| "我索引了多少东西？" | `mcp__memex__stats` | — |
| "把这个 repo 的项目工作记忆拉出来" | `mcp__memex__get_project_context` | —（自动 cwd），可加 `top=5` |
| "把 6 月 1-7 号所有会话列出来" | `mcp__memex__list_sessions_by_range` | `after="2026-06-01"`, `before="2026-06-07"` |

### search_memory 推荐用法

```json
{
  "query": "retry policy exponential backoff",
  "limit": 8,
  "adapter": "claude_code",
  "since_days": 30
}
```

**返回字段**：`chunk_id` / `session_id` / `snippet`（含 `<mark>` 高亮）/ `adapter` / `project` / `timestamp` / `match_reason`。

## Claude Code 客户端注意事项

1. **工具命名前缀**：所有调用都要加 `mcp__memex__`，例如 `mcp__memex__search_memory`，否则 Claude Code 不会路由到 Memex。
2. **history.jsonl + projects/**：Memex 解析 `~/.claude/history.jsonl`（高层 session 元数据）和 `~/.claude/projects/**/*.jsonl`（完整消息），增量入库。
3. **session 边界**：每个 jsonl 文件 = 一个 session，跨文件合并时通过 `parent_uuid` 不强依赖。
4. **session_id 可前缀匹配**：传前 6~8 位即可。
5. **跨 adapter 检索**：可以同时把 Cursor / Codex 的会话也作为 RAG 输入，参数留空 `adapter` 即跨全 adapter 搜索。
6. **离线**：MCP 直接读本地 SQLite + FTS5，**不**调用任何远程服务；可与 Claude API 调用并行。

## 触发短语

`search memory`, `find claude session`, `what did I discuss`, `previous claude conversation`, `recall from claude code`, `cross-session search`, `what we decided last week`, `recall`.
