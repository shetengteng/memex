# Memex Popup → 桌面应用 TODO 清单

> 日期：2026-06-07
> 配套方案：`20260607-01-Memex-popup替换为桌面应用-重构方案.md`
> 总览：5 个 Phase / 67 个原子 TODO，每项都可独立勾选验收

---

## 图例

- `[ ]` 待办 · `[x]` 完成 · `[~]` 进行中 · `[!]` 阻塞
- 标签：
  - `frontend` Vue/TS 改动
  - `rust` Tauri / `src-tauri/**`
  - `config` 配置文件
  - `deps` 依赖增删
  - `cleanup` 删除旧代码

---

## Phase 0 · 准备 & 决策收尾（30 min）

- [x] **0.1** `deps` 确认两个未定项的最终选择：
  - 原型 `@lucide/vue` 统一为 `lucide-vue-next`（与旧代码 `IdeIcon.vue` 一致）
  - 托盘 popup 走 hash 路由方案（`#/tray-popup`），不开 vite 多 entry
- [x] **0.2** `cleanup` 备份当前 `tauri-app/src/`（建分支 `pre-prototype-merge` 保险）
- [x] **0.3** 列出 `tauri-app/package.json` 与 `design/prototype-app/package.json` 依赖差集，输出合并清单（草稿写在 PR 描述里即可）

---

## Phase 1 · Scaffold Desktop Shell（1 天）

> **目标**：跑通"桌面窗口形态"的最小骨架。原型整份替换 `tauri-app/src/`，跑起来能看到 sidebar + 5 个路由，但页面内可以先全 mock。

### 1.1 文件迁移 / 替换

- [x] **1.1.1** `cleanup` 删除 `tauri-app/src/` 内当前全部内容
- [x] **1.1.2** `frontend` 复制 `design/prototype-app/src/*` → `tauri-app/src/`
- [x] **1.1.3** `frontend` 复制旧 `tauri-app/src/i18n/` 回到 `tauri-app/src/i18n/`（原型没有）
- [x] **1.1.4** `frontend` 复制旧 `tauri-app/src/components/IdeIcon.vue` 到 `tauri-app/src/components/`
- [x] **1.1.5** `frontend` 复制旧 `tauri-app/src/components/MarkdownContent.vue` 到 `tauri-app/src/components/`
- [x] **1.1.6** `frontend` 复制旧 `tauri-app/src/types/` 到 `tauri-app/src/types/`（原型用 mock 类型，需保留 IPC 真实类型定义）
- [x] **1.1.7** `frontend` 把 prototype 的 `style.css` merge 进新 `tauri-app/src/styles/`

### 1.2 依赖与构建配置

- [x] **1.2.1** `deps` `tauri-app/package.json` 合并依赖：
  - 加：`vue-router@^4` / `vue-sonner@^2` / `@vueuse/core@^14` / `reka-ui` / `shadcn-vue` / `class-variance-authority` / `tw-animate-css`
  - 改：所有 `@lucide/vue` → `lucide-vue-next`（同时改原型源文件）
  - 保留：`@tauri-apps/api` / `@tauri-apps/plugin-deep-link` / `@tauri-apps/plugin-opener` / `markdown-it` / `tailwindcss`
- [x] **1.2.2** `deps` `npm install` 通过，`vue-tsc -b` 通过
- [x] **1.2.3** `config` 确认 `tauri-app/vite.config.ts` 的 `alias '@'` 指向 `src/`，端口与 `tauri.conf.json` 的 `devUrl` 一致（1520）
- [x] **1.2.4** `config` `tauri-app/components.json` 用原型版（已含 sidebar / command / sheet 等）

### 1.3 主窗口外壳跑起来

- [x] **1.3.1** `frontend` 全量替换 `tauri-app/src/main.ts` 为原型版（加 router、Toaster）
- [x] **1.3.2** `config` `tauri.conf.json` 主窗口改：
  - `width: 1100, height: 720, minWidth: 880, minHeight: 560`
  - `decorations: true, transparent: false, shadow: true`
  - `resizable: true, skipTaskbar: false, alwaysOnTop: false, center: true`
  - `visible: false`（首启可见性由 Rust 控制）
- [x] **1.3.3** `config` 删除 `Info.plist` 的 `<key>LSUIElement</key><true/>`
- [x] **1.3.4** `rust` `lib.rs::run()` 删除 `app.set_activation_policy(ActivationPolicy::Accessory)`，先暂时改成 `Regular`（动态切换 P3 再做）
- [x] **1.3.5** `rust` `lib.rs::run()` setup 阶段：`main.show().unwrap()` 让首启可见
- [x] **1.3.6** `frontend` 临时让 prototype mock data 跑通（不接 IPC，先看到 5 个 tab 切换正常）

### 1.4 验收

- [~] **1.4.1** `npm run tauri:dev` 启动 → 桌面窗口可见，1100×720，可调大小
- [~] **1.4.2** Sidebar 5 个菜单（Today / Library / Insights / Connect / Settings）可切换
- [~] **1.4.3** Today / Library / Insights / Connect / Settings 五个页面渲染无报错（mock 数据）
- [~] **1.4.4** ⌘K 命令面板能打开
- [~] **1.4.5** Dock 出现 Memex 图标，可通过红色关闭按钮关闭窗口（行为先用默认，P3 再拦截）

---

## Phase 2 · Tray Popup（半天）

