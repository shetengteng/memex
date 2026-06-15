# Memex 使用说明 / Onboarding 视图设计

> 日期：2026-06-15
> 状态：草稿（待用户确认）
> 范围：仅 `app/desktop/` 内新增 `/help` 视图 + i18n + 辅助入口；不触碰 `memex-core` / daemon / MCP 协议 / 数据模型
> 参考：[shadcn-vue tabs](https://www.shadcn-vue.com/docs/components/tabs) · [shadcn-vue accordion](https://www.shadcn-vue.com/docs/components/accordion) · 现有 `views/settings/` Tab 模式 · `.cursor/rules/memex.mdc`

---

## 一、范围（先对齐再开工）

**本轮做**：

- 新增 `/help` 路由 + 容器视图，按 Tabs 拆 7 章使用说明
- 在侧栏 / Connect 空状态 / Today 空状态 提供进入入口
- i18n key 命名空间 `help.*`，中英双语全量文案
- 单元测试覆盖路由 + 每章组件渲染 + i18n key 完整性

**本轮不做**：

- 不做"首次启动强制 Onboarding 弹窗"（理由见 §3.4）
- 不做内置交互式 walkthrough / spotlight tour（M3 之后再单独评估）
- 不复制 README 全文进 app（README 偏长且面向 GitHub 浏览，受众不同）
- 不写视频版（已有 `docs/promo.html` 落地页视频版，使用说明聚焦"装上之后怎么用"）

---

## 二、现状盘点

### 2.1 当前可用入口

| 文档 / 入口 | 受众 | 内容形态 |
|---|---|---|
| `README.md` / `README.en.md` | GitHub 浏览的潜在用户 | 长文 Markdown，包含 Cask 安装 / 架构 / Skill 列表 |
| `docs/index.html` + `docs/promo.html` | 还没装的潜在用户 | 静态落地页 + 1 分钟视频走查 |
| `app/SKILL.md` | AI Agent | 给 AI 看的工具说明，不给人看 |
| `.cursor/rules/memex.mdc` | 在 Cursor 里用 memex MCP 的 AI | 强制规则，不给人看 |
| **缺失** | **已经装了 Memex，但不知道怎么把它接到 IDE / 怎么搜索 / 怎么排障的人** | **本设计要补的位** |

### 2.2 用户最近反馈的痛点（来自 prompts.txt 历史）

- "IDE 集成 0 / 0 已接入" 不知道下一步怎么操作
- 重置数据库后报错 `unable to open database file: Error code 14`，不知道这是预期还是 bug
- 想看 MCP 自然语言示例，但 .mdc 文件不在 app 里展示
- 不知道有 SKILL，更不知道触发关键词

### 2.3 已有可复用资产

| 资产 | 用途 | 文件 |
|---|---|---|
| `IdeIntegrationsCard` | 一键安装 MCP / SKILL / hooks 到 IDE | `views/connect/components/IdeIntegrationsCard.vue` |
| `AdaptersCard` | 检测 IDE 是否在用 | `views/connect/components/AdaptersCard.vue` |
| `McpActivityCard` | 24h MCP 调用统计 | `views/connect/components/McpActivityCard.vue` |
| `OllamaSetupDialog` | 引导用户配置本地 LLM | `components/shell/OllamaSetupDialog.vue` |
| Settings Tabs 模式 | Tab 切换 + `?tab=xxx` query 持久化 | `views/settings/index.vue` |

`/help` 不重复实现这些卡片，而是用 `<router-link>` 引到对应入口。

---

## 三、信息架构（IA）

### 3.1 一级位置

```
AppSidebar
├─ ◉ Today
├─ ◯ Library
├─ ◯ Insights
├─ ◯ Connect
├─ ◉ Help          ← 新增（侧栏次底部，与 Settings 同段）
└─ ◯ Settings
```

侧栏分组：`Today / Library / Insights / Connect` 是"用"，`Help / Settings` 是"配置 + 学"，视觉上用 Separator 分两段。

### 3.2 七章 Tabs（`/help?tab=xxx`）

| # | tab key | 标题 | 受众 | 一句话目标 |
|---|---|---|---|---|
| 1 | `quickstart` | 快速开始 | 完全新手 | "5 分钟内跑通：装好 → 首次扫描 → 第一次搜索成功" |
| 2 | `integrations` | IDE 接入 | 装完想接 IDE 的人 | "Cursor / Claude Code / Codex / OpenCode 各自怎么一键安装 MCP / SKILL / hook" |
| 3 | `mcp` | MCP 工具用法 | 在 AI 编辑器里调 memex 的人 | "6 个 MCP tool 各自适用什么自然语言场景，附原句示例" |
| 4 | `skills` | Skill 用法 | 想触发 Skill 的人 | "skill-cli / skill-quality / skill-market 等如何在 Cursor / Claude Code 里被触发" |
| 5 | `context` | 项目记忆 / Hook | 高级用户 | "sessionStart hook 把项目摘要注入新会话的原理 + 调试方法" |
| 6 | `privacy` | 隐私与数据 | 在意数据的人 | "数据存哪、谁能读、redactions.yaml 怎么写、如何导出 / 删除" |
| 7 | `troubleshooting` | 维护与排障 | 出问题的人 | "重建索引 / 彻底重置 / 看日志 / 报错对照表" |

进入 `/help` 默认落到 `quickstart`。Tab 切换走 `?tab=xxx` 持久化（与 Settings 一致），便于辅助入口直接深链。

### 3.3 每章内部结构（统一模板）

每个 Tab 组件按以下顺序组织：

```
┌─ 1. 章节简介（1-2 句话告诉读者「这章学完你能做什么」）
│
├─ 2. 步骤卡片 / 内容块（Card + 序号）
│     - 操作描述
│     - 截图占位（先 ASCII，后期补 PNG）
│     - 可执行的 deep link：<router-link to="/connect">去看 IDE 状态 →</router-link>
│
├─ 3. 常见问题（Accordion 展开）
│
└─ 4. 「下一步去看」交叉引导（链接到下一章 / 相关 Tab）
```

### 3.4 不做强制 Onboarding 弹窗的理由

- **侵入式弹窗** 在桌面 app 普遍降低首次体验：用户希望立刻看到"我的东西"，不是被 modal 拦截
- 现有 `OllamaSetupDialog` 已经覆盖 LLM 首次配置；再叠 onboarding 弹窗会形成"开两个 modal"的尴尬
- 我们的入口策略（侧栏永久按钮 + Connect 空状态 + Today 空状态）能在用户**真正卡住的时刻**精准触达，比一次性弹窗 dismiss 后再也找不到更友好
- 卫语句风格：把"哪些人需要看"放到一个清晰入口里，而不是反向假设"所有人都没看过文档"

---

## 四、ASCII 原型

### 4.1 容器（`/help` 入口）

```
┌──────────────────────────────────────────────────────────────────┐
│ 帮助 · 使用说明                                                   │
│ 5 分钟跑通 → 接入 IDE → 排障对照表，按需查阅。                     │
│                                                                  │
│ ┌─[ 快速开始 | IDE 接入 | MCP | Skill | Hook | 隐私 | 排障 ]──┐  │
│ │                                                              │  │
│ │  (当前 tab 内容)                                             │  │
│ │                                                              │  │
│ └──────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────┘
```

### 4.2 快速开始 Tab（最高优先级）

```
1️⃣  装好 Memex
    [  截图：DMG 拖入 Applications  ]
    brew tap shetengteng/memex && brew install --cask memex
    或从 Releases 下载 DMG。
    > 详情见 README → [GitHub ↗]

2️⃣  打开 Memex，看第一次扫描
    [  截图：Library 页 + sessions 数字从 0 跳到 N  ]
    Memex 启动后会自动扫描 Cursor / Claude Code / Codex / OpenCode
    的本地数据库，把过往会话索引进 ~/.memex/。
    🟢 你看到「Library 共 N 个会话」就说明扫描成功。

3️⃣  做第一次搜索
    [  截图：⌘K 命令面板 + 一行搜索结果  ]
    按 ⌘K 打开命令面板，输入任何你过去聊过的关键词，
    例如「retry 策略」「Tauri 升级 v2」。
    搜索结果会标注：来自哪个 IDE / 哪个项目 / 何时聊的。

下一步 → [接入到你的 AI 编辑器 →]   [看 MCP 工具用法 →]
```

### 4.3 IDE 接入 Tab（与 Connect 页强联动）

```
[ 当前接入状态：2 / 4 已接入 ]   <router-link>到 Connect 页查看 →</router-link>

Cursor
  [Card]  说明：Cursor 接入后，AI 提问时可调用 memex MCP 检索历史
          一键安装 MCP / SKILL：跳转 Connect → IDE Integrations
          自然语言示例：「我之前讨论过 X 吗？」
          排障：找不到？→ Cursor 设置 → MCP → 检查 ~/.cursor/mcp.json

Claude Code
  [Card]  ...

Codex
  [Card]  ...

OpenCode
  [Card]  ...
```

### 4.4 排障 Tab（常见问题 Accordion）

```
[Accordion]
  ▶ "读取失败：unable to open database file: Error code 14"
      原因：reset 后 daemon 重启竞态。v1.0.4+ 已修复。
      手动恢复：重启 Memex.app，或 ~/.memex/memex.db 健康时直接重开。

  ▶ "0 / 0 已接入" 但 IDE 明明在跑
      原因：IDE 的 sqlite db 路径变了 / 没有 Full Disk Access。
      手动检查：系统设置 → 隐私与安全性 → 完全磁盘访问 → 加 Memex。

  ▶ daemon 起不来
      看日志：[查看日志 →]（深链 /logs）
      端口冲突：默认 9999，被占自动 fallback 到 10000-10010。

  ▶ 重建索引 / 彻底重置在哪
      [查看 →]（深链 /settings?tab=system）

  ▶ ...
```

---

## 五、技术方案

### 5.1 文件拆分（遵守 ≤ 300 行 / 文件）

```
app/desktop/src/views/help/
├── index.vue                              # 容器：Tabs + ?tab 同步
├── components/
│   ├── QuickStartTab.vue                  # ~150 行
│   ├── IntegrationsTab.vue                # ~200 行
│   ├── McpUsageTab.vue                    # ~180 行
│   ├── SkillsTab.vue                      # ~150 行
│   ├── ContextHookTab.vue                 # ~150 行
│   ├── PrivacyTab.vue                     # ~120 行
│   ├── TroubleshootingTab.vue             # ~250 行（含 Accordion）
│   └── HelpStepCard.vue                   # 复用：编号 + 标题 + body + 截图 slot
└── help.test.ts                           # 路由 + 各 Tab 渲染 smoke test
```

每个 Tab 组件 lazy `import()`，避免一次性全部进 bundle。

### 5.2 路由

```typescript
{
  path: '/help',
  name: 'help',
  component: () => import('@/views/help/index.vue'),
  meta: { title: 'nav.help', breadcrumb: ['nav.help'] },
}
```

### 5.3 i18n key 设计（命名空间 `help.*`）

`zh.ts` / `en.ts` 同步加：

```ts
'nav.help': '帮助',
'sidebar.nav.help': '帮助',

'help.title': '使用说明',
'help.subtitle': '5 分钟跑通 → 接入 IDE → 排障对照表，按需查阅。',

'help.tabs.quickstart': '快速开始',
'help.tabs.integrations': 'IDE 接入',
'help.tabs.mcp': 'MCP 工具',
'help.tabs.skills': 'Skill',
'help.tabs.context': '项目记忆',
'help.tabs.privacy': '隐私与数据',
'help.tabs.troubleshooting': '维护与排障',

'help.quickstart.intro': '...',
'help.quickstart.step1.title': '装好 Memex',
'help.quickstart.step1.body': '...',
'help.quickstart.step2.title': '打开 Memex，看第一次扫描',
'help.quickstart.step2.body': '...',
// ... 余下章节同上
```

总计预估约 200 条 i18n key（每章 ~25-30 条）。

### 5.4 shadcn-vue 组件复用清单

| 组件 | 用途 |
|---|---|
| `Tabs` / `TabsList` / `TabsTrigger` / `TabsContent` | 章节切换 |
| `Card` | 步骤卡片 / IDE 模块 |
| `Accordion` / `AccordionItem` / `AccordionTrigger` / `AccordionContent` | FAQ 展开（**需 add**：`npx shadcn-vue@latest add accordion`） |
| `Badge` | 状态标记 |
| `Button` | 跳转链接 |
| `Separator` | 章节分隔 |
| `RouterLink` | 内部跳转 |

无三方依赖新增。

### 5.5 辅助入口埋点

| 位置 | 文件 | 触发条件 | 跳转 |
|---|---|---|---|
| 侧栏永久按钮 | `components/shell/AppSidebar.vue` | 永久显示 | `/help` |
| Connect 空状态 | `views/connect/components/IdeIntegrationsCard.vue` | `已接入数 == 0` 时 | `/help?tab=integrations` |
| Today 空状态 | `views/today/components/EmptyState.vue`（如已有） | 还没采集到任何会话 | `/help?tab=quickstart` |
| 设置页底部 | `views/settings/components/SystemTab.vue` | 永久显示 | `/help?tab=troubleshooting` |
| 用户菜单 | `components/shell/AppSidebar.vue`（底部弹出） | 永久显示一项「使用说明」 | `/help` |

### 5.6 文案策略

- **任务导向**：每章首句直接告诉读者"这章学完你能做什么"
- **不堆 feature**：不写"Memex 支持 N 种适配器、M 个 Skill"，写"在 Cursor 里这样问就能调出三个月前的对话"
- **代码块统一**：用 `<code>` + `Button copy` 复制按钮（沿用现有 `ToolCallBlock.vue` 风格）
- **截图统一规范**：先 ASCII 占位，后期 docs/screenshots 补 PNG，通过 `<img :src>` 引用
- **关键操作前必有截图**（截图清单单独维护，先列出 12 张优先级最高的）

### 5.7 截图清单（M2 阶段补图）

| # | 截图 | 用途 |
|---|---|---|
| 1 | DMG 拖入 Applications | quickstart step1 |
| 2 | Library 首次扫描后 sessions 数字 | quickstart step2 |
| 3 | ⌘K 命令面板 + 搜索结果 | quickstart step3 |
| 4 | Connect 页 IDE Integrations 4 个 IDE | integrations |
| 5 | Cursor MCP 配置成功界面 | integrations.cursor |
| 6 | Claude Code Skill 调用界面 | skills.claude_code |
| 7 | Settings → System → 重建索引按钮 | troubleshooting |
| 8 | Logs 页面 | troubleshooting |
| 9 | Privacy redactions.yaml 编辑 | privacy |
| 10 | sessionStart hook 注入 banner | context |
| 11 | OllamaSetupDialog | quickstart 进阶 |
| 12 | macOS 完全磁盘访问设置 | troubleshooting.fda |

---

## 六、测试策略（按 `project.mdc` 单元测试要求）

| 测试 | 文件 | 覆盖 |
|---|---|---|
| 路由解析 | `router/router.test.ts` | `/help` 能解析、`?tab=mcp` 能传给 view |
| 容器组件 | `views/help/help.test.ts` | Tabs 渲染 7 个 trigger / `?tab` 切换更新 activeTab / 默认 fallback `quickstart` |
| 各 Tab smoke | `QuickStartTab.test.ts` 等 7 个 | 渲染不爆 / i18n key 全部解析（不出现 `help.xxx` 字面量） |
| i18n 完整性 | `i18n/i18n.test.ts` | `zh.ts` / `en.ts` 中 `help.*` 全部 key 一一对应 |
| 辅助入口 | 现有 `IdeIntegrationsCard.test.ts` 增 case | `已接入数 == 0` 时显示 "查看接入说明" 链接 |

预计新增 ~12 个测试文件，~80 个 case。

---

## 七、落地节奏（5 个里程碑）

| 里程碑 | 范围 | 估时 |
|---|---|---|
| **M1 骨架** | 路由 + Tabs 容器 + 7 个空 Tab + i18n 占位 + 侧栏入口 | 0.5 天 |
| **M2 高频章节** | QuickStart + Integrations + Troubleshooting 三章完整内容 + 单测 | 1.5 天 |
| **M3 中频章节** | McpUsage + Skills 两章内容 + 单测 | 1 天 |
| **M4 进阶章节** | ContextHook + Privacy 两章内容 + 单测 | 1 天 |
| **M5 辅助入口** | Connect / Today 空状态链接 + 截图替换 ASCII | 0.5 天 |

总计 ~4.5 人日。M1 + M2 上线即可覆盖 80% 用户场景，可以单独发版。

---

## 八、验收清单

- [ ] 侧栏出现「帮助」按钮，点击进入 `/help` 默认 quickstart Tab
- [ ] 7 个 Tab 都能渲染，无 i18n 字面量泄漏
- [ ] `/help?tab=integrations` 等深链可直达指定 Tab
- [ ] Connect 页 "0/0 已接入" 时显示链接到 `/help?tab=integrations`
- [ ] Today 空状态显示链接到 `/help?tab=quickstart`
- [ ] zh / en 切换全章节文案跟随
- [ ] 单文件 ≤ 300 行
- [ ] `cargo test` / `npm test` 全绿
- [ ] 至少 12 张关键截图就位（M5 完成时）

---

## 九、待决策项（开工前敲定）

| # | 待决策 | 备选 |
|---|---|---|
| 1 | 截图风格 | A. 真实 macOS UI 截图（推荐）<br>B. 抽象插画 / 卡通风<br>C. ASCII 终止于 M5 |
| 2 | Help 在侧栏的位置 | A. Settings 上方（与 Connect 同段）<br>B. 侧栏底部独立段（与用户菜单同段，推荐）<br>C. 顶栏齿轮按钮旁 |
| 3 | 是否在第一次启动 toast 提示 | A. 不弹（推荐，§3.4 理由）<br>B. 一次性 banner，dismissable<br>C. 强制 modal |
| 4 | 是否做交互式 walkthrough | A. M5 之后再评估（推荐）<br>B. 现在就规划成 M6<br>C. 不做 |
| 5 | 是否同步更新 README | A. 等 M2 上线后再单独 PR 同步（推荐）<br>B. 同 PR 一起改<br>C. 不动 README |

---

## 十、与现有规则的对齐

- ✅ `project.mdc` "前端组件必须使用 shadcn vue" → 全部组件来自 shadcn-vue
- ✅ `project.mdc` "前后端代码超过 300 行需要按照模块进行拆分" → 7 章拆 7 个 .vue
- ✅ `project.mdc` "完成一个模块功能需要有 单元测试" → 见 §6
- ✅ `project.mdc` "多使用卫语句，提前 return" → §3.4 的设计决策本身就是卫语句思维
- ✅ `project.mdc` "开发完成后需要部署最新的版本" → 每个 M 完成后用 `scripts/upgrade-local.sh` 替换本地 Memex.app
