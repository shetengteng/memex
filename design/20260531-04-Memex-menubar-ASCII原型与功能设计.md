# Memex menubar ASCII 原型与功能设计

> 日期：2026-05-31
> 状态：开发用原型
> 范围：macOS menubar 入口、搜索、详情、设置、报告、异常态

---

## 1. 设计目标

menubar 是 Memex 给人的入口。

它要做到：

- 用户不离开当前 IDE 就能搜索历史。
- 用户一眼知道今天 AI 工具使用情况。
- 用户能快速打开最近 session。
- 用户能确认采集、MCP、Ollama、Claude fallback 的状态。
- 用户能修改隐私、adapter、备份、LLM 设置。

menubar 不承担复杂管理后台职责。复杂配置仍然可以通过 CLI 或未来 Web UI 完成。

---

## 2. 信息架构

```text
Memex menubar
├── 状态栏图标
│   ├── 今日调用数
│   ├── 采集状态
│   └── 异常状态
├── 主面板
│   ├── 搜索入口
│   ├── 今日统计
│   ├── 最近 session
│   └── 快捷操作
├── 搜索面板
│   ├── 搜索框
│   ├── adapter 过滤
│   ├── 时间过滤
│   └── 搜索结果
├── 详情面板
│   ├── session 元信息
│   ├── 摘要
│   ├── 消息内容
│   └── 跳转操作
├── 设置面板
│   ├── adapter 设置
│   ├── 隐私设置
│   ├── LLM 设置
│   ├── MCP / Skill 设置
│   └── 备份设置
└── 报告面板
    ├── 今日报告
    ├── 本周报告
    └── 导出 Markdown
```

---

## 3. 状态栏图标设计

### 3.1 默认状态

```text
┌──────────────────────── macOS menu bar ────────────────────────┐
│  Finder  File  Edit                         Memex 23  Wi-Fi  🔋 │
└────────────────────────────────────────────────────────────────┘
```

含义：

- `Memex`：应用标识。
- `23`：今天 AI/MCP/CLI 查询或采集相关调用次数。
- 无颜色异常：系统正常。

### 3.2 状态枚举

| 状态 | 图标文案 | 含义 | 点击后默认面板 |
|---|---|---|---|
| 正常 | `Memex 23` | daemon、SQLite、adapter 正常 | 主面板 |
| 正在采集 | `Memex ...` | Collector 正在处理新消息 | 主面板 |
| 有警告 | `Memex !` | 某个 adapter 失败或 Ollama 不可用 | 异常面板 |
| 暂停 | `Memex Paused` | 用户暂停采集 | 主面板 |
| 隐私锁定 | `Memex Private` | private mode 开启 | 主面板 |

### 3.3 右键菜单

```text
┌──────────────────────────┐
│ Open Memex               │
│ Search Memories          │
│ Pause Collection         │
│ Private Mode             │
├──────────────────────────┤
│ Rebuild Index            │
│ Run Doctor               │
├──────────────────────────┤
│ Preferences              │
│ Quit                     │
└──────────────────────────┘
```

---

## 4. 主面板原型

### 4.1 ASCII 原型

```text
╭────────────────────────────────────────────╮
│ Memex                                23    │
│ 本地记忆运行中 · 最后采集 2 分钟前          │
├────────────────────────────────────────────┤
│ 🔍 搜索 Cursor / Claude / Codex 历史...     │
├────────────────────────────────────────────┤
│ 今日                                      │
│  MCP 调用 12   CLI 搜索 8   新消息 184     │
├────────────────────────────────────────────┤
│ 最近 session                              │
│  Claude   Rust 主体迁移方案          5m   │
│  Cursor   SQLite FTS5 chunk 检索     1h   │
│  Codex    Tauri menubar 原型         3h   │
│  Claude   隐私 opt-in 设计           1d   │
│                                            │
│ [打开搜索]  [生成周报]  [设置]             │
╰────────────────────────────────────────────╯
```

### 4.2 功能说明

- 顶部显示系统状态和今日总数。
- 搜索框是最高优先级入口。
- 今日统计展示调用、搜索、新消息。
- 最近 session 支持点击进入详情。
- 底部保留三个常用操作：搜索、周报、设置。

### 4.3 交互规则

- 点击搜索框：切换到搜索面板。
- 点击最近 session：切换到详情面板。
- 点击生成周报：切换到报告面板。
- 点击设置：切换到设置面板。
- 按 `Esc`：关闭 menubar popup。

---

## 5. 搜索面板原型

### 5.1 ASCII 原型