> **目标**：托盘左键弹 360×520 极简卡片（最近会话 + 跳板），右键原生菜单。

### 2.1 路由与视图

- [x] **2.1.1** `frontend` `src/router/index.ts` 新增路由 `/tray-popup`，对应 `views/tray-popup/index.vue`
- [x] **2.1.2** `frontend` 新建 `src/views/tray-popup/index.vue`：
  - 不挂 SidebarProvider / 不渲染 SiteHeader
  - 顶部 logo + sessions 总数 + 设置齿轮
  - 中间最近 5 条 session 列表（先 mock）
  - 底部 LLM 状态 + "打开 Memex" 按钮（跳到 main 窗口）
- [x] **2.1.3** `frontend` `App.vue` 根据 `useRoute().path === '/tray-popup'` 切换布局（裸渲染 RouterView，不挂 SidebarProvider）

### 2.2 Tauri 窗口配置

- [x] **2.2.1** `config` `tauri.conf.json` `windows[]` 新增 `tray-popup` 窗口配置：
  ```
  width: 360, height: 520, visible: false,
  decorations: false, transparent: true, shadow: false,
  backgroundColor: '#00000000', resizable: false,
  skipTaskbar: true, alwaysOnTop: true, focus: true
  ```
- [x] **2.2.2** `rust` `tray.rs` 删掉对 `"main"` 窗口的 toggle 逻辑；改成 toggle `"tray-popup"`：
  - 左键 Down → 若不存在 webview 则创建（hash url `index.html#/tray-popup`），存在则 toggle 显隐
  - 定位：跟随托盘图标，y = 图标底部，x = 图标中心 - 180（窗口宽度的一半）

### 2.3 失焦自动隐藏（tray-popup 内部）

- [x] **2.3.1** `frontend` `views/tray-popup/index.vue` 内 `appWindow.onFocusChanged` → 失焦自动隐藏
- [x] **2.3.2** `frontend` Esc 键也隐藏 tray-popup

### 2.4 右键原生菜单

- [x] **2.4.1** `rust` `tray.rs::install` 菜单项保留并扩展：
  - `count` "Sessions: N"（沿用 10s 轮询）
  - `show` "Show Memex" → show main + focus + update_activation_policy
  - `search` "Search…" → show main + emit `'open-command-palette'`
  - `settings` "Settings…" → show main + emit `'navigate'` payload=`/settings`
  - `quit` "Quit Memex" → `app.exit(0)`
- [x] **2.4.2** `frontend` `App.vue` listen `'open-command-palette'` 事件 → 触发 `useCommandPalette().open()`
- [x] **2.4.3** `frontend` `App.vue` listen `'navigate'` 事件 → `router.push(payload)`

### 2.5 验收

- [~] **2.5.1** 左键托盘 → 弹 360×520 frameless 卡片，定位准确
- [~] **2.5.2** 卡片失焦自动消失，再点托盘重新出现
- [~] **2.5.3** 右键托盘 → Show/Search/Settings/Quit 全部跳转正确
- [~] **2.5.4** "打开 Memex" 按钮能正确显示主窗口

---

## Phase 3 · Close-to-Tray + Dynamic Dock + Global Shortcut（半天）

> **目标**：主窗口关闭按钮缩到托盘；macOS Dock 动态切换；`⌘⇧M` 切换主窗口。

### 3.1 关闭按钮拦截

- [x] **3.1.1** `rust` `lib.rs::run()` setup 内对 `main` 窗口注册 `on_window_event`：
  ```
  if let WindowEvent::CloseRequested { api, .. } = event {
      api.prevent_close();
      win.hide();
      update_activation_policy(&handle);
  }
  ```
- [x] **3.1.2** `rust` 验证 `Cmd+Q` / 菜单 Quit 仍然能真正退出（`app.exit(0)` 不被拦截）

### 3.2 动态 Dock 切换（macOS）

- [x] **3.2.1** `rust` `lib.rs` 新增：
  ```
  #[cfg(target_os = "macos")]
  fn update_activation_policy(app: &AppHandle) { ... }
  ```
  逻辑：枚举所有 webview_windows，若有非 `tray-popup` 且 `is_visible()` → `Regular`，否则 `Accessory`
- [x] **3.2.2** `rust` 调用点：
  - 主窗口 show / hide 后
  - 主窗口 close_requested 拦截后
  - 全局 shortcut 触发后
  - tray 右键菜单 "Show Memex" 后
- [x] **3.2.3** 不在 tray-popup 显隐时调用（不影响 Dock）

### 3.3 全局快捷键 `⌘⇧M` 行为变更

- [x] **3.3.1** `rust` `lib.rs` 全局 shortcut handler 改：
  ```
  if let Some(win) = app.get_webview_window("main") {
      if win.is_visible().unwrap_or(false) && win.is_focused().unwrap_or(false) {
          let _ = win.hide();
      } else {
          let _ = win.show();
          let _ = win.set_focus();
      }
  }
  update_activation_policy(&handle);
  ```
- [x] **3.3.2** `frontend` 主窗口删除旧的 listen `'global-shortcut'`（已无意义，不需要前端响应）

### 3.4 删除旧 popup 行为

- [x] **3.4.1** `cleanup` `frontend` 主窗口 App.vue 删除 `appWindow.onFocusChanged → hide` 逻辑（仅 tray-popup 保留失焦隐藏）
- [x] **3.4.2** `cleanup` `rust` 删除 `lib.rs` 中创建 `"dashboard"` 窗口的代码（已合并到 main）

