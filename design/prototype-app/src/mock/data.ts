export type Adapter = 'claude_code' | 'cursor' | 'codex' | 'opencode' | 'aider' | 'continue' | 'cline'
export type AdapterStatus = 'active' | 'disabled'

export interface AdapterInfo {
  id: Adapter
  label: string
  /** Tailwind bg class — kept for legacy callers */
  color: string
  /** css var name from style.css */
  cssVar: string
  status: AdapterStatus
  sessions: number
  /** glob path the adapter watches */
  path: string
}

export const adapters: AdapterInfo[] = [
  {
    id: 'claude_code',
    label: 'Claude Code',
    color: 'bg-indigo-500',
    cssVar: 'var(--adapter-claude)',
    status: 'active',
    sessions: 3210,
    path: '~/.claude/projects/**/*.jsonl',
  },
  {
    id: 'cursor',
    label: 'Cursor',
    color: 'bg-emerald-500',
    cssVar: 'var(--adapter-cursor)',
    status: 'active',
    sessions: 2148,
    path: '~/Library/Application Support/Cursor/User/workspaceStorage/**/state.vscdb',
  },
  {
    id: 'codex',
    label: 'Codex',
    color: 'bg-orange-500',
    cssVar: 'var(--adapter-codex)',
    status: 'disabled',
    sessions: 321,
    path: '~/.codex/sessions/*.json',
  },
  {
    id: 'opencode',
    label: 'OpenCode',
    color: 'bg-violet-500',
    cssVar: 'var(--adapter-opencode)',
    status: 'active',
    sessions: 842,
    path: '~/.local/share/opencode/projects/**/*.jsonl',
  },
  {
    id: 'aider',
    label: 'Aider',
    color: 'bg-sky-500',
    cssVar: 'var(--adapter-aider)',
    status: 'disabled',
    sessions: 0,
    path: '~/.aider.chat.history.md',
  },
  {
    id: 'continue',
    label: 'Continue',
    color: 'bg-pink-500',
    cssVar: 'var(--adapter-continue)',
    status: 'disabled',
    sessions: 0,
    path: '~/.continue/sessions/*.json',
  },
  {
    id: 'cline',
    label: 'Cline',
    color: 'bg-orange-500',
    cssVar: 'var(--adapter-cline)',
    status: 'disabled',
    sessions: 0,
    path: '~/.cline/tasks/**/*.json',
  },
]

export const ADAPTER_MAP: Record<Adapter, AdapterInfo> = Object.fromEntries(
  adapters.map((a) => [a.id, a]),
) as Record<Adapter, AdapterInfo>

/** legacy alias so older imports keep working */
export const ADAPTERS = ADAPTER_MAP

export interface Session {
  id: string
  adapter: Adapter
  workspace: string
  project: string
  startedAt: string
  durationMin: number
  messages: number
  title: string
  topics: string[]
  l2Done: boolean
  interruptedAt?: string
  decisions?: string[]
  intent?: string
  next?: string[]
}