```text
╭────────────────────────────────────────────╮
│ ← 搜索记忆                                  │
├────────────────────────────────────────────┤
│ redis pipeline 优化                         │
├────────────────────────────────────────────┤
│ [全部] [Claude] [Cursor] [Codex] [OpenCode] │
│ [7天] [30天] [全部时间]   [当前项目 ✓]       │
├────────────────────────────────────────────┤
│ 1. Cursor · 2 小时前                        │
│    Redis pipeline 批量写入优化              │
│    ...pipeline 批量执行 SET，吞吐提升...    │
│    match: title + code block + current dir  │
│                                            │
│ 2. Claude · 昨天                            │
│    缓存写入与错误恢复策略                   │
│    ...失败时回退单条写入并记录 dead letter...│
│    match: summary + recent                 │
│                                            │
│ 3. Codex · 3 天前                           │
│    benchmark 脚本调试                       │
│    ...redis pipeline benchmark...          │
│    match: code snippet                     │
╰────────────────────────────────────────────╯
```

### 5.2 功能说明

- 搜索输入 300ms debounce。
- 支持 adapter 过滤。
- 支持时间范围过滤。
- 支持“当前项目”过滤。
- 每条结果展示来源、时间、标题、snippet、match_reason。

### 5.3 交互规则

- 输入时自动触发搜索。
- 上下方向键切换结果。
- Enter 打开当前结果详情。
- `Cmd + Enter` 打开原始 session 文件。
- `Esc` 返回主面板。

---

## 6. 详情面板原型

### 6.1 ASCII 原型

```text
╭────────────────────────────────────────────╮
│ ← Session 详情                       打开  │
├────────────────────────────────────────────┤
│ Cursor · SQLite FTS5 chunk 检索             │
│ memex · /Users/example/projects/memex       │
│ 2026-05-31 10:12 · 47 messages · public     │
├────────────────────────────────────────────┤
│ 摘要                                       │
│ 讨论将 FTS 索引从 messages 调整到 chunks，  │
│ 以提升 snippet 定位和长消息 ranking 准确性。│
├────────────────────────────────────────────┤
│ 关键决策                                   │
│ ✓ FTS 建在 chunks 上                       │
│ ✓ Markdown 是主存储                        │
│ ✓ SQLite 可从 Markdown 重建                │
├────────────────────────────────────────────┤
│ 消息                                       │
│ User                                       │
│   为啥不要直接对 message 建索引？           │
│ Assistant                                  │
│   因为检索命中粒度会太粗，snippet 不稳定... │
│                                            │
│ [复制摘要] [在 IDE 打开] [标记 private]     │
╰────────────────────────────────────────────╯
```

### 6.2 功能说明

- 展示 session 元信息。
- 展示 L2 session 摘要。
- 展示关键决策列表。
- 展示原始消息内容。
- 支持复制摘要。
- 支持跳转 IDE。
- 支持标记 private。

### 6.3 交互规则

- 点击“打开”：打开 Markdown session 文件。
- 点击“在 IDE 打开”：通过 `memex://session/<id>` deep link 跳转。
- 点击“标记 private”：当前 session 从 MCP 搜索结果中移除。
- private 状态变更后立即更新 SQLite 和 Markdown frontmatter。

---

## 7. 设置面板原型

### 7.1 ASCII 原型

```text
╭────────────────────────────────────────────╮
│ ← 设置                                      │
├────────────────────────────────────────────┤
│ Adapter                                    │
│ [✓] Claude Code   ~/.claude/projects       │
│ [✓] Cursor        待确认路径               │
│ [✓] Codex         ~/.codex/sessions        │
│ [ ] OpenCode      ~/.opencode              │
├────────────────────────────────────────────┤
│ 隐私                                       │
│ [✓] 自动脱敏                               │
│ [✓] private session 不进入 MCP             │
│ [ ] Private Mode                           │
│ 自定义规则  redactions.yaml     [编辑]      │
├────────────────────────────────────────────┤
│ LLM                                        │
│ 本地摘要      Ollama qwen2.5:7b            │
│ [ ] Claude 云端兜底                        │
│     需要 ANTHROPIC_API_KEY 或 credentials   │
│     开启前会先脱敏，并只发送最小上下文      │
├────────────────────────────────────────────┤
│ 集成                                       │
│ Cursor MCP        已安装                   │
│ Claude Code Skill 未安装       [安装]      │
├────────────────────────────────────────────┤
│ 备份                                       │
│ [✓] 每周备份到 ~/.memex/backup             │
│ [ ] Git 自动 commit                        │
╰────────────────────────────────────────────╯
```

### 7.2 功能说明

- Adapter 区控制采集来源。
- 隐私区控制 redaction、private mode、MCP 可见性。
- LLM 区控制 Ollama 和 Claude fallback。
- 集成区安装 MCP / Skill。
- 备份区控制本地备份和 Git 备份。

### 7.3 Claude fallback 开启确认

```text
╭────────────────────────────────────────────╮
│ 开启 Claude 云端兜底？                      │
├────────────────────────────────────────────┤
│ 开启后，Memex 会在本地检索和 Ollama 都无法  │
│ 给出结果时，把已脱敏的最小必要上下文发送到  │
│ Claude API。                               │
│                                            │
│ 默认不会发送 private session。              │
│ 需要先配置 Anthropic API key。              │
│ 你可以随时在设置中关闭。                    │
│                                            │
│ [取消]                         [确认开启]   │
╰────────────────────────────────────────────╯
```

---

