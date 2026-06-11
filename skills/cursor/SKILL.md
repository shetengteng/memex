---
name: memex
description: 本地优先的跨 LLM 会话记忆中枢——搜索、检索、浏览 Cursor / Claude Code / Codex / OpenCode 等 IDE 的历史对话，本地 FTS5 全文索引。
client: cursor
---

# Memex (Cursor Skill)

> Cursor 客户端专属 SKILL。通用工具说明与 CLI 命令请见仓库根目录的 `SKILL.md`。

## 适用场景（Cursor 内）

当你在 Cursor 中遇到以下情境时，调用 Memex MCP：

- "我之前是怎么解决 X 的？"
- "前几天讨论的 Redis pipeline 方案最后定了哪个？"
- "查一下我和 Cursor 在 `<project>` 里聊过的 auth 话题"
- 需要把过往会话作为 RAG 上下文塞进当前 Composer

## 一次性启用（已通过 setup 自动完成则跳过）

```bash
memex setup cursor          # 写入 ~/.cursor/mcp.json
memex ingest                # 拉一遍历史
```

`setup cursor` 会向 `~/.cursor/mcp.json` 注入：

```json
{
  "mcpServers": {
    "memex": { "command": "<absolute-path>/memex", "args": ["mcp"] }
  }
}
```

注入后**重启 Cursor**，工具会出现在 Composer 的工具面板。

## 工具调用样例（Composer 内）

| 用户原话 | 应调用 | 关键参数 |
|---|---|---|
| "在我之前的 cursor 会话里搜 redis pipeline" | `search_memory` | `query="redis pipeline"`, `adapter="cursor"` |
| "上周这个 repo 的对话有没有提到 auth？" | `search_memory` | `query="auth"`, `project="<当前 repo 名>"`, `since_days=7` |
| "把会话 abc12 的完整内容给我" | `get_session` | `session_id="abc12"` |
| "最近 5 个会话" | `list_recent` | `limit=5` |
| "memex 索引规模" | `stats` | — |
| "把当前 repo 之前的进度拉出来" | `get_project_context` | —（自动取 cwd），可加 `top=5` |
| "看一下 6 月 1-7 号都在做什么" | `list_sessions_by_range` | `after="2026-06-01"`, `before="2026-06-07"` |

### search_memory 推荐用法

```json
{
  "query": "authentication middleware",
  "limit": 5,
  "adapter": "cursor",
  "project": "my-app"
}
```

**返回字段**：`chunk_id` / `session_id` / `snippet`（含 `<mark>` 高亮）/ `adapter` / `project` / `timestamp` / `match_reason`。

## Cursor 客户端注意事项

1. **Composer 上下文**：MCP 工具调用结果会进入当前 Composer 的上下文，可直接被后续推理引用。
2. **跨 workspace 检索**：Memex 索引是全局的，跨 Cursor workspace 也能命中——加 `project=` 过滤可锁定单 repo。
3. **session_id 可前缀匹配**：Cursor 的 session UUID 较长，传前 6~8 位即可。
4. **隐私**：在 `redactions.yaml` 配 `private_paths` 可让某些目录的会话不入库；运行时已脱敏（email / API key / IP / 银行卡）。
5. **离线**：MCP 直接读本地 SQLite + FTS5，**不**调用任何远程服务。

## 触发短语

`search memory`, `find session`, `what did I discuss`, `previous conversation`, `cursor history`, `composer history`, `what we worked on yesterday`, `recall`, `cross-LLM search`.