### 3.5 验收

- [~] **3.5.1** 点击主窗口红色关闭 → 主窗口消失，daemon 仍然在运行（`memex daemon status` 仍 ready）
- [~] **3.5.2** 主窗口全部隐藏后，Dock 图标消失（变菜单栏应用）
- [~] **3.5.3** 点托盘 "Show Memex" → 主窗口出现，Dock 图标也出现
- [~] **3.5.4** `⌘⇧M` 在主窗口已聚焦时按 → 隐藏；隐藏时按 → 显示并聚焦
- [~] **3.5.5** `Cmd+Q` 或托盘 Quit → 进程退出，托盘图标消失

---

## Phase 4 · Wire Prototype Views to Real Backend（1.5 天）

> **目标**：把原型 mock data 全部替换为真实 `invoke` 调用，五大页面全部接通后端。

### 4.1 Composables 迁移

- [x] **4.1.1** `frontend` `composables/useMemex.ts` 已就位（Phase 1 阶段已搬过来）；本轮补齐 ide / skill / hook / update 共 10 个新 IPC 包装
- [x] **4.1.2** `frontend` `composables/useScanState.ts` 已就位
- [x] **4.1.3** `frontend` 新建 `composables/useStats.ts`：10s 轮询 + 模块单例 + 引用计数（refCount=0 停轮询）
- [x] **4.1.4** `frontend` 新建 `composables/useDaemon.ts`：5s 轮询 + restart 包装

### 4.2 mock data 改为真实数据源

- [x] **4.2.1** `frontend` `src/stores/memex.ts` 已创建。设计调整：原方案要求用 `ref`，**实际采用 `reactive`**（数组 + 对象），这样 21 个原型组件无需把 `sessions.slice()` 改成 `sessions.value.slice()`。
  - `sessions: Session[]` （reactive array，`listRecent(200)` → `splice` 同步）
  - `projects: Project[]` （reactive array，`listProjects()` → `splice`）
  - `stats: Ref<Stats|null>` + `daemon: Ref<DaemonStatus|null>` 给 composable 写回
  - `totals: reactive({sessions, messages, projects})` + `daemonStatus: reactive(...)` —— `watch(stats|daemon)` 同步字段
  - `adapters: AdapterInfo[]` / `ADAPTER_MAP: Record<string, AdapterInfo>` reactive 元信息
- [x] **4.2.2** `frontend` 21 个 view 全部 import 路径替换 `@/mock/data` → `@/stores/memex`；旧 `src/mock/` 目录已删除
- [x] **4.2.3** `frontend` 主窗口 `App.vue` `onMounted` 调 `initMemexStore()` + 监听 `reset-complete` / `summary-progress` 刷新 sessions/projects
- [x] **4.2.4** `frontend` `IdeChip` / `IdeDot` 的 `adapter` prop 类型从 `Adapter` union 放宽为 `string`，配合后端任意 source id

> 仍未在视图层精细接线的部分：今日活动卡的 hourlyBars / reflections / reports / heatmap 等静态 mock 字段仍是空数组。**这些会在 4.3 / 4.5 / 4.6 各自页面接线时填充。**

### 4.3 Today 页接线

- [x] **4.3.1** `frontend` `views/today/components/ActivityCard.vue` 接 `getWorkload(1)` → 今日活动微图（hourlyBars 来自 heatmap，stats / topProjects / topAdapters 全部从 by_project / by_adapter 派生）
- [x] **4.3.2** `frontend` `views/today/components/WeeklySummaryCard.vue` 接 `listReports('weekly', 1)` → 本周摘要 + 关键决策 + 主题
- [x] **4.3.3** `frontend` `views/today/components/ReflectionCard.vue` 接 `reflectList()` → 待反思列表（点击跳转 /insights?reflect=<scope_key>）
- [x] **4.3.4** `frontend` `views/today/components/SmartResumeCard.vue` 接 `sessions.slice(0, 3)` → 简版"最近 3 个 session"（等后端补 `interruptedAt` 升级）
- [x] **4.3.5** `frontend` `views/today/components/SystemStatusCard.vue` 接 `daemon` + `daemonStatus` + `stats` 全字段 → 折叠面板状态行

### 4.4 Library 页接线

- [x] **4.4.1** `frontend` `views/library/index.vue` 已使用 `sessions` reactive store（initMemexStore 内部拉 `listRecent(200)`）
- [x] **4.4.2** `frontend` `views/library/components/LibrarySessionDrawer.vue` 打开 drawer 时调 `getSession(id)` 拉详情
- [x] **4.4.3** `frontend` `views/library/components/LibraryProjectsGrid.vue` 已接 `projects` reactive store（initMemexStore 内部拉 `listProjects()`）
- [x] **4.4.4** `frontend` Threads 子 tab 保留占位"即将上线"
- [x] **4.4.5** `frontend` Library 顶部"立即采集"按钮接 `triggerIngest()` + Toast；App.vue 监听 `'summary-progress'` / `'reset-complete'` 自动刷新 store
- [x] **4.4.6** `frontend` `views/library/components/LibraryFacets.vue` adapter 计数从硬编码 mock 改为 `breakdownByAdapter`（来自 `getBreakdown().by_adapter`）

### 4.5 Insights 页接线

