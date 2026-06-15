# MCP 工具

Memex 暴露 **6 个 MCP tool**。AI 在你提问时会按上下文自动选择调用哪个，不需要你记参数。下面每个工具都给了真实的"用户提问 → AI 选了什么工具 → 实际返回"链路，方便理解触发逻辑。

> 所有 MCP 调用都会被记录到 `~/.memex/memex.db` 的 `mcp_call_log` 表，**Connect 页 → MCP 活动**实时展示 24h 调用统计 + 失败列表。

## `get_project_context` · 最常用

**用于**：用户回到一个项目、想"接着干上次的活"，AI 需要快速理解"这个项目最近在搞什么"。

**触发关键词**：「之前」「接着干」「上次讨论」「当前进度」「回顾决策」「项目阶段」「整理一下」。

**参数**：

```json
{ "project": "/Users/me/work/memex", "top": 5 }
```

`project` 强烈建议传**绝对路径**——hook 启动时 `$PWD` 不一定是 workspace 根，传错路径会拿不到上下文。

**返回**（Markdown 格式）：

```markdown
## Memex 工作记忆 · memex

### 项目摘要
本地优先的跨 LLM 会话记忆中枢，由 Tauri + Vue + Rust 实现。

### 最近 5 条会话
1. [2026-06-15] 私有标记 + 部署
2. [2026-06-14] EBADF 修复
...

### 关键决策
- v5 migration 加 sessions.is_private 列
- MCP get_session fail-closed 过滤私有会话
```

## `search_memory`

**用于**：「我之前讨论过 X 吗？」「找一下关于 X 的对话」。FTS5 全文检索 + 时间衰减排序，最近聊过的优先。

**参数**：

```json
{
  "query": "retry 策略 exponential backoff",
  "limit": 5,
  "adapter": "cursor",
  "project": "memex"
}
```

`adapter` / `project` 是**可选过滤器**。比如想限定"只看 Cursor 里聊过的"或"只看 memex 项目的"。

**返回片段示例**：

```json
{
  "results": [
    {
      "session_id": "abc123",
      "snippet": "重试用 exponential backoff，初始 1s，最大 30s...",
      "adapter": "cursor",
      "project": "memex",
      "timestamp": "2026-06-12T14:30:00Z",
      "score": 0.87
    }
  ]
}
```

## `list_sessions_by_range`

**用于**：日报 / 周报 / 月报。「本周做了什么」「上个月在搞什么」「6 月 1 日到 6 月 8 日」。

**参数**：

```json
{ "after": "2026-06-01", "before": "2026-06-15", "limit": 20 }
```

返回区间内所有会话的元数据 + L2 摘要（如有），AI 会自动归纳成日报。

## `list_recent`

**用于**：「我最近的会话有哪些」「上一次开的 session 是什么」。

简单直白——按 `updated_at desc` 返回最新 N 条会话。配合 `get_session` 用，先列再翻详情。

```json
{ "limit": 10 }
```

## `get_session`

**用于**：从 `search_memory` / `list_recent` 拿到 session ID 后，查具体对话内容。

```json
{ "session_id": "abc123" }
```

**重要**：**私有会话 fail-closed**——AI 即便知道 ID 也取不到，返回 `session not found: abc123`，连"存在性"都不暴露。

## `stats`

**用于**：「memex 数据库现状」「攒了多少会话」「一共聊了多少消息」。

```json
{}  // 无参数
```

返回：

```json
{ "sessions": 1247, "messages": 38291, "chunks": 142876 }
```

## 怎么看 AI 实际调了哪个工具

**Connect 页 → MCP 活动 卡片**：

```
今日调用 47 次（成功 45 / 失败 2）  · 平均 38ms

按工具：
  search_memory          21 次  · avg 42ms
  get_project_context     8 次  · avg 215ms（含 LLM 摘要）
  list_recent            14 次  · avg 12ms
  get_session             4 次  · avg 8ms

最近失败：
  16:45  search_memory   "Project not found: /old/path"  cursor
  ...
```

如果 AI 应该用 MCP 但没用，看这里 = 0 调用记录就是 IDE 端 MCP 配置错了。