export const sessions: Session[] = [
  {
    id: 's-202606060942',
    adapter: 'cursor',
    workspace: 'memex',
    project: 'memex',
    startedAt: '2026-06-06T14:32:00+08:00',
    durationMin: 58,
    messages: 42,
    title: 'Memex menubar shadcn 重构原型',
    topics: ['ui', 'shadcn', 'vue', 'sidebar'],
    l2Done: true,
    interruptedAt: '在讨论 Tab → Sidebar 改造',
    intent: '统一交互语言、降低视觉噪音、为 5 个一级菜单建立稳定 shell',
    decisions: ['用 Sidebar block 替代手写 nav', '7 tab → 5 tab + ⌘K', 'Search 统一 ⌘K'],
    next: ['搭建 Vue prototype', '实现 5 个一级菜单', '验证组件一致性'],
  },
  {
    id: 's-202606061248',
    adapter: 'cursor',
    workspace: 'memex',
    project: 'memex',
    startedAt: '2026-06-06T12:48:00+08:00',
    durationMin: 41,
    messages: 28,
    title: 'shadcn-vue blocks 调研',
    topics: ['shadcn', 'blocks'],
    l2Done: true,
    intent: '挑选合适的 dashboard 布局作为 shell',
    decisions: ['选 dashboard-01 作为骨架', 'sidebar 用 collapsible="icon"'],
    next: ['批量装组件', '搭 5 个 view'],
  },
  {
    id: 's-202606052211',
    adapter: 'claude_code',
    workspace: 'tt-projects',
    project: 'tt-projects',
    startedAt: '2026-06-05T22:11:00+08:00',
    durationMin: 76,
    messages: 63,
    title: 'AsyncMQ 集成方案',
    topics: ['asyncmq', 'integration'],
    l2Done: true,
    interruptedAt: '在确认 ack 重试机制',
    intent: '让 tt-projects 串联起 AsyncMQ 异步任务',
    decisions: ['用 retry queue 而非 dead letter', '消费者用 at-least-once'],
    next: ['写消费者样例', '加 Grafana 监控'],
  },
  {
    id: 's-202606052141',
    adapter: 'cursor',
    workspace: 'memex',
    project: 'memex',
    startedAt: '2026-06-05T21:41:00+08:00',
    durationMin: 32,
    messages: 21,
    title: 'OverviewTab 数据加载重构',
    topics: ['vue', 'performance'],
    l2Done: true,
    intent: '把异步 12 个查询合成 1 个',
    decisions: ['新增 dashboard_overview RPC', '前端缓存 60s'],
    next: ['加骨架屏', '观测 P95'],
  },
  {
    id: 's-202606051010',
    adapter: 'codex',
    workspace: 'metadata-server',
    project: 'metadata-server',
    startedAt: '2026-06-05T10:10:00+08:00',
    durationMin: 41,
    messages: 28,
    title: '语义元数据 Domain 树字段补全',
    topics: ['java', 'mms'],
    l2Done: false,
    intent: '让 facet 文档导入能识别 sub_domain.linked_processes',
    decisions: ['新增 ProcessBindingEntity'],
    next: ['补 importer dry-run'],
  },
  {
    id: 's-202606041530',
    adapter: 'claude_code',
    workspace: 'tt-projects',
    project: 'launcher',
    startedAt: '2026-06-04T15:30:00+08:00',
    durationMin: 22,
    messages: 18,
    title: 'launcher 状态栏改成 Tauri menubar',
    topics: ['tauri', 'menubar'],
    l2Done: true,
    intent: '让常驻入口更轻量',
    decisions: ['弃用 tray + 主窗口双形态'],
    next: ['处理 macOS notch 适配'],
  },
]

export interface Project {
  id: string
  name: string
  path: string
  sessions: number
  lastActiveAt: string
  primaryAdapter: Adapter
  tags: string[]
}

export const projects: Project[] = [
  { id: 'p-memex', name: 'memex', path: '~/Documents/personal/tt-projects/memex', sessions: 84, lastActiveAt: '2026-06-06T14:32:00+08:00', primaryAdapter: 'cursor', tags: ['rust', 'tauri', 'vue'] },
  { id: 'p-tt', name: 'tt-projects', path: '~/Documents/personal/tt-projects', sessions: 56, lastActiveAt: '2026-06-05T22:11:00+08:00', primaryAdapter: 'claude_code', tags: ['mono', 'workspace'] },
  { id: 'p-metadata', name: 'metadata-server', path: '~/work/zoom/metadata-server', sessions: 41, lastActiveAt: '2026-06-05T10:51:00+08:00', primaryAdapter: 'codex', tags: ['java', 'spring-boot', 'mms'] },
  { id: 'p-launcher', name: 'launcher', path: '~/Documents/personal/tt-projects/launcher', sessions: 17, lastActiveAt: '2026-06-04T15:52:00+08:00', primaryAdapter: 'claude_code', tags: ['tauri', 'menubar'] },
]

export interface Reflection {
  id: string
  date: string
  topic: string
  weekSpan: string
  sessions: number
  summary: string
  unread?: boolean
}