## 8. 报告面板原型

### 8.1 ASCII 原型

```text
╭────────────────────────────────────────────╮
│ ← 本周报告                                  │
├────────────────────────────────────────────┤
│ 时间范围  2026-05-25 至 2026-05-31          │
│ Sessions 128   Messages 3,420   Tools 512  │
├────────────────────────────────────────────┤
│ 本周主题                                   │
│ 1. Memex v4 架构收敛                       │
│ 2. MCP 永远 spawn CLI                      │
│ 3. chunk 级 FTS 设计                       │
│ 4. menubar 原型设计                        │
├────────────────────────────────────────────┤
│ 关键决策                                   │
│ ✓ CLI 是稳定契约层                         │
│ ✓ Claude fallback 必须 opt-in              │
│ ✓ Markdown 是主存储，SQLite 可重建          │
├────────────────────────────────────────────┤
│ [复制 Markdown] [导出报告] [重新生成]       │
╰────────────────────────────────────────────╯
```

### 8.2 功能说明

- 展示本周 AI 工作概览。
- 聚合主题、决策、风险、待办。
- 支持复制 Markdown。
- 支持导出到 `~/.memex/reports`。

---

## 9. 异常面板原型

### 9.1 ASCII 原型

```text
╭────────────────────────────────────────────╮
│ Memex 需要处理 2 个问题                     │
├────────────────────────────────────────────┤
│ ! Cursor adapter 读取失败                   │
│   路径 ~/.cursor/chats 不存在               │
│   [重新检测] [打开设置]                     │
│                                            │
│ ! Ollama 不可用                             │
│   http://localhost:11434 无响应             │
│   当前退化为 FTS5 / metadata / 已有摘要      │
│   [重试] [配置 Claude Key] [仅使用 FTS5]    │
├────────────────────────────────────────────┤
│ [运行 doctor]                    [忽略一次] │
╰────────────────────────────────────────────╯
```

### 9.2 功能说明

- 聚合 adapter、SQLite、daemon、Ollama、MCP 的异常。
- 每个异常提供直接操作。
- 不把异常藏在日志里，优先给用户可执行修复入口。

---

## 10. 功能到面板映射

| 功能 | 所属面板 | 用户操作 | 后端调用 |
|---|---|---|---|
| 查看今日统计 | 主面板 | 打开 menubar | `get_stats` |
| 查看最近 session | 主面板 | 打开 menubar | `list_recent_sessions` |
| 搜索历史 | 搜索面板 | 输入关键词 | `search_memex` |
| 查看详情 | 详情面板 | 点击结果 | `get_session` |
| 打开原始文件 | 详情面板 | 点击打开 | `open_session_file` |
| 标记 private | 详情面板 | 点击标记 | `mark_private` |
| 配置 adapter | 设置面板 | 勾选开关 | `update_config` |
| 开启 Claude fallback | 设置面板 | 确认开启 | `update_llm_config` |
| 安装 MCP | 设置面板 | 点击安装 | `setup_mcp` |
| 生成报告 | 报告面板 | 点击生成 | `generate_report` |
| 运行 doctor | 异常面板 | 点击运行 | `run_doctor` |

---

## 11. Tauri IPC 接口

```text
get_stats() -> Stats
list_recent_sessions(limit: number) -> SessionSummary[]
search_memex(query: string, filters: SearchFilters) -> SearchResult[]
get_session(session_id: string) -> SessionDetail
open_session_file(session_id: string) -> void
mark_private(session_id: string, private: boolean) -> void
get_config() -> Config
update_config(patch: ConfigPatch) -> Config
setup_mcp(target: "cursor" | "claude-code") -> SetupResult
generate_report(range: "today" | "week") -> Report
run_doctor() -> DoctorResult
```

---

## 12. 视觉与交互规范

### 12.1 尺寸

- 默认宽度：420px。
- 最小高度：360px。
- 最大高度：720px。
- 内容超出时内部滚动。

### 12.2 快捷键

| 快捷键 | 行为 |
|---|---|
| `Cmd + Shift + M` | 打开 menubar 搜索 |
| `Esc` | 关闭当前面板或返回上一级 |
| `Enter` | 打开选中的搜索结果 |
| `Cmd + Enter` | 在 IDE 打开选中的 session |
| `Cmd + ,` | 打开设置 |

### 12.3 状态反馈

- 搜索中：显示 skeleton 或 loading row。
- 无结果：给出修改关键词、放宽时间范围、关闭当前项目过滤的建议。
- adapter 异常：在主面板顶部显示轻量提示。
- daemon 异常：CLI fallback 仍可用时显示 warning，不阻断搜索。

---

## 13. 开发优先级

### P0

- 状态栏图标。
- 主面板。
- 搜索面板。
- 详情面板。
- 设置里的 adapter 和隐私开关。

### P1

- 报告面板。
- 异常面板。
- MCP / Skill 安装入口。
- Claude fallback 确认弹层。

### P2

- Git 自动备份开关。
- 高级过滤。
- 自定义快捷键。
- 多主题。