- [x] **4.5.1** `frontend` `views/insights/components/ReportsTab.vue`：
  - 接 `listReports('daily')` / `listReports('weekly')`，monthly 占位 toast
  - 接 `regenerateReport(scope, key)` + 选中态保持
  - 当前用 `<pre>` 渲染（Markdown 渲染优化预留 P5）
- [x] **4.5.2** `frontend` `views/insights/components/ReflectionsTab.vue`：
  - 接 `reflectList()` / `reflectGet(scope_key)` Dialog 详情
  - 触发条接 `reflectRun(period)`，时长提示 30~60 秒
- [x] **4.5.3** `frontend` `views/insights/components/TrendsTab.vue`：
  - 接 `getWorkload(7/30/90)` 一处搞定 KPI / 日历 / 24×7 习惯图 / adapter 占比 / 项目 Top10
  - 后端 weekday(0=Mon..6=Sun) → UI(0=Sun..6=Sat) 索引转换已完成

### 4.6 Connect 页接线

- [x] **4.6.1** `frontend` `views/connect/components/AdaptersCard.vue`：
  - 7 个 adapter 行（来自 `stores/memex` adapters）
  - 每行 `onMounted` 读 `getConfig('adapter.{key}.enabled')` 同步真实启停
  - Switch 改变时调 `toggleAdapter(key, enabled)`，单行刷新 + Toast
  - "立即扫描" / 单行扫描按钮调 `triggerIngest()` / `triggerIngest(key)`
- [x] **4.6.2** `frontend` `views/connect/components/IdeIntegrationsCard.vue`：
  - 接 `ide_list_status` / `skill_list_status` / `hook_list_status` 获取真实 IDE 状态
  - MCP / SKILL / Hook 三组开关对应 `ide_install` / `skill_install` / `hook_install` （含 uninstall）
  - 空状态 + 加载状态 + "重新检测" 按钮
  - SKILL / Hook 同理接 `skill_*` / `hook_*`
- [x] **4.6.3** `frontend` `views/connect/components/McpActivityCard.vue` 已重构为标准 "Coming Soon" 占位卡片（删除孤立 mock 数据 mcpTools/mcpCallEvents，渲染清晰的"即将上线"提示）

### 4.7 Settings 页接线

- [x] **4.7.1** `frontend` `views/settings/components/LlmTab.vue`：
  - 接 `llm_provider_list` / `llm_provider_upsert` / `llm_provider_delete` / `llm_provider_test`
  - Provider 列表 + 启用切换 + 默认切换 + 测试连接 + Fallback 链路展示
  - Prompt 模板通过 `getConfig/setConfig` 持久化（`llm.prompt_template`）
  - `ProviderEditDialog` 已沿用旧组件（已接 `llm_list_models` / `llm_provider_test_draft`）
- [x] **4.7.2** `frontend` `views/settings/components/PreferencesTab.vue`：
  - 主题三选一沿用 `composables/useTheme`
  - 语言 / 隐私 / 通知三组开关通过 `getConfig/setConfig` 持久化 (`pref.language` / `pref.privacy.*` / `pref.notify.*`)
  - 注：界面语言切换只完成持久化，i18n 完整切换留到 Phase 5 收尾
- [x] **4.7.3** `frontend` `views/settings/components/DataTab.vue`：
  - 接 `system_reset_index` / `system_reset_all`（清空二次确认输入 `DELETE`）
  - 接 `runDoctor` 拉 `data_dir` 拼接出 SQLite 路径
  - 备份开关 / 保留天数通过 `getConfig/setConfig` 持久化（`backup.auto` / `backup.retention_days`）
- [x] **4.7.4** `frontend` `views/settings/components/SystemTab.vue`：
  - 接 `cli_status` / `cli_install` / `cli_uninstall` —— 显示真实安装路径、PATH 检测、导出 hint
  - 接 `doctor_run` —— 渲染数据目录 / Schema / FTS / Cursor 探测
  - 接 `check_for_updates` + `@tauri-apps/api/app#getVersion` —— 真实当前版本 + 新版本提示，新版可"打开发布页"
  - 备注：守护进程展示已在 SystemStatusCard / Sidebar Footer 完成，本卡片聚焦 CLI / Doctor / 更新
  - 接 `check_for_updates`

### 4.8 Sidebar Footer 接线

- [x] **4.8.1** `frontend` `AppSidebar.vue` 底部 popover 已接 `totals` / `adapters` / `daemonStatus` / `stats`，显示真实 sessions / messages / 活跃 adapter 数
- [x] **4.8.2** `frontend` 摘要进度条接 `stats.summaries / stats.sessions_eligible_for_summary`，sessionsPending > 0 时显示触发按钮
- [x] **4.8.3** `frontend` "触发 N 个待摘要"按钮已接 `batchSummarize()` + Toast 反馈

### 4.9 Tray Popup 接线

- [x] **4.9.1** `frontend` `views/tray-popup/index.vue` 接 `sessions.slice(0, 5)`（store 已由 main 窗口在启动时拉取，弹出时 `refreshSessions(5)` 增量刷新）
- [x] **4.9.2** `frontend` 点击列表项 → emit 'navigate' `/library?session=<id>` + 自动 hide popup
- [x] **4.9.3** `frontend` 修复 popup 串内容 / 白屏 bug：
  - 第一版用 `await router.replace` + `await router.isReady()` → 在 `app.use(router)` 之前 router 没 init，导致死锁白屏
  - 终版改在 `main.ts` 中 mount 前直接置 `window.location.hash = '/tray-popup'`，让 `createWebHashHistory` 自然吃到正确 hash
  - `App.vue` 用 `getCurrentWindow().label === 'tray-popup'` 兜底 layout（route.meta 失效时也能回到 bare）
