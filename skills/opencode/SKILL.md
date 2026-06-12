---
name: memex
description: 本地优先的跨 LLM 会话记忆中枢——搜索、检索、浏览 OpenCode / Cursor / Codex / Claude Code 等 IDE 的历史对话，本地 FTS5 全文索引。
client: opencode
---

# Memex (OpenCode Skill)

> OpenCode 客户端专属 SKILL。通用工具说明与 CLI 命令请见仓库根目录的 `SKILL.md`。

> OpenCode 不使用 SKILL.md 体系；本文件由 Memex `setup` 写入到
> `~/.config/opencode/commands/memex.md`，让 OpenCode 在 `/commands` 列表里展示
> 一个 `memex` 命令，触发后把这份说明作为 system 上下文带入。

## 适用场景（OpenCode 内）

当你在 OpenCode 中遇到以下情境时，调用 Memex MCP：

- 跨多个 OpenCode session 找之前实现的 pattern
- 跨 IDE（OpenCode + Cursor + Claude Code）联合检索经验
- 通过 `/memex` 命令把记忆插件唤醒后再让 LLM 自由调用工具

## 一次性启用（已通过 setup 自动完成则跳过）

```bash
memex-cli setup opencode       # 写入 ~/.config/opencode/opencode.json mcp.memex
memex-cli ingest               # 拉一遍历史
```

`setup opencode` 会向 `~/.config/opencode/opencode.json` 追加：

```jsonc
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "memex": {
      "type": "local",
      "command": ["<absolute-path>/memex-cli", "mcp"],
      "enabled": true
    }
  }
}
```

注入后**重启 OpenCode**，运行 `opencode mcp list` 应该能看到 `memex` 已连接。

## 工具调用样例（OpenCode 内）

| 用户原话 | 应调用 | 关键参数 |
|---|---|---|
| "我在 opencode 项目里聊过 react server components 吗" | `search_memory` | `query="react server components"`, `adapter="opencode"` |
| "找一下昨天讨论的部署脚本" | `search_memory` | `query="deploy script"`, `since_days=2` |
| "拉个 9a8b session 给我看" | `get_session` | `session_id="9a8b"` |
| "最近 10 个会话" | `list_recent` | `limit=10` |
| "看一下索引规模" | `stats` | — |
| "把当前 repo 的工作记忆带进来" | `get_project_context` | —（自动 cwd），可加 `top=5` |
| "把上周（6 月 1-7 号）所有 session 列出来" | `list_sessions_by_range` | `after="2026-06-01"`, `before="2026-06-07"` |

### search_memory 推荐用法

```json
{
  "query": "deployment workflow github actions",
  "limit": 8,
  "adapter": "opencode"
}
```

**返回字段**：`chunk_id` / `session_id` / `snippet`（含 `<mark>` 高亮）/ `adapter` / `project` / `timestamp` / `match_reason`，每条还包含 `deep_link`（如 `memex://session/<id>`）可一键在 Memex Dashboard 里打开。

## OpenCode 客户端注意事项

1. **`type: "local"`**：OpenCode 区分 `local`（stdio）和 `remote`（HTTP），Memex 始终走 `local`。
2. **`enabled = false`**：可以临时禁用 memex 而不删除条目，下次再切回 true。
3. **跨项目使用**：MCP 配置写在 `~/.config/opencode/opencode.json`（global），所有 OpenCode session 都能访问。
4. **session_id 可前缀匹配**：传前 6~8 位即可。
5. **离线**：MCP 直接读本地 SQLite + FTS5，**不**调用任何远程服务。

## 触发短语

`/memex`, `recall opencode session`, `search memory`, `find what I discussed`, `cross-LLM history`, `pull session`, `memex search`.
