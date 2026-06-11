---
name: memex
description: 本地优先的跨 LLM 会话记忆中枢——搜索、检索、浏览 Codex / Cursor / Claude Code / OpenCode 等 IDE 的历史对话，本地 FTS5 全文索引。
client: codex
---

# Memex (Codex Skill)

> Codex 客户端专属 SKILL。通用工具说明与 CLI 命令请见仓库根目录的 `SKILL.md`。

> Codex 没有原生 `~/.codex/skills/` 目录；本文件由 Memex `setup` 写入到
> `~/.codex/prompts/memex.md`，可以在 Codex 内通过 `/memex` 命令快速注入到上下文。

## 适用场景（Codex 内）

当你在 Codex 中遇到以下情境时，调用 Memex MCP：

- 跨 session 找上次实现某个特性的具体方案
- 跨 Codex / Cursor / Claude Code 等多 IDE 的对话进行联合检索
- 写代码前快速复盘最近 3 天的相关讨论
- 调试时找"我们之前是怎么 fix 这个 bug 的"

## 一次性启用（已通过 setup 自动完成则跳过）

```bash
memex setup codex          # 写入 ~/.codex/config.toml [mcp_servers.memex]
memex ingest               # 拉一遍历史
```

`setup codex` 会向 `~/.codex/config.toml` 追加：

```toml
[mcp_servers.memex]
command = "<absolute-path>/memex"
args = ["mcp"]
enabled = true
```

注入后**重启 Codex CLI**（`codex` 二进制），运行 `/mcp` 应该能看到 `memex` 服务器及它暴露的 6 个工具（search_memory / get_session / list_recent / stats / get_project_context / list_sessions_by_range）。

## 工具调用样例（Codex 内）

| 用户原话 | 应调用 | 关键参数 |
|---|---|---|
| "我在 codex 里聊过 streaming 这事吗" | `search_memory` | `query="streaming"`, `adapter="codex"` |
| "搜一下昨天和 cursor 聊的那个 rate limit 方案" | `search_memory` | `query="rate limit"`, `adapter="cursor"`, `since_days=2` |
| "拉一下 9a8b 那个 session" | `get_session` | `session_id="9a8b"` |
| "最近 5 个会话" | `list_recent` | `limit=5` |
| "memex 现在索引了多少" | `stats` | — |
| "看下当前 repo 之前讨论到哪了" | `get_project_context` | —（自动 cwd），可加 `top=5` |
| "拉一下 6 月 1-7 号所有 session" | `list_sessions_by_range` | `after="2026-06-01"`, `before="2026-06-07"` |

### search_memory 推荐用法

```json
{
  "query": "function calling parallel tools",
  "limit": 5,
  "since_days": 14
}
```

**返回字段**：`chunk_id` / `session_id` / `snippet`（含 `<mark>` 高亮）/ `adapter` / `project` / `timestamp` / `match_reason`，每条还包含 `deep_link`（如 `memex://session/<id>`）可一键在 Memex Dashboard 里打开。

## Codex 客户端注意事项

1. **MCP 标识**：Codex 把每个 server 当作一个工具集；调用形如 `mcp.memex.search_memory`（具体取决于 Codex 版本，可以让 Codex 自己 dispatch）。
2. **trust prompt**：项目级 `.codex/config.toml` 仅对受信任目录生效，全局配置（user-level）始终生效——Memex 默认写 user-level。
3. **enabled 字段**：把 `enabled = false` 可以临时禁用 memex 而不删除条目，下次想用再切回 true。
4. **session_id 可前缀匹配**：传前 6~8 位即可，CLI 会自动解析为完整 UUID。
5. **离线**：MCP 直接读本地 SQLite + FTS5，**不**调用任何远程服务；可与 Codex 模型推理并行。

## 触发短语

`search memory`, `recall codex session`, `cross-LLM history search`, `find previous discussion`, `what did we decide`, `pull session`, `memex search`.