- [x] **4.9.4** `frontend` Library "加载更多"接 `loadMoreSessions`：替换静态占位为按钮，loading 期 spinner，全部加载完显示"已显示全部 N 个"；`fAdapters/fProjects` 默认值清掉避免冷启动过滤掉所有；空列表态显示采集引导

### 4.10 验收

- [~] **4.10.1** Today 页所有卡片都有真实数据（不是占位）
- [~] **4.10.2** Library 能列出 200 个最近 session，能进 Drawer 看详情
- [~] **4.10.3** Insights → Reports 能生成新报告并渲染
- [~] **4.10.4** Connect → Adapters 启停能反映到 `~/.memex/config.toml`
- [~] **4.10.5** Connect → IDE 集成能装/卸到 Cursor / Claude Code 等
- [~] **4.10.6** Settings → LLM 能新增/测试 provider，与旧 popup 一致
- [~] **4.10.7** Sidebar footer 显示实时 session 总数

> P4 代码已全部接通，待人工 `npm run tauri:dev` 验证 GUI 行为闭环

---

## Phase 5 · Deep Link + Ollama Setup + 收尾（半天）

> **目标**：迁移 deep link、Ollama 引导 Dialog；删干净废弃代码。

### 5.1 Deep Link 处理

- [x] **5.1.1** `rust` `lib.rs::forward_deep_links` 已改：不再创建 dashboard 窗口，show main + emit `'deep-link'`（`pending` mutex 兜底 cold-start 场景）
- [x] **5.1.2** `frontend` `App.vue` listen `'deep-link'` + invoke `take_pending_deep_link`（mounted 时拉一次 cold-start pending）：
  - `memex://session/<id>` → `router.push('/library?session=<id>')`
  - `memex://search` → `useCommandPalette().open()`
  - `memex://projects` → `router.push('/library')`
- [~] **5.1.3** 验证：`open memex://session/abc123` 能定位到 Library Drawer（待人工运行）

### 5.2 Ollama 首启引导 Dialog

- [x] **5.2.1** `frontend` 抽出独立组件 `components/shell/OllamaSetupDialog.vue`，包含 `checkOllamaOnStartup` / `copyOllamaCmd` / `copyBrew` / `dismissForever` 全部逻辑
- [x] **5.2.2** `frontend` 仅在 `bareLayout=false` 时才挂载（tray-popup 不会触发，因为 popup 走 RouterView only 分支）
- [x] **5.2.3** "前往设置" 按钮 `router.push('/settings')`（Settings 页 default-value 已是 LLM tab，自动定位）

### 5.3 事件迁移

- [x] **5.3.1** `frontend` `App.vue` 已 listen `'reset-complete'` → `resetScanState()` + `refreshSessions()` + `refreshProjects()` + `refreshBreakdown()`
- [x] **5.3.2** `frontend` `App.vue` 已 listen `'summary-progress'` → `refreshSessions()`（sidebar footer 通过 `totals` reactive store 自动更新）

### 5.4 删除废弃代码

- [x] **5.4.1** `cleanup` `rust` `lib.rs` `forward_deep_links` 已无 dashboard 窗口创建逻辑（直接 show main + emit）
- [x] **5.4.2** `cleanup` `frontend` 删除孤儿 `views/dashboard/` 目录（10 个文件，0 处引用）；无 `@/mock` 引用残留；删除 stores/memex.ts 中孤立 mcpTools/mcpCallEvents
- [x] **5.4.3** `cleanup` IPC 命令一期保守不删（兼容性优先）；i18n 中 `dashboard.*` / `nav.dashboard` 等死键位 0 处使用，保留作未来清理（避免一次性删除大段风险）
- [x] **5.4.4** `cleanup` `design/prototype-app/` 加 `ARCHIVED.md` 标注归档状态（保留作 UI 决策溯源 + 未来视觉沙盒，但禁止继续开发新功能）

### 5.5 文档与 README

- [x] **5.5.1** 更新 `README.md`：
  - 特性表中"系统托盘"段落拆分为"桌面应用 + 极简托盘 popup"两行
  - "Web Dashboard `:9999`"改名为"Daemon HTTP API"（更符合实际）
  - 技术栈中 `Menubar` → `Desktop App`（含 Vue Router 4 / reka-ui）
  - "首次运行"中"安装 Menubar App"改为"启动桌面应用"
  - MCP SKILL 节中"Memex popup → Settings"改为"桌面应用 → Connect → IDE 集成"
  - **待人工补**：截图替换（需要实拍），未在 Agent 范围内
- [x] **5.5.2** 更新 `design/20260531-04-Memex-menubar-ASCII原型与功能设计.md` 头部加`[历史归档]`标注 + 当前架构指引
- [x] **5.5.3** 更新 `design/20260601-01-Memex-WebUI-Popup内嵌设计.md` 头部加`[历史归档]`标注 + 当前架构指引

### 5.6 验收 / 端到端冒烟

