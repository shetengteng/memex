# TARS Hook 系统分析 & Memex 适用性评估

## 1. TARS Hook 机制概览

TARS 通过 Claude Code / Cursor 原生的 hook 系统实现了会话生命周期管理。Hook 配置分布在两个位置：

| 平台 | 配置文件 | Hook 类型 |
|------|----------|-----------|
| Claude Code | `~/.claude/settings.json` | SessionStart, PostToolUse, Stop, UserPromptSubmit, UserPromptExpansion |
| Cursor | `~/.cursor/hooks.json` | postToolUse, stop |

### 1.1 SessionStart — 上下文注入

**触发时机**：AI 会话启动时，在用户第一条消息被处理前执行。

**做了什么**：运行 `tars inject`，该命令：

1. 获取当前工作目录（`$PWD`）
2. 调用 `search_by_project(cwd)` 按三级优先级匹配项目：
   - Tier 1: 精确路径匹配
   - Tier 2: 精确项目名匹配
   - Tier 3: 模糊子串匹配
3. 从 `.md` WorkContext 文件中加载最近的会话摘要
4. 输出结构化 Markdown 到 stdout，包含：
   - 项目概览（总会话数、最近活跃时间）
   - 最近 2 个会话上下文（分支、摘要、下一步）
   - 聚焦建议（未完成任务的优先级排序）
   - 代码风格画像（从历史中提取的编码偏好）

**关键设计**：hook 的 stdout 输出会被 AI 平台注入到会话的初始上下文中，让 AI 在第一轮对话就具备项目记忆。

```json
// ~/.claude/settings.json 中的配置
"SessionStart": [{
  "matcher": "",
  "hooks": [{
    "type": "command",
    "command": "/Users/TerrellShe/.local/bin/tars inject"
  }]
}]
```

### 1.2 Stop — 会话结束回收

**触发时机**：AI 会话结束时。

**做了什么**（两个独立任务并行）：

1. **Skill 使用统计回收**：`skill-session-end.py` 分析本次会话的 skill 使用情况，判断是否需要收集反馈
2. **异步重建索引**：`tars reindex --no-summarize` 在后台扫描刚结束的会话文件，创建/更新 WorkContext `.md` 文件

```json
"Stop": [{
  "hooks": [
    {"type": "command", "command": "python3 skill-session-end.py"},
    {"type": "command", "command": "(tars reindex --no-summarize >/tmp/tars-hook-reindex.log 2>&1 &)"}
  ]
}]
```

### 1.3 PostToolUse — 工具调用追踪

**触发时机**：每次 AI 调用工具（Shell/Read/Write/StrReplace）后。

**做了什么**：记录 skill 使用日志到 `~/.skill-market/logs/usage.jsonl`，用于 skill 推荐和反馈收集。

### 1.4 UserPromptSubmit — 用户输入追踪

**触发时机**：用户提交 prompt 时。

**做了什么**：维护 turn 计数器，用于 skill 使用统计和会话计量。

## 2. TARS 与 Memex 架构对比

| 维度 | TARS | Memex |
|------|------|-------|
| **运行模型** | 无 daemon，纯 CLI | daemon 常驻 + file watcher |
| **数据采集触发** | hook 被动触发（Stop 时 reindex） | watcher 主动监听文件变更 |
| **索引格式** | `.md` WorkContext 文件 | SQLite（sessions/messages/chunks/summaries） |
| **上下文注入** | SessionStart hook → stdout → AI 初始上下文 | MCP server（按需查询） |
| **摘要生成** | reindex 时执行（可选 fast/ollama） | ingest 时自动触发 L1→L2→L3→L4 管线 |
| **会话边界检测** | 依赖文件系统 mtime + hook 时机 | 依赖 `.jsonl` 文件变更事件 |

**核心区别**：TARS 没有 daemon，必须依赖 hook 在关键时机触发工作。Memex 有 daemon，已经通过 file watcher 自动化了 TARS 需要 hook 才能完成的数据采集工作。

## 3. Memex 是否需要 Hook？

### 3.1 不需要 Hook 的部分（Memex 已有更好方案）

**数据采集（替代 Stop hook 的 reindex）**

Memex daemon 的 `start_watcher()` 已经在 `.jsonl/.json` 文件变更时自动触发 `run_ingest()`。这比 TARS 的 Stop hook 更优：

