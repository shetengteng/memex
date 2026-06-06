# Memex UI 重新设计提案 v1（shadcn-vue 重构）

> 日期：2026-06-06
> 状态：草稿（待用户确认）
> 范围：仅 `tauri-app/src/` 内 UI/IA 重构，不触碰 `memex-core` / `memex-daemon` / MCP 协议 / 数据模型
> 参考：[shadcn-vue blocks](https://www.shadcn-vue.com/blocks) / [shadcn-vue components](https://www.shadcn-vue.com/docs/components)

---

## 一、范围（先对齐再开工）

**本轮只做**：UI/IA 重构 + shadcn-vue 组件化升级。

**不做**：新统计字段、新后端能力、新 LLM 流程。如果新设计需要后端补字段，本文末会单独标出来等决策。

---

## 二、现状盘点

### 2.1 两种交互形态

| 形态 | 入口 | 现状 |
|---|---|---|
| **Popup（菜单栏弹窗）** | `App.vue` | 顶部搜索框 + 中间 SearchView/Settings/Status/Session + 底部状态栏（统计 + Ollama 进度 + 4 个导航按钮） |
| **Dashboard（独立窗口）** | `dashboard/index.vue` | 左侧 `DashSidebar`（7 项）+ 右侧 7 个 Tab：Overview / Sessions / Projects / Reports / Reflect / Workload / Search / SessionDetail |

### 2.2 已使用的 shadcn-vue 组件（11 个）

`tooltip` · `dialog` · `select` · `collapsible` · `separator` · `badge` · `button` · `input` · `switch` · `toggle-group` · `card`

**未使用的旗舰组件**：`sidebar` / `command` / `sheet` / `tabs` / `skeleton` / `scroll-area` / `progress` / `dropdown-menu` / `avatar` / `resizable` / `breadcrumb` / `checkbox` / `radio-group` / `sonner` / `calendar`

### 2.3 核心痛点

| # | 痛点 | 影响 |
|---|---|---|
| 1 | 7 个并列 Tab 太多，心智模型不清 | Overview / Sessions / Projects / Workload 都在"看数据"，Reports / Reflect 都在"看 LLM 总结"，分类维度混乱 |
| 2 | "我今天干了啥 / 接下来能干啥" 缺明确入口 | Overview 是死板 KPI，不回答用户打开 Memex 时的第一个问题 |
| 3 | Sessions Tab 像 DBA 表格 | 6 列 + 4 个 Select + 分页，90% 用户只想按时间扫一眼 |
| 4 | Reports vs Reflect 概念高度重叠 | 都是 LLM 生成 Markdown，用户分不清何时点哪个 |
| 5 | MCP / IDE 集成是核心价值，但深埋 Settings 二级菜单 | 新用户根本看不到 |
| 6 | Search 体验割裂 | popup 是命令面板风格、dashboard 又是另一套，缺少全局 ⌘K |
| 7 | 只用了 11 个 shadcn-vue 基础组件 | block 级组件几乎没用上 |

---

## 三、新信息架构（IA）

**核心动作**：7 tab → **5 一级菜单 + 1 全局命令面板**

```
┌─ Dashboard ─────────────────────────────────────────────────┐
│                                                             │
│  ⌘K 全局命令面板（Command）—— 跨页面唤起                    │
│                                                             │
│  Sidebar（shadcn-vue Sidebar block）                        │
│  ├─ ◉ Today          ← 新增 / 替代 Overview                 │
│  ├─ ◯ Library        ← 合并 Sessions + Projects             │
│  ├─ ◯ Insights       ← 合并 Reports + Reflect + Workload    │
│  ├─ ◯ Connect        ← 新增（MCP / Adapter / IDE 状态）     │
│  └─ ◯ Settings       ← 保留（LLM Provider / 偏好 / 关于）   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 3.1 演进映射

| 旧 | 新 | 为什么 |
|---|---|---|
| Overview（数字仪表盘） | **Today**（活动叙事） | 用 narrative 而非 dashboard 回答"我在干啥" |
| Sessions + Projects 各一个 tab | **Library**（一个 tab，多视图切换） | 都是"我的对话资料库"，只是分组维度不同 |
| Reports + Reflect + Workload 各一个 tab | **Insights**（一个 tab，3 个二级 tab） | 都是"对历史的二次加工与洞察" |
| Settings 里藏着 IDE Integrations | **Connect**（独立一级菜单） | MCP 接入是核心卖点，要前置 |
| Search 在 popup 和 dashboard 各一份 | **⌘K 全局命令面板** | 统一搜索入口，符合现代生产力软件惯例 |
| 7 tab | **5 tab + 1 命令面板** | 减少认知负担 |

---

## 四、ASCII 原型（按页面）

### 4.1 Dashboard 整体骨架

```
┌───────────────────────────────────────────────────────────────────────────┐
│ ⌘ Memex                            [⌘K Search anything…]   [●Live] [👤]   │
├──────────────┬────────────────────────────────────────────────────────────┤
│              │                                                            │
│  ◉ Today     │   <主内容区，对应右侧每个 tab 的 ASCII 见 4.2 ~ 4.6>       │
│  ─────────   │                                                            │
│  ◯ Library   │                                                            │
│  ◯ Insights  │                                                            │
│  ─────────   │                                                            │
│  ◯ Connect   │                                                            │
│  ◯ Settings  │                                                            │
│              │                                                            │
│  ─────────   │                                                            │
│  📦 Today    │                                                            │
│  47 sessions │                                                            │
│  qwen2.5 OK  │                                                            │
│  ─────────   │                                                            │
│  v0.2.0      │                                                            │
└──────────────┴────────────────────────────────────────────────────────────┘
```

| 区域 | shadcn-vue 组件 |
|---|---|
| Sidebar | `Sidebar`, `SidebarHeader`, `SidebarContent`, `SidebarMenu`, `SidebarMenuItem`, `SidebarFooter`, `SidebarTrigger` |
| Header | `Button` (ghost) + `Badge` + `Avatar` |
| 全局搜索触发 | `Button` (outline) 点击打开 `CommandDialog` |
| 主内容滚动 | `ScrollArea` |

---

### 4.2 Today 页（默认首页，替代 Overview）

**理念**：从"展示数字"改成"叙事 + 行动"。用户打开第一眼应看到的是 *"你今天 / 这周做了什么、什么需要你关注、可以继续什么"*，而不是死板总计。

```
┌────────────────────────────────────────────────────────────────────┐
│  早上好，Terrell                                          ⟳ Refresh │
│  今天是 2026 年 6 月 6 日 周六 · 上次采集 2 分钟前                  │
│                                                                    │
│ ┌──────────────────────────────────────────────────────────────┐  │
│ │  📍 你今天的活动                                              │  │
│ │                                                              │  │
│ │  ▂▃▅▇▆▄▂▁  12 sessions · 348 msgs · 4 projects · 跨 3 个工具  │  │
│ │  ▔▔▔▔▔▔▔▔                                                   │  │
│ │  最活跃 14:00–16:00 · 主要 in memex (8) / tt-projects (3)    │  │
│ └──────────────────────────────────────────────────────────────┘  │
│                                                                    │
│ ┌──────────────────────────┐  ┌──────────────────────────────┐    │
│ │ 🧠 本周自动摘要 (L3)      │  │ 🔥 等待你的反思 (L4)          │   │
│ │                          │  │                              │    │
│ │ Week 23 · 67 sessions   │  │ ✦ 上周反思未读 (12 digests)   │    │
│ │ ────────────────────    │  │   "shadcn 重构 + MCP …"      │    │
│ │ 主线：tauri 迁移、MCP …  │  │ ─────────────────────────    │    │
│ │ 关键决策：3 条           │  │ ✦ 5 月反思未生成              │    │
│ │   • 用 Sidebar block …  │  │   [+ 立即生成]                │    │
│ │   • 重构 7 → 5 tab …   │  │                              │    │
│ │   • Search → ⌘K …      │  │                              │    │
│ │ [查看完整摘要 →]         │  │ [全部反思 →]                  │    │
│ └──────────────────────────┘  └──────────────────────────────┘    │
│                                                                    │
│ ┌──────────────────────────────────────────────────────────────┐  │
│ │  💡 接着想想？（智能续接）                                    │  │
│ │                                                              │  │
│ │  ┌──────────────────────────────────────────────────┐       │  │
│ │  │ memex · Cursor · 2 小时前                         │       │  │
│ │  │ "Memex menubar shadcn 重构原型"                   │       │  │
│ │  │ 中断点：在讨论 Tab → Sidebar 改造                  │       │  │
│ │  │ [继续会话 ↗]  [发送到 IDE]  [归档]                 │       │  │
│ │  └──────────────────────────────────────────────────┘       │  │
│ │  ┌──────────────────────────────────────────────────┐       │  │
│ │  │ tt-projects · Claude Code · 昨天                  │       │  │
│ │  │ "AsyncMQ 集成方案确认"                            │       │  │
│ │  │ [继续会话 ↗]  ...                                │       │  │
│ │  └──────────────────────────────────────────────────┘       │  │
│ └──────────────────────────────────────────────────────────────┘  │
│                                                                    │
│ ┌──────────────────────────────────────────────────────────────┐  │
│ │  ⚙ 系统状态                                                  │  │
│ │  Daemon ● Running · Adapter 5/7 active · LLM Ollama:qwen2.5 │  │
│ │  存储 ~/.memex/ 824 MB · FTS 索引 health ●                   │  │
│ │  [打开 Connect →]                                            │  │
│ └──────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────┘
```

| 区域 | shadcn-vue 组件 | 数据源 |
|---|---|---|
| 顶部叙事 | `Card` + 极简 inline chart（保留现有 timeline 缩小版） | 现 `getStats` + `getBreakdown` |
| Week 摘要 | `Card` + `Separator` + `Badge` | 现 `listReports('weekly', 1)` |
| 待反思 | `Card` + `Button` | 现 `reflectList()` |
| 智能续接 | `Card` 列表 + `Button` group | 现 `listRecent(N)` + 简单前端过滤 |
| 系统状态 | `Collapsible` + `Badge` | 现 `getStats` / Daemon status |

**新增小功能**：
- ❶ "智能续接" 列表（前端实现，先用最近 5 个 session，未来可接 LLM 判断"中断点"）
- ❷ "Today" 时间维度的活动汇总（前端用现有 timeline 数据过滤当天）

---

### 4.3 Library 页（合并 Sessions + Projects）

```
┌────────────────────────────────────────────────────────────────────┐
│  Library                                                           │
│  你的全部对话记忆 · 6,521 sessions · 184,209 messages              │
├────────────────────────────────────────────────────────────────────┤
│  [Sessions]  [Projects]  [Threads]   ← shadcn Tabs                 │
├────────────────────────────────────────────────────────────────────┤
│ ┌────────────┬──────────────────────────────────────────────────┐  │
│ │ 过滤 (左)   │  搜索 + 视图切换 (顶部)                          │  │
│ │            │  ┌────────────────────────────┬──────┬────────┐ │  │
│ │ 🛠 工具     │  │ Search title/snippet…      │ ⊞ ☰ ▤│ ⇅ Sort │ │  │
│ │ ☑ Cursor   │  └────────────────────────────┴──────┴────────┘ │  │
│ │ ☑ Claude   │                                                  │  │
│ │ ☐ Codex    │  ┌─────────────────────────────────────────────┐ │  │
│ │ ☐ OpenCode │  │ ▣ memex · Cursor · 14:32                     │ │  │
│ │            │  │ "Memex menubar shadcn 重构原型"               │ │  │
│ │ 📁 项目     │  │ 42 msgs · L2 summary ✓ · 主题: ui, vue, shadcn│ │  │
│ │ ☑ memex    │  ├─────────────────────────────────────────────┤ │  │
│ │ ☑ tt-proj  │  │ ▣ memex · Cursor · 12:48                     │ │  │
│ │            │  │ "shadcn-vue blocks 调研"                      │ │  │
│ │ 📅 时间     │  │ 28 msgs · L2 summary ✓                       │ │  │
│ │ ○ Today    │  ├─────────────────────────────────────────────┤ │  │
│ │ ◉ 7d       │  │ ▣ tt-projects · Claude Code · 昨天 22:11      │ │  │
│ │ ○ 30d      │  │ "AsyncMQ 集成方案"                            │ │  │
│ │ ○ 自定义    │  │ ...                                          │ │  │
│ │            │  └─────────────────────────────────────────────┘ │  │
│ │ ✨ 摘要状态  │  ← infinite scroll，不再分页                     │  │
│ │ ◉ 全部     │                                                  │  │
│ │ ○ 仅已摘要 │  ┌─ 详情侧抽屉（Sheet，点 row 才出现） ──────┐    │  │
│ │ ○ 仅可摘要 │  │ "Memex menubar shadcn 重构原型"            │    │  │
│ │            │  │ ────────────────────────────────────────  │    │  │
│ │ [清除筛选] │  │ Cursor · memex · 2026-06-06 14:32          │    │  │
│ │            │  │ 42 messages · L2 summary 完成              │    │  │
│ └────────────┘  │                                            │    │  │
│                 │ 📝 LLM 摘要                                │    │  │
│                 │ 围绕将现有 7 tab 重构为 ...                 │    │  │
│                 │                                            │    │  │
│                 │ 🏷 主题: ui, shadcn, vue, sidebar           │    │  │
│                 │ 📌 决策: 收敛 7→5 tab、用 ⌘K …             │    │  │
│                 │                                            │    │  │
│                 │ [打开完整会话]  [发送到 Cursor]  [归档]     │    │  │
│                 └────────────────────────────────────────────┘    │  │
└────────────────────────────────────────────────────────────────────┘
```

**关键升级**：
- 把现有 6 列 Table 换成 **List/Card/Compact 三视图切换**（Cards 默认）
- 把现有 4 个独立 Select 收成 **左侧 Facets 面板**（Checkbox group + RadioGroup），更直观、支持多选
- **点击 row 不跳转新页面**，而是右侧弹 `Sheet` drawer 预览详情，预览满意再点 "打开完整会话"
- 滚动改 infinite scroll + virtualization，取消硬分页
- 新增 **Threads 子 tab**（占位）：未来按"主题 thread"聚合跨 session 的对话（需后端配合，先留 UI 入口）

| 区域 | shadcn-vue 组件 |
|---|---|
| 子 tab | `Tabs` + `TabsList` + `TabsTrigger` |
| 左侧 Facets | `Sidebar` 二级 / `Checkbox` + `RadioGroup` + `Calendar`（自定义时间段） |
| 顶部工具栏 | `Input` + `ToggleGroup`（视图切换） + `DropdownMenu`（排序） |
| List 卡片 | `Card` + `Badge` + `Avatar`（IDE 图标） |
| 详情抽屉 | `Sheet`（侧滑 drawer） + `Separator` + `Button` group |
| 无限滚动 | `ScrollArea` + 自定义 IntersectionObserver |
| 加载骨架 | `Skeleton` |

---

### 4.4 Insights 页（合并 Reports + Reflect + Workload）

```
┌────────────────────────────────────────────────────────────────────┐
│  Insights                                                          │
│  AI 帮你回顾、提炼、找规律                                          │
├────────────────────────────────────────────────────────────────────┤
│  [📰 Reports]  [🪞 Reflections]  [📊 Trends]   ← shadcn Tabs       │
├────────────────────────────────────────────────────────────────────┤
│                                                                    │
│  〔Reports tab〕                                                    │
│  ┌────────────┬───────────────────────────────────────────────┐    │
│  │ Daily ◉   │  2026-06-06 周六                  ⟳ 重新生成   │    │
│  │ Weekly ○  │  ─────────────────────────────────────         │    │
│  │ Monthly ○ │  ## 主要工作                                   │    │
│  │           │  - 完成 Memex 7→5 tab 重构原型                 │    │
│  │ ─────── │  - 调研 shadcn-vue blocks                       │    │
│  │ 06-06 12 │  ## 关键决策                                   │    │
│  │ 06-05 38 │  - 引入 Sidebar block 替代手写 nav             │    │
│  │ 06-04 24 │  ## 主题                                       │    │
│  │ 06-03 41 │  [ui] [shadcn] [vue] [refactor]                │    │
│  │ ...      │                                                │    │
│  └──────────┴────────────────────────────────────────────────┘    │
│                                                                    │
│  〔Reflections tab〕                                                │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │ 触发新反思:                                                  │  │
│  │ 时间范围 [近 7 天 ▼]   [✨ 让 AI 反思一下…]                  │  │
│  ├──────────────────────────────────────────────────────────────┤  │
│  │ Week 23 · 67 sessions   2026-06-06 09:12                     │  │
│  │ ▸ 你这周在 …                                                  │  │
│  ├──────────────────────────────────────────────────────────────┤  │
│  │ Week 22 · 89 sessions   ...                                  │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                    │
│  〔Trends tab〕  （把现有 Workload 的可视化挪进来）                  │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │ 范围 [7d] [30d] [90d]                                        │  │
│  │ ──────────────────────────────────                           │  │
│  │ KPI: 1,234 sessions · 28,910 msgs · 27 active days           │  │
│  │ ─────────────                                                │  │
│  │ 📅 GitHub 风格日历热图（保留现有实现）                       │  │
│  │ 🕒 7 × 24 习惯热图（保留现有实现）                            │  │
│  │ 🛠 工具占比 / 📁 项目 Top10（保留现有实现）                  │  │
│  └──────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────┘
```

**关键升级**：
- 三种 L3/L4 输出整合到一个 Insights 入口，用 Tabs 切换
- Reports 加 **Monthly** 维度（未来后端支持）
- Reflections 文案优化（"让 AI 反思一下"比"运行 reflect"更友好）
- Trends 沿用 Workload 现有可视化，包装到统一容器

| 区域 | shadcn-vue 组件 |
|---|---|
| 二级 Tabs | `Tabs` |
| Reports 左侧时间列表 | `ScrollArea` + `Button` (ghost) |
| Reports 详情 | `Card` + Markdown 渲染（现有） + `Badge` |
| Reflections 触发条 | `Select` + `Button` + `Sparkles` icon |
| Trends KPI | `Card` |
| 日历热图 / 24×7 热图 | 沿用现有 table 实现 + `Tooltip` |

---

### 4.5 Connect 页（新增，把 IDE / MCP 推到一级）

```
┌────────────────────────────────────────────────────────────────────┐
│  Connect                                                           │
│  让你的 AI 编辑器记住一切                                           │
├────────────────────────────────────────────────────────────────────┤
│                                                                    │
│  📡 Adapters（数据采集源）                                          │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │ ✦ Claude Code      ●Active     3,210 sessions    [⚙]         │  │
│  │ ✦ Cursor           ●Active     2,148 sessions    [⚙]         │  │
│  │ ✦ Codex            ○Disabled       0 sessions    [启用]      │  │
│  │ ✦ OpenCode         ●Active        842 sessions    [⚙]         │  │
│  │ ✦ Aider            ○Disabled                     [启用]      │  │
│  │ ✦ Continue         ○Disabled                     [启用]      │  │
│  │ ✦ Cline            ○Disabled                     [启用]      │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                    │
│  🧩 IDE 集成（MCP + SKILL）                                         │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │ ✦ Cursor                                                     │  │
│  │   MCP: ●已安装 (/Users/.../mcp.json)   SKILL: ●已安装         │  │
│  │   [测试连接]  [重新安装]  [卸载]                               │  │
│  │                                                              │  │
│  │ ✦ Claude Code                                                │  │
│  │   MCP: ●已安装   SKILL: ○未安装                              │  │
│  │   [安装 SKILL]                                                │  │
│  │                                                              │  │
│  │ ✦ Codex                                                      │  │
│  │   MCP: ○未安装   SKILL: ○未安装                              │  │
│  │   [一键接入]                                                  │  │
│  │                                                              │  │
│  │ ✦ OpenCode                                                   │  │
│  │   ...                                                        │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                    │
│  🔌 MCP 工具与活动                                                  │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │ 暴露给 AI 的 4 个工具                                         │  │
│  │ ├─ search_memory     最近 24h 被调用 47 次                    │  │
│  │ ├─ get_session       最近 24h 被调用 12 次                    │  │
│  │ ├─ list_recent       最近 24h 被调用 8 次                     │  │
│  │ └─ stats             最近 24h 被调用 3 次                     │  │
│  │ [查看实时日志 →]                                              │  │
│  └──────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────┘
```

**为什么独立成一级菜单**：
- Memex 最大价值 = "给你的 AI 编辑器装记忆"。这事埋在 Settings 二级，新用户根本发现不了
- 第一段 Adapters 解决"哪些 IDE 的历史进 Memex"
- 第二段 IDE 集成解决"哪些 IDE 能用 Memex 的记忆"
- 第三段 MCP 活动给出 *最有说服力的反馈*：你的 AI 真的在用 Memex

| 区域 | shadcn-vue 组件 |
|---|---|
| Adapter 行 | `Card`（外层） + 自定义 row + `Switch` + `Badge` |
| IDE 卡片 | `Card` + `Badge` + `Button` group |
| MCP 工具调用统计 | `Card` + `Progress`（小） |

> ⚠ "MCP 调用统计" 是**新功能**，需要 daemon 加一张表记录 `mcp_call_log`（method, ts, latency）。如果不想做后端改动，先在 UI 上画占位 "Coming soon"。

---

### 4.6 全局 Command Palette（⌘K）

```
┌────────────────────────────────────────────────────────────────────┐
│  ⌘ Search anything…                                       Esc 关闭 │
├────────────────────────────────────────────────────────────────────┤
│                                                                    │
│  🔍 搜索结果（输入后才显示）                                        │
│  ▸ "shadcn 重构原型"  · memex · Cursor · 2h ago                    │
│  ▸ "AsyncMQ 集成方案"  · tt-proj · Claude Code · 昨天               │
│                                                                    │
│  ⚡ 快速操作                                                        │
│  ▸ 跳到 Today                                              ⌘1     │
│  ▸ 跳到 Library                                            ⌘2     │
│  ▸ 跳到 Insights                                           ⌘3     │
│  ▸ 跳到 Connect                                            ⌘4     │
│  ▸ 立即采集一次                                            ⌘R     │
│  ▸ 生成本周反思                                                    │
│  ▸ 复制 MCP 配置 (Cursor)                                          │
│                                                                    │
│  ⏱ 最近会话                                                        │
│  ▸ memex · Cursor · 14:32                                          │
│  ▸ memex · Cursor · 12:48                                          │
│                                                                    │
│  📁 项目                                                            │
│  ▸ memex                                                           │
│  ▸ tt-projects                                                     │
└────────────────────────────────────────────────────────────────────┘
```

| 区域 | shadcn-vue 组件 |
|---|---|
| 全部 | `Command` + `CommandDialog` + `CommandInput` + `CommandList` + `CommandGroup` + `CommandItem` + `CommandShortcut` + `CommandSeparator` |

**触发方式**：
- 任意页面 `⌘K` / `Ctrl+K` 打开
- 也是顶部 Header 那个搜索按钮
- 取代现在 popup 的 SearchTab + dashboard 的 SearchTab，**两边代码合并**

---

### 4.7 Popup（菜单栏弹窗）—— 极简化

```
┌─────────────────────────────────────────────┐
│ ⌘ Memex                          [⌘K] [⚙]   │
├─────────────────────────────────────────────┤
│                                             │
│  最近会话                                    │
│  ┌─────────────────────────────────────┐    │
│  │ ▣ memex · Cursor · 14:32             │    │
│  │   "shadcn 重构原型"                  │    │
│  ├─────────────────────────────────────┤    │
│  │ ▣ memex · Cursor · 12:48             │    │
│  │   "blocks 调研"                      │    │
│  ├─────────────────────────────────────┤    │
│  │ ▣ tt-proj · Claude · 昨天            │    │
│  └─────────────────────────────────────┘    │
│                                             │
├─────────────────────────────────────────────┤
│ ● 6,521 · qwen2.5 98%  [⛭] [📊 Dashboard ↗] │
└─────────────────────────────────────────────┘
```

**变化**：
- Popup 只保留 "**快速访问 + 跳板**" 角色：最近会话 / 打开 Dashboard / 打开 Settings
- 搜索完全交给 `⌘K`（无论在 popup 还是 dashboard 都同一套）
- 去掉 popup 内的 Status 子视图，挪到 Today 页底部的 "系统状态" 折叠面板

| 区域 | shadcn-vue 组件 |
|---|---|
| Header | `Button` (ghost) + 简单 logo |
| 列表 | `ScrollArea` + 自定义 row |
| Footer | `Separator` + `Badge` + `Button` |

---

## 五、需要补充安装的 shadcn-vue 组件

| 组件 | 用途 | 优先级 |
|---|---|---|
| `sidebar` | Dashboard 主导航（block 级旗舰组件） | **P0** |
| `command` | ⌘K 全局命令面板 | **P0** |
| `sheet` | Session 详情侧抽屉 | **P0** |
| `tabs` | Library / Insights 子 tab | **P0** |
| `skeleton` | 加载骨架（取代手写 animate-pulse） | **P0** |
| `scroll-area` | 自定义滚动条 | P1 |
| `progress` | LLM 摘要进度（取代手写 div） | P1 |
| `dropdown-menu` | 排序、更多操作菜单 | P1 |
| `avatar` | IDE 图标圆形容器 | P1 |
| `checkbox` | Facets 多选 | P1 |
| `radio-group` | Facets 单选 | P1 |
| `breadcrumb` | Session Detail 面包屑 | P2 |
| `resizable` | Library 左右可调 | P2 |
| `sonner` 或 `toast` | 操作反馈 | P2 |
| `calendar` + `popover` | 自定义时间范围 | P2 |

---

## 六、迁移路径（建议分 4 个 PR）

| 阶段 | 范围 | 估时 |
|---|---|---|
| **Phase 1** | 装 P0 组件 + 用 `Sidebar` block 重构 Dashboard 外壳 + 引入 ⌘K Command Palette（先复用现有 search 接口） | 1 天 |
| **Phase 2** | 新建 **Today** 页替代 Overview（叙事卡 + 复用 Reports/Reflect 数据） | 0.5 天 |
| **Phase 3** | 合并 Sessions + Projects → **Library**（Tabs + Facets + Sheet drawer） | 1 天 |
| **Phase 4** | 合并 Reports + Reflect + Workload → **Insights** + 独立 **Connect** 页（搬移现有 Settings 内容） | 1 天 |

每个 Phase 独立可发布，不会一次性大爆炸。

---

## 七、需要确认的关键决策

为避免做了一半返工，请逐项给回复（一句话即可）：

1. **5 tab IA（Today / Library / Insights / Connect / Settings）你认同吗？** 还是想要别的切分？
2. **Today 页用"叙事 + 行动"代替 KPI 仪表盘**，你认同吗？还是想保留 Overview 的硬核数字？
3. **⌘K 全局命令面板**取代现在 popup 和 dashboard 各自的 search，你接受吗？
4. **Connect 提到一级菜单**（IDE 集成 + MCP 调用统计），你认同吗？
5. **MCP 调用统计**是新功能（要 daemon 加 log 表），要做吗？还是 UI 占位等以后？
6. **Library 用 Card 视图为主 + 详情走 Sheet drawer**（取代当前 Table 跳页），你接受吗？
7. **Threads 子 tab** 暂时占位还是不要？
8. 你希望整体风格更接近 shadcn-vue 哪个 block？
   - Dashboard-01（标准 SaaS 仪表盘）
   - Sidebar-07（多层级 collapsible nav）
   - 还是参考某个具体 URL？

---

## 八、与现有设计文档的关系

| 文档 | 状态 | 备注 |
|---|---|---|
| `20260531-03-Memex-v4最终设计文档.md` | 不冲突 | 后端架构、数据模型不变 |
| `20260531-09-Memex-menubar-交互原型-v3.html` | 部分替代 | popup 单 popup 聚焦版思路保留，但 search 改 ⌘K |
| `20260601-01-Memex-WebUI-Popup内嵌设计.md` | 部分替代 | popup 极简化，dashboard 在独立窗口承载所有重型操作 |
| `20260602-01-Memex-v4功能点开发清单-100%.md` | 不冲突 | 功能 100% 保留，仅 UI/IA 重构 |

---

## 九、风险与不做的事

**风险**：
- `sidebar` block 的实现依赖 `useSidebar` 等 composable，需要在 `App.vue` 顶层 wrap 一次 `SidebarProvider`，会动到根组件
- `command` 组件需要装 `cmdk-vue` 或类似底层，确认 shadcn-vue 当前是否已自带（确认后再开 Phase 1）
- `sheet` 在 Tauri 窗口里需要确认 z-index 与现有 dialog 不冲突

**本轮明确不做**：
- 不动 `memex-core` Rust 代码
- 不改 IPC contract（`invoke` 的命令名 / 参数）
- 不动数据库 schema
- 不引入新的状态管理库（继续用 composable + ref）
- 不接入 i18n 之外的新 SDK

---

> 等你在 `prompts.txt` 给出第七节 8 个问题的回复后，按 Phase 1→4 分批落地。