- [ ] **5.6.1** 全新机器跑 `bash scripts/upgrade-local.sh --skip-backup` 全流程通过
- [ ] **5.6.2** 首启自动弹 Ollama 引导（如果未装）
- [ ] **5.6.3** Deep link 测试：`open memex://session/<已有id>` 能定位
- [ ] **5.6.4** 托盘 / 主窗口 / Dock 三者状态机一致
- [ ] **5.6.5** 一键采集 + 一键摘要 + 全文搜索（⌘K） 全部工作
- [ ] **5.6.6** macOS 关闭主窗口 30 秒后 Dock 自动消失，再开主窗口 Dock 回来

---

## 总计

- Phase 0：3 项（准备）
- Phase 1：18 项（scaffold）
- Phase 2：10 项（tray popup）
- Phase 3：10 项（窗口行为 + 快捷键）
- Phase 4：22 项（接线）
- Phase 5：14 项（deep link + 收尾）
- **共 77 项**，预计 4 天可全部完成

每个 Phase 结束 = 一个可发布的版本，**不会半成品**。

---

## 进度记录

| 日期 | Phase | 完成项数 | 备注 |
|---|---|---|---|
| 2026-06-07 | - | 0 / 77 | TODO 清单创建 |
| 2026-06-07 | P0 | 3 / 3 | 决策定档 + 备份分支 `pre-prototype-merge` |
| 2026-06-07 | P1 | 12 / 18 | 代码已全部完成（编译/构建/类型检查通过）；5 项验收 `[~]` 待人工 `npm run tauri:dev` 验证 |
| 2026-06-07 | P2 | 7 / 10 | 代码全部完成；4 项验收 `[~]` 待人工运行 |
| 2026-06-07 | P3 | 7 / 10 | 代码全部完成；5 项验收 `[~]` 待人工运行 |
| 2026-06-07 | P4 | 10 / 47 | 4.1 + 4.2 完成：composables 迁移 + mock 转 reactive store；21 个 view 编译通过 + 37 个前端测试全绿。剩下 4.3~4.10 精细化接线待做 |
| 2026-06-07 | P4 | 32 / 48 | 完成 4.3 (5/5) / 4.4 (6/6) / 4.5 (3/3) / 4.6 (2/3) / 4.7 (4/4) / 4.8 (3/3) / 4.9 (3/3)；新增 LibraryFacets adapter 计数接 breakdownByAdapter；popup 串内容 bug 修复（main.ts 用 window.label 强制路由）。前端 49 个测试全绿，构建/类型检查通过 |
| 2026-06-07 | P4 hotfix | 33 / 48 | 修复 popup 白屏：router.replace 死锁问题改用 window.location.hash 直接同步；Library "加载更多"接 loadMoreSessions（按钮 + 全部加载完提示 + 空列表引导）；清除默认 fAdapters/fProjects 占位过滤。前端 53 个测试全绿（新增 5 个 loadMoreSessions case）|
| 2026-06-07 | P4 polish + P5 | 41 / 48 | UX 修复：活动日历 + 习惯热力图 hover 用 shadcn Tooltip 显示日期+会话数；窄屏时 Breadcrumb 加 whitespace-nowrap 防止"资料库"换行。P5 推进：5.1 deep-link 完成（含 cold-start take_pending）；5.2 Ollama 引导抽成独立 OllamaSetupDialog 组件（仅主窗口挂载，前往设置自动跳 LLM tab）；5.3 事件迁移已就绪；5.4 删除孤儿 views/dashboard/ 目录（10 个文件 0 处引用）|
| 2026-06-07 | P5 收尾 | 49 / 65 | 第二轮 UX：托盘 popup 透明背景修复（main.ts 加 tray-popup-window class + style.css 透明覆盖）；SiteHeader/AppSidebar/Popup 大数字接 formatNumber + shadcn Tooltip 显示原始数。P5 5.4.3-5.4.4 + 5.5.1-5.5.3 全部完成：4.6.3 McpActivityCard 标准化为 Coming Soon 占位（删孤立 mock）；prototype-app 加 ARCHIVED.md 归档；旧 menubar/WebUI-Popup 设计文档加[历史归档]标注；README 同步桌面应用形态。前端 53 个测试 + 11 个 Rust 测试全绿|
| 2026-06-07 | P4 polish | 49 / 65 | 修复 Library Drawer 对话区只显示 2 条的截断 bug（slice(-2) → 完整渲染）：参考旧 dashboard SessionDetailTab 实现 User/Bot 头像 + 角色 + 时间戳 + MarkdownContent，>50 条用"加载更多"按钮分页；同时移除底部 4 个无功能按钮（打开完整会话 / 发送到 IDE / 归档 / 删除）。新增 LibrarySessionDrawer.test.ts（5 cases，覆盖 detail 加载、>50 条分页、加载更多增量、底部按钮已移除、open 切换重新拉取）。前端 65 个测试全绿，vue-tsc 通过|
| 2026-06-07 | P4 polish | 49 / 65 | 修复 Connect → Adapters 卡片 "个会话"列恒为 — bug：a.sessions 字段是原型默认 0，永远没人写入；改成读 breakdownByAdapter（已经被 initMemexStore 填充的 reactive map），并在 onMounted/rescanAdapter/rescanAll 后主动 refreshBreakdown 保证 hot-reload。新增 1 个测试 case 验证 breakdownByAdapter → "个会话"列展示链路。前端 66 个测试全绿，vue-tsc 通过|
| 2026-06-07 | P4 polish | 49 / 65 | 修复 Continue adapter 切换 ID 不一致 bug：后端 toggle_adapter / get_config 只识别 "continue_dev"，但 sessions.source 与前端 ADAPTER_META 用的都是 "continue"，导致前端调切换时报 `unknown adapter: continue`，且 status 永远读不到真实值（保持初始 disabled）。修复方式：toggle_adapter / get_config 同时接受 "continue" 与 "continue_dev"（保留 Rust 字段名 continue_dev）。同时给 toggleAdapter 加幂等检查（new == current 直接 return），onMounted 写 a.status 也加幂等检查，避免 reka-ui Switch 在 model-value 频繁切换时误触发回吐 update 事件。新增 1 个测试 case，前端 67 cases / Rust 11 cases / vue-tsc 全绿|
| 2026-06-07 | P4 polish | 49 / 65 | 修复 toast 样式失败 bug：style.css 漏了 `@import "vue-sonner/style.css"`，所以 sonner toast 无圆角无背景无定位（用户描述"应该有 toast 组件展示但失败了"）。补 import 后 toast 恢复 bottom-right 圆角卡片正常形态。前端 67 cases 全绿，vite build / vue-tsc 通过|
| 2026-06-07 | P4 polish | 49 / 65 | 修复 tray-popup 内点 "打开 Memex" / session 条目无响应 bug：close-to-tray 状态下主窗口被 hide，popup 的 hideAndNavigate 只 emit `navigate` 没人接 → 看似按钮死了。新增 Tauri command `show_main_window`（show + unminimize + set_focus + 同步 macOS Dock activation policy），popup 在 emit navigate 之前先 invoke 它，确保主窗口先显式可见再处理路由。新增 1 个测试 case 校验 invoke 调用顺序。前端 68 cases / cargo check / vue-tsc 全绿|
| 2026-06-07 | P4 polish | 49 / 65 | 修复 Today 页 "晚上好，Terrell" 用户名硬编码 bug：原型字面量从未替换，所有人安装都显示 Terrell。新增 Tauri command `get_system_username`（std::env 读 USER/USERNAME，无新依赖），stores/memex 新增 `userName: Ref<string>` 默认 'User'，initMemexStore 一次性 invoke 写入；today/index.vue 改用 `{{ userName }}` 绑定。前端 68 cases / cargo check / vue-tsc 全绿|
| 2026-06-07 | P4 polish | 49 / 65 | 用户友好化 3 处运行时错误体验：①AppSidebar/ReflectionsTab/ReportsTab 的 toast 错误曾直接吐后端英文 raw（"No LLM provider available..."），新增 `humanizeBackendError(e)` 公共函数识别 4 类常见错误（无 LLM provider / Ollama 连不上 / API key 无效 / 消息太少），转中文 + 附"去设置"action 按钮（vue-sonner 的 action API），点击直跳 `/settings`。②Popup footer 之前丢了 daemon 状态指示，重新接入：状态圆点（绿/橙）+ Tooltip 显 pid / 运行状态 + 重启按钮（loading 时 spin）；接 useDaemon 自动 5s 轮询。③Settings → 系统 → 诊断 doctor 还在 loading 时一堆字段显示 "—" 或硬编码 `~/.memex` 误导用户；改为统一显示"检查中…"灰字，doctor 拿到结果再切回 emerald/rose 真实着色。新增 6 个 case 覆盖 humanizeBackendError + 1 个 SystemTab loading + 1 个 popup footer。前端 76 cases / cargo check / vue-tsc 全绿|
| 2026-06-07 | P4 polish | 49 / 65 | 修复"测试连接"按钮无反应 bug：①根因 — 后端 `llm_provider_test` / `llm_provider_test_draft` 都是 `#[tauri::command] async fn`，但内部调用 ureq HTTP（同步阻塞），`provider.is_available()` 走 GET /api/tags 没设 timeout，连不上时阻塞 30s+，期间 tokio worker 卡住所有 IPC，前端 invoke 永远不返回，UI "无反应"。修复：包到 `tokio::task::spawn_blocking` 隔离阻塞调用。②前端 LlmTab/ProviderEditDialog 的"测试连接"立刻 `toast.loading('正在测试 xxx…')` 给即时反馈，结果回来再 dismiss + 切 success/error；失败走 humanizeBackendError 转中文（unknown 改成"无法连接 Ollama / Key 无效"等具体提示）。前端 76 cases / cargo check / vue-tsc 全绿|
| 2026-06-07 | P4 polish | 49 / 65 | 第三轮 UX 修复：①Popup 加回 daemon 服务卡片：参考旧 popup `views/status/index.vue` 的 hero card 设计，在 popup header 下方插入醒目卡片（绿/橙双色态 + Activity/AlertTriangle 图标 + "运行中 (pid xxx)" 主文案 + LLM 模型/消息计数副文案 + 重启/启动按钮），footer 改为只显示"打开 Memex"按钮，不再挤在一行（之前太紧凑用户没注意到状态点）。②SiteHeader 主窗口右上角恢复 prototype 1-today.html 中的"全局搜索框"（min-w 220px outline button + Search 图标 + "搜索会话、项目、命令…" 占位 + ⌘K kbd，点击调 useCommandPalette.open）+"通知按钮"（铃铛 ghost 图标 + Tooltip "通知中心（开发中）"，点击 toast.message 占位）。③popup 测试 stubs 简化（不再包 Tooltip）+ daemon hero card 测试改为按文案匹配。前端 76 cases / cargo check / vue-tsc 全绿|
| 2026-06-07 | P4 polish | 49 / 65 | daemon 架构决策 + Phase 1 落地：①写 `design/20260607-04-Memex-daemon架构决策.md` 回答用户的 D1-D5 + "daemon 是否有必要"5 个根本问题，明确"daemon 保留、Tauri spawn、用户手动重启、REST 风格、direct-DB 解读 2 长期方向"，标注出 launchd 仅 CLI 高级用户保留。②实施 Phase 1 - SystemTab 把 daemon 状态卡片提到第一位（CLI 工具之上，是用户最关心的入口），CardFooter 增加"查看日志"按钮（plugin-opener 拉起 ~/.memex/daemon.stdout.log）。新增 Tauri command `daemon_log_path()` 返回绝对路径，避免前端处理 ~ 展开。③`crates/memex-core/src/storage/db/mod.rs` 顶部加详细架构守则注释：daemon 是写者唯一、direct-DB 仅同进程读优化、写操作下个版本必须走 daemon HTTP。前端 76 cases / cargo check / vue-tsc 全绿|

