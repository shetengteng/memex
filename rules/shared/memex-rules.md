# Memex MCP 使用规则

本机已通过该 IDE 的 MCP 配置接入 Memex —— 本地 AI 记忆中枢 —— 暴露 6 个工具：
`get_project_context` / `search_memory` / `list_recent` / `get_session` /
`stats` / `list_sessions_by_range`。

## 强制规则

当用户的提问命中下列**任一**触发场景时，**必须先调对应的 memex MCP 工具**再回答，
不允许直接用 `git log` / `Read` 文件 / `cat README.md` 等手工方式拼凑历史。

### 场景 1：项目工作记忆 → `get_project_context`

关键词："我之前在这个项目做到哪了" / "接着干" / "上次讨论了什么" / "整理一份当前进度" /
"回顾一下决策" / "项目现在到什么阶段" / "继续做之前的功能"。

操作：以 `project = <当前 workspace 绝对路径>` + `top = 5` 调用 `get_project_context`。
不要省略 `project` 参数，因为部分 IDE 启动 hook 时 `$PWD` 不是 workspace 根。

### 场景 2：跨会话知识检索 → `search_memory`

关键词："我之前讨论过 X 吗" / "之前聊过 X 没" / "找一下之前关于 X 的对话" /
"我跟你聊过 X 吗"。

操作：调 `search_memory(query="<关键词>", limit=5)`；如果用户指定了适配器或项目，
带上 `adapter=` / `project=` 过滤。

### 场景 3：日报 / 周报 / 月报 → `list_sessions_by_range`

关键词："本周做了什么" / "今天一天干嘛了" / "上个月在搞什么" / "近 N 天" /
"6 月 1 日到 6 月 8 日"。

操作：以 ISO 日期 `after=` / `before=` 调用 `list_sessions_by_range`，默认 `limit=10`。

### 场景 4：单条会话回顾 → `list_recent` + `get_session`

关键词："我最近的会话" / "上一次开的 session" / "前缀 X 那条对话"。

操作：先 `list_recent(limit=N)` 拿 ID，再用 `get_session(session_id=<前缀或全 id>)`
取详情。

### 场景 5：总览 → `stats`

关键词："攒了多少会话" / "memex 数据库现状" / "一共聊了多少消息"。

操作：直接调 `stats`，三个数字 sessions / messages / chunks 都要带出来。

## 禁止行为

- **禁止**用 `git log` / `git status` 回答"我之前做了什么"—— git 只反映 commit，
  反映不了讨论过程与决策路径
- **禁止**用 `Read README.md` / `Read design/*.md` 替代 MCP 工作记忆 ——
  这些是产物文档，不是会话历史
- **禁止**在没调 MCP 的情况下编造会话数 / 项目名 / 日期 —— 凡引用具体数字必须有 MCP 工具
  返回作为证据

## 验证清单（回答中必须满足）

- [ ] 调用了至少一个 memex MCP 工具，且参数合理
- [ ] 回答里至少出现一条 `memex://session/<id>` deep link 引用
- [ ] 引用的会话数 / 项目名 / 日期 与 MCP 工具返回值字面一致（不允许从其他来源凑）
- [ ] 如 `get_project_context` 返回"暂无关联会话记忆"，先尝试用 `project=` 绝对路径
  重新调一次，再 fallback 到 `list_recent(limit=10)` 给用户看最近活动

## hook cwd 误判时的处理

部分 IDE（Cursor / Claude Code）的 sessionStart hook 启动时 `$PWD` 可能 = IDE 自身的
配置目录（`~/.cursor` / `~/.claude`），不是 workspace 根。此时 hook 注入的
`Memex 工作记忆` banner 已经在 memex CLI 内部做了 fallback —— 自动切到「最近活跃项目」
的摘要，banner 上会带 `fallback_from_ide_dir: true` 标记。

即便看到 banner 说"暂无关联会话记忆"，也**应主动**用当前 workspace 绝对路径再调一次
`get_project_context(project=<absolute path>)` 双检，而不是放弃。