export const reflections: Reflection[] = [
  {
    id: 'r-w23',
    date: '2026-06-06T09:12:00+08:00',
    topic: '你这周在 memex 上做了什么',
    weekSpan: '第 23 周',
    sessions: 67,
    summary: '本周主线是 memex 的 shadcn 重构与 MCP 工具调用统计设计。最重要的决策有 3 条：把 7 tab 收敛为 5 tab + 全局 ⌘K、用 Sidebar block 替代手写 nav、把 Search 入口完全统一到命令面板。',
    unread: true,
  },
  {
    id: 'r-w22',
    date: '2026-05-30T08:50:00+08:00',
    topic: '第 22 周反思',
    weekSpan: '第 22 周',
    sessions: 89,
    summary: '上周完成 launcher 的 menubar 重构、reflection prompts v2 上线、AsyncMQ 消费者初版。',
  },
]

export type ReportScope = 'daily' | 'weekly' | 'monthly'

export interface Report {
  id: string
  scope: ReportScope
  period: string
  date: string
  sessions: number
  topics: string[]
  body: string
}

export const reports: Report[] = [
  {
    id: 'd-20260606',
    scope: 'daily',
    period: '2026-06-06 周六',
    date: '2026-06-06',
    sessions: 12,
    topics: ['ui', 'shadcn', 'vue', 'refactor'],
    body: `## 主要工作\n- 完成 Memex 7→5 tab 重构原型\n- 调研 shadcn-vue blocks，选定 dashboard-01\n\n## 关键决策\n- 引入 Sidebar block 替代手写 nav\n- 把 search 全部收敛到 ⌘K\n\n## 主题\n[ui] [shadcn] [vue] [refactor]`,
  },
  {
    id: 'd-20260605',
    scope: 'daily',
    period: '2026-06-05 周五',
    date: '2026-06-05',
    sessions: 38,
    topics: ['perf', 'mms'],
    body: '## 主要工作\n- OverviewTab 性能重构\n- metadata-server domain 树字段补全',
  },
  {
    id: 'd-20260604',
    scope: 'daily',
    period: '2026-06-04 周四',
    date: '2026-06-04',
    sessions: 24,
    topics: ['tauri', 'menubar'],
    body: '## 主要工作\n- launcher 改 Tauri menubar',
  },
  {
    id: 'd-20260603',
    scope: 'daily',
    period: '2026-06-03 周三',
    date: '2026-06-03',
    sessions: 41,
    topics: ['ai-hub'],
    body: '## 主要工作\n- ai-hub-connector tool 编排',
  },
  {
    id: 'w-23',
    scope: 'weekly',
    period: '第 23 周 · 2026-06-01 ~ 2026-06-07',
    date: '2026-06-07',
    sessions: 67,
    topics: ['memex', 'shadcn', 'refactor'],
    body: '## 本周主线\n- memex shadcn 重构启动\n- OverviewTab P95 -38%\n- metadata-server domain 树完成',
  },
  {
    id: 'w-22',
    scope: 'weekly',
    period: '第 22 周 · 2026-05-25 ~ 2026-05-31',
    date: '2026-05-31',
    sessions: 89,
    topics: ['launcher', 'reflection'],
    body: '## 本周主线\n- launcher menubar 重构\n- reflection prompts v2 上线',
  },
  {
    id: 'm-202605',
    scope: 'monthly',
    period: '2026-05',
    date: '2026-05-31',
    sessions: 312,
    topics: ['memex', 'metadata-server', 'launcher'],
    body: '## 本月主线\n- 累计 312 个会话\n- memex 占比 52%\n- 反思 4 条',
  },
]

export interface IdeIntegration {
  id: 'cursor' | 'claude_code' | 'codex' | 'opencode'
  label: string
  mcpInstalled: boolean
  skillInstalled: boolean
  hookSupported: boolean
  hookInstalled: boolean
  configPath?: string
  skillPath?: string
}

export const ideIntegrations: IdeIntegration[] = [
  {
    id: 'cursor',
    label: 'Cursor',
    mcpInstalled: true,
    skillInstalled: true,
    hookSupported: false,
    hookInstalled: false,
    configPath: '~/.cursor/mcp.json',
    skillPath: '~/.cursor/skills/memex/',
  },
  {
    id: 'claude_code',
    label: 'Claude Code',
    mcpInstalled: true,
    skillInstalled: false,
    hookSupported: true,
    hookInstalled: true,
    configPath: '~/.claude/mcp.json',
    skillPath: '~/.claude/skills/memex/',
  },
  {
    id: 'codex',
    label: 'Codex',
    mcpInstalled: false,
    skillInstalled: false,
    hookSupported: false,
    hookInstalled: false,
  },
  {
    id: 'opencode',
    label: 'OpenCode',
    mcpInstalled: true,
    skillInstalled: true,
    hookSupported: false,
    hookInstalled: false,
    configPath: '~/.config/opencode/mcp.json',
  },
]