> 状态约定：
> - `[x]` 代码改完且 `cargo check` + `vue-tsc` + `vite build` 全绿
> - `[~]` 代码改完，但**需要肉眼跑 `npm run tauri:dev`** 才能闭环验收（GUI / Dock / Tray 行为依赖运行时）
> - `[ ]` 完全未动

每完成一项更新本表对应 Phase 一行的"完成项数"。

---

## 测试与回归

### 前端（vitest + happy-dom + @vue/test-utils）

```bash
cd tauri-app
npm test            # 单跑一次
npm run test:watch  # 文件改动自动重跑
```

当前测试文件：

- `src/composables/useCommandPalette.test.ts` —— 5 cases，验证 open/close/toggle + 单例
- `src/composables/useScanState.test.ts` —— 4 cases，验证最小展示 2s + reset 清定时器
- `src/router/router.test.ts` —— 6 cases，验证 5 个主路由 + tray-popup（meta.layout=bare）+ 重定向
- `src/lib/utils.test.ts` —— 12 cases，验证 cn / formatNumber / adapterLabel / 占位标题识别
- `src/views/tray-popup/tray-popup.test.ts` —— 6 cases，验证组件能 mount + 显示 mock 数据 + 点 session 条目先 invoke `show_main_window` 再 emit navigate（修复 close-to-tray 状态下点击无响应）+ footer 渲染 daemon 状态点 + 重启按钮（aria-label）
- `src/lib/utils.test.ts` —— 18 cases，新增 6 个 humanizeBackendError 用例（4 类常见错误转中文 + 兜底 + null/undefined）
- `src/stores/memex.test.ts` —— 14 cases，验证 adapter/totals/daemonStatus 响应式 + initMemexStore/refreshSessions/refreshProjects/refreshBreakdown 行为（mock invoke），含 breakdown 错误吞掉
- `src/views/library/components/LibraryFacets.test.ts` —— 7 cases，验证项目默认 8 条 + "+ N 更多"展开/收起 + 项目搜索（不区分大小写）+ 空状态 + 工具/项目"全选"emit 全部 id
- `src/views/library/components/LibrarySessionDrawer.test.ts` —— 5 cases，验证 detail 加载、消息全量渲染（≤50 条）、>50 条分页 + 加载更多增量、底部 4 个无功能按钮已移除、open 切换重新拉取
- `src/views/connect/components/AdaptersCard.test.ts` —— 4 cases，验证 7 个 adapter 渲染 + "立即扫描"触发 + "个会话"列从 breakdownByAdapter 取数 + toggleAdapter 防抖（值未变时跳过 IPC）