- **实时性更高**：文件变更后 2 秒内触发（DEBOUNCE），而非等会话结束
- **不丢数据**：watcher 持续运行，即使 hook 配置缺失或被跳过也能采集
- **支持增量**：通过 `source_offset` 只处理新增消息

**摘要管线（替代 reindex + summarize）**

Memex 的 `try_summarize_new_sessions()` 在每次 ingest 后自动执行 L1→L4 全链路，无需额外触发。

### 3.2 可以从 Hook 机制借鉴的部分

#### A. SessionStart 上下文注入（高价值）

**TARS 的杀手级功能**是 SessionStart hook——让 AI 在会话开始时就知道"你之前在这个项目上做了什么"。Memex 目前没有这个能力。

**Memex 的两种实现路径**：

| 方案 | 实现方式 | 优点 | 缺点 |
|------|----------|------|------|
| **A1: Hook 注入** | 注册 SessionStart hook，调用 `memex-cli context` 输出项目摘要到 stdout | 与 TARS 一致，直接进入 AI 上下文 | 需要维护 hook 配置，每个 IDE 平台不同 |
| **A2: MCP 被动查询** | AI agent 在 skill/rule 中被指引"先查 Memex MCP 获取项目上下文" | 不依赖 hook，MCP 已有 | 依赖 AI 主动查询，不保证执行 |

**推荐**：先做 A1（CLI 命令），同时保留 A2 作为补充。

具体做法：
1. 给 `memex-cli` 添加 `context` 子命令，功能类似 `tars inject`——按 `$PWD` 查找项目，输出最近会话摘要 + 项目概览
2. 编写一个 setup 命令，自动向 `~/.claude/settings.json` 和 `~/.cursor/hooks.json` 注入 SessionStart hook

#### B. 会话边界精确检测（中等价值）

当前 Memex watcher 基于文件变更触发 ingest，但它不知道"某个会话是否已经结束"。如果有 Stop hook，可以：

- 标记会话为"已完成"，触发最终 L2 摘要
- 避免对未完成会话生成不完整的摘要

**但**：Memex 当前的"会话无新消息超过阈值 → 视为完成"策略已经足够好。Stop hook 只是锦上添花。

#### C. 使用统计（低价值）

PostToolUse/UserPromptSubmit 追踪是 Skill Market 的需求，Memex 本身不需要。

## 4. 推荐实现优先级

| 优先级 | 功能 | 依赖 | 预估工作量 |
|--------|------|------|-----------|
| P0 | `memex-cli context` 命令 | 无 | 小（查 DB → 格式化输出） |
| P1 | `memex-cli setup hooks` 自动注入 hook 配置 | P0 | 小（读写 JSON） |
| P2 | SessionStart hook 注册到 Claude Code / Cursor | P0+P1 | 小（配置层面） |
| P3 | Stop hook 精确会话结束标记 | 可选 | 中（需要修改摘要触发逻辑） |

## 5. `memex-cli context` 命令设计草案

```
memex context [--project <path>] [--top <n>] [--format markdown|json]
```

输出示例：
```markdown
**Memex 项目记忆** — memex

最近 3 个会话 · 最后活跃 2026-06-03

1. [2026-06-03] LLM 配置系统重构
   添加 DeepSeek provider、Settings UI test 按钮、修复 ollama 空字符串 bug
   下一步：通用 LLM provider 体系

2. [2026-06-02] Cursor adapter SQLite 修复
   修复 TEXT/BLOB 列类型导致的数据采集失败
   下一步：验证数据入库

项目概览：跨 IDE AI 会话记忆工具，Rust+Tauri+Vue 架构
```

## 6. 结论

Memex 在数据采集层面已经比 TARS 的 hook 机制更先进（daemon + watcher > 被动 hook）。但在**上下文注入**这个维度，TARS 的 SessionStart hook 是 Memex 目前完全缺失的能力，值得借鉴实现。

建议的演进路线：
1. **短期**：实现 `memex-cli context` 命令 + hook 自动注入
2. **中期**：利用 MCP server 提供更丰富的查询能力（按时间/项目/关键词）
3. **长期**：考虑 proactive context push（daemon 检测到新会话启动 → 主动通过某种机制推送上下文）