export interface McpTool {
  name: string
  description: string
  calls24h: number
  avgLatencyMs: number
  pctMaxLatency?: number
  live: boolean
}

export const mcpTools: McpTool[] = [
  { name: 'search_memory', description: '查询历史会话 (FTS5 + BM25)', calls24h: 47, avgLatencyMs: 24, pctMaxLatency: 80, live: true },
  { name: 'get_session', description: '按 ID 拉完整会话', calls24h: 12, avgLatencyMs: 18, live: true },
  { name: 'list_recent', description: '最近 N 条会话', calls24h: 8, avgLatencyMs: 12, live: true },
  { name: 'stats', description: '仓库统计数', calls24h: 3, avgLatencyMs: 9, live: true },
]

export interface McpCallEvent {
  ts: string
  client: Adapter
  tool: string
  args: string
  resultSummary: string
  latencyMs: number
}

export const mcpCallEvents: McpCallEvent[] = [
  { ts: '14:32:18', client: 'cursor', tool: 'search_memory', args: 'q="shadcn sidebar"', resultSummary: '12 条命中', latencyMs: 28 },
  { ts: '14:29:55', client: 'claude_code', tool: 'get_session', args: 'id="a7b3…"', resultSummary: '42 条消息', latencyMs: 19 },
  { ts: '14:18:02', client: 'cursor', tool: 'list_recent', args: 'limit=20', resultSummary: '20 项', latencyMs: 11 },
  { ts: '13:54:11', client: 'opencode', tool: 'search_memory', args: 'q="asyncmq"', resultSummary: '7 条命中', latencyMs: 23 },
  { ts: '13:48:30', client: 'cursor', tool: 'stats', args: '—', resultSummary: '6,521 个会话', latencyMs: 9 },
]

export interface WorkloadCell {
  date: string
  count: number
}

export const workload: WorkloadCell[] = (() => {
  const days = 91
  const today = new Date('2026-06-06T00:00:00+08:00')
  const out: WorkloadCell[] = []
  for (let i = days - 1; i >= 0; i--) {
    const d = new Date(today)
    d.setDate(d.getDate() - i)
    const seed = (d.getDay() + d.getDate() * 7) % 11
    out.push({ date: d.toISOString().slice(0, 10), count: seed > 7 ? 0 : seed })
  }
  return out
})()

export const habitHeatmap: number[][] = (() => {
  const out: number[][] = []
  for (let d = 0; d < 7; d++) {
    const row: number[] = []
    for (let h = 0; h < 24; h++) {
      let v = 0
      if (h >= 9 && h <= 12) v = ((d + h) * 7) % 4
      else if (h >= 14 && h <= 18) v = 1 + ((d + h) * 11) % 5
      else if (h >= 20 && h <= 23) v = ((d * 3 + h) * 5) % 6
      row.push(v)
    }
    out.push(row)
  }
  return out
})()

export const todayActivity = {
  hourlyBars: [0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 3, 2, 3, 4, 5, 5, 4, 3, 2, 1, 1, 0, 0, 0] as number[],
  sessions: 12,
  messages: 348,
  projects: 4,
  toolsUsed: 3,
  peakWindow: '14:00 – 16:00',
  byProject: [
    { name: 'memex', sessions: 8 },
    { name: 'tt-projects', sessions: 3 },
    { name: 'metadata-server', sessions: 1 },
  ],
}

export const daemonStatus = {
  running: true,
  startedAt: '2026-06-05T20:18:34+08:00',
  adapterActive: 5,
  adapterTotal: 7,
  llmProvider: 'Ollama',
  llmModel: 'qwen2.5',
  llmHealth: 'ok' as 'ok' | 'degraded' | 'down',
  storage: '~/.memex/ 824 MB',
  ftsHealth: 'ok' as 'ok' | 'degraded' | 'down',
  lastIngest: '2 分钟前',
}

export const totals = {
  sessions: 6521,
  messages: 184209,
  projects: 47,
}