> 现状：12 文件 / 76 cases / **全绿**

### Rust 端（cargo test）

```bash
cd tauri-app/src-tauri
cargo test                          # 跑全部
cargo test --test ipc_contract      # 单独跑 IPC 契约
cargo test --test window_config     # 单独跑窗口配置回归
```

当前测试文件：

- `tests/ipc_contract.rs` —— 8 cases，pin Stats / DaemonStatus / LockInfo / Timeline / Breakdown 等给前端的 JSON 字段名
- `tests/window_config.rs` —— 3 cases，回归 tauri.conf.json 的 main / tray-popup 窗口尺寸 + capabilities

> 现状：2 文件 / 11 cases / **全绿**

### CI 跑测试的最小命令

```bash
# 一键完整 smoke：
cd tauri-app && npm install && npm test && cd src-tauri && cargo test
```

### 已补充的测试

- `views/connect/components/AdaptersCard.test.ts` —— 适配器列表 / 启停 / 单行扫描
- `views/connect/components/IdeIntegrationsCard.test.ts` —— IDE / SKILL / Hook 状态 + install/uninstall
- `views/settings/components/LlmTab.test.ts` —— Provider 列表 / 启用切换 / 测试连接 / Fallback 链路
- `views/settings/components/SystemTab.test.ts` —— CLI 状态 / Doctor / 检查更新

### 后续打算补的测试（P5 接线后再加）

- `useMemex.ts` 各 invoke 包装的 happy-path / error-path（mock `@tauri-apps/api/core`）
- Today / Library 真实数据接线后的 view-level integration test
- Deep link `memex://session/<id>` 解析的 router-level test
