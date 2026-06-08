/**
 * 全局响应式 Memex store。
 *
 * 设计目标：把原型 `src/mock/data.ts` 里的字面量替换成 reactive ref，
 * 让所有 view 在不改 import 路径的情况下（统一改用 `@/stores/memex`）
 * 切到真实 IPC 数据。
 *
 * 启动流程：
 *   - main.ts 调用 `initMemexStore()` 一次 → 拉 sessions / projects / stats / daemon
 *   - useStats / useDaemon 内部各自做轮询，写回这里的 ref
 *   - 组件直接读 ref 即可，Vue 自动响应
 *
 * 静态部分（adapter 元信息、MCP 占位 demo 数据）暂时保留原型 mock 值，
 * Phase 4.6 接线 Connect 页时再替换。
 */

import { reactive, ref, watch, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type {
  DaemonStatus as IpcDaemonStatus,
  ProjectSummary,
  SessionRow,
  Stats,
  StatsBreakdown,
  ThreadDetail,
  ThreadRow,
} from '@/types'
import { useMemex } from '@/composables/useMemex'
import { meaningfulPrompt } from '@/lib/utils'

// ============================================================
// Adapter 静态元信息（暂时沿用原型）
// ============================================================

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

const ADAPTER_META: AdapterInfo[] = [
  { id: 'claude_code', label: 'Claude Code', color: 'bg-indigo-500',  cssVar: 'var(--adapter-claude)',   status: 'active',   sessions: 0, path: '~/.claude/projects/**/*.jsonl' },
  { id: 'cursor',      label: 'Cursor',      color: 'bg-emerald-500', cssVar: 'var(--adapter-cursor)',   status: 'active',   sessions: 0, path: '~/Library/Application Support/Cursor/User/workspaceStorage/**/state.vscdb' },
  { id: 'codex',       label: 'Codex',       color: 'bg-orange-500',  cssVar: 'var(--adapter-codex)',    status: 'disabled', sessions: 0, path: '~/.codex/sessions/*.json' },
  { id: 'opencode',    label: 'OpenCode',    color: 'bg-violet-500',  cssVar: 'var(--adapter-opencode)', status: 'active',   sessions: 0, path: '~/.local/share/opencode/projects/**/*.jsonl' },
  { id: 'aider',       label: 'Aider',       color: 'bg-sky-500',     cssVar: 'var(--adapter-aider)',    status: 'disabled', sessions: 0, path: '~/.aider.chat.history.md' },
  { id: 'continue',    label: 'Continue',    color: 'bg-pink-500',    cssVar: 'var(--adapter-continue)', status: 'disabled', sessions: 0, path: '~/.continue/sessions/*.json' },
  { id: 'cline',       label: 'Cline',       color: 'bg-orange-500',  cssVar: 'var(--adapter-cline)',    status: 'disabled', sessions: 0, path: '~/.cline/tasks/**/*.json' },
]

// 用 reactive 而不是 ref：组件可以直接读 `adapters.length` / `adapters.map` /
// `ADAPTER_MAP['cursor']`，无需 `.value`，21 个原型组件不必改写。
export const adapters: AdapterInfo[] = reactive(ADAPTER_META.map((a) => ({ ...a })))

// ADAPTER_MAP 还是用 computed —— 但读它的对象属性时（`ADAPTER_MAP['cursor']`）
// 会自动通过 proxy 访问 .value.cursor，所以我们改成普通 reactive Record。
export const ADAPTER_MAP: Record<string, AdapterInfo> = reactive(
  Object.fromEntries(adapters.map((a) => [a.id, a])) as Record<string, AdapterInfo>,
)
export const ADAPTERS = ADAPTER_MAP

// ============================================================
// Session 列表（真实数据：listRecent(200)）
// ============================================================

/**
 * 原型 Session 形状（适配 UI 字段）。后端 SessionRow 字段映射成这个形状。
 */
export interface Session {
  id: string
  adapter: Adapter | string
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

// reactive 数组：组件可以 `sessions.slice(0, 5)` / `sessions.length`，
// 后端数据通过 `sessions.splice(0, sessions.length, ...rows)` 同步进来，
// 视图自动响应。
export const sessions: Session[] = reactive([])

export function rowToSession(row: SessionRow): Session {
  // 用 project_path 末段当 workspace/project 名；title 缺失时退到 first_user_message
  const projName = row.project_path
    ? row.project_path.split('/').filter(Boolean).pop() ?? row.project_path
    : '(未知项目)'
  // claude_code 等 IDE 的 workflow agent 框架会把 `=== Role === ...` 这种
  // system prompt 模板写进第一条 user message。这种内容做标题 / intent fallback
  // 完全是噪音，必须过滤掉。
  const cleanFirstPrompt = meaningfulPrompt(row.first_user_message)
  const title =
    (row.summary_title && row.summary_title.trim()) ||
    (row.title && row.title.trim()) ||
    (cleanFirstPrompt && cleanFirstPrompt.slice(0, 60)) ||
    '(无标题)'
  // intent 优先取 LLM 摘要里推断出来的；摘要还没生成时退到第一条 user 消息预览，
  // 让列表行依然有第二行有意义的辅助文字（LibrarySessionListItem 会读 session.intent）。
  const intent =
    (row.intent && row.intent.trim()) ||
    (cleanFirstPrompt && cleanFirstPrompt.slice(0, 120)) ||
    undefined
  return {
    id: row.id,
    adapter: row.source,
    workspace: projName,
    project: projName,
    startedAt: row.created_at,
    durationMin: computeDurationMin(row.created_at, row.updated_at),
    messages: row.message_count,
    title,
    topics: [],
    l2Done: !!row.summary_title,
    intent,
  }
}

// 后端 sessions 表里的 created_at / updated_at 都直接来自 adapter（claude /
// cursor / codex 的 session 文件 ctime/mtime），所以两者差值就是会话的实际持续
// 时长。失败兜底返回 0，避免 NaN 漏到 UI 上。
export function computeDurationMin(
  createdAt: string | null | undefined,
  updatedAt: string | null | undefined,
): number {
  if (!createdAt || !updatedAt) return 0
  const start = Date.parse(createdAt)
  const end = Date.parse(updatedAt)
  if (!Number.isFinite(start) || !Number.isFinite(end)) return 0
  const diffMs = end - start
  if (diffMs <= 0) return 0
  return Math.max(1, Math.round(diffMs / 60_000))
}

// ============================================================
// Project 列表
// ============================================================

export interface Project {
  id: string
  name: string
  path: string
  sessions: number
  lastActiveAt: string
  primaryAdapter: Adapter | string
  tags: string[]
}

export const projects: Project[] = reactive([])

function projectSummaryToProject(p: ProjectSummary): Project {
  const primary = Object.entries(p.by_adapter)
    .sort((a, b) => b[1] - a[1])[0]?.[0] ?? 'cursor'
  return {
    id: p.project_path,
    name: p.name || p.project_path,
    path: p.project_path,
    sessions: p.session_count,
    lastActiveAt: p.last_updated,
    primaryAdapter: primary,
    tags: [],
  }
}

// ============================================================
// Stats / Daemon（响应式镜像，由 useStats / useDaemon 写回）
// ============================================================

// stats / daemon：用 ref 暴露给 composable 写回，同时用 reactive 容器
// `totals` / `daemonStatus` 给 view 直接读字段（不用 .value）。
export const stats: Ref<Stats | null> = ref(null)
export const daemon: Ref<IpcDaemonStatus | null> = ref(null)

// 当前操作系统登录用户名，用于 Today 页 "晚上好，xxx" 问候。
// 由 initMemexStore 一次性调 invoke('get_system_username') 填入。
// 默认 'User' 让 SSR/单测/无 Tauri 环境也能渲染。
export const userName: Ref<string> = ref('User')

// 按 adapter 的会话数（来自 get_breakdown.by_adapter）。
// LibraryFacets 等组件读这个字段展示每个适配器的会话计数。
export const breakdownByAdapter: Record<string, number> = reactive({})

// totals 是 reactive object，组件直接读 `totals.sessions`。
// 它由 watchEffect 同步而不是 computed，避免 reactive + computed 的解包问题。
export const totals = reactive({
  sessions: 0,
  messages: 0,
  projects: 0,
})

// 同步 stats → totals
watch(
  [stats, () => projects.length],
  () => {
    totals.sessions = stats.value?.sessions ?? 0
    totals.messages = stats.value?.messages ?? 0
    totals.projects = projects.length
  },
  { immediate: true },
)

/**
 * UI 友好版的 daemon 状态。包含原型 `daemonStatus` 的全部字段，
 * 但 running / startedAt 由 IPC 同步，剩余字段先给静态/计算值。
 * 用 reactive 而非 computed，组件可直接读 `daemonStatus.running`。
 */
export const daemonStatus = reactive({
  running: false,
  startedAt: '',
  adapterActive: 0,
  adapterTotal: 7,
  llmProvider: 'Ollama',
  llmModel: 'qwen2.5',
  llmHealth: 'ok' as 'ok' | 'degraded' | 'down',
  storage: '~/.memex/',
  ftsHealth: 'ok' as 'ok' | 'degraded' | 'down',
  lastIngest: '',
})

// 同步 daemon / stats / adapters → daemonStatus
watch(
  [daemon, stats, () => adapters.length, () => adapters.filter((a) => a.status === 'active').length],
  () => {
    daemonStatus.running = daemon.value?.running ?? false
    daemonStatus.startedAt = daemon.value?.started_at ?? ''
    daemonStatus.adapterActive = adapters.filter((a) => a.status === 'active').length
    daemonStatus.adapterTotal = adapters.length
    daemonStatus.llmProvider = stats.value?.llm_provider ?? 'Ollama'
  },
  { immediate: true },
)

// ============================================================
// 静态原型 demo 数据（reflections / reports / mcpTools / heatmap）
// 这些一期保留 mock，后续 Phase 4.5 / 4.6 接线时再替换。
// ============================================================

export interface Reflection {
  id: string
  date: string
  topic: string
  weekSpan: string
  sessions: number
  summary: string
  unread?: boolean
}
export const reflections: Reflection[] = reactive([])

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
export const reports: Report[] = reactive([])

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
export const ideIntegrations: IdeIntegration[] = reactive([])

export interface WorkloadCell {
  date: string
  count: number
}
export const workload: WorkloadCell[] = reactive([])

export const habitHeatmap: number[][] = reactive([])

// 兜底"今日"卡片数据（Phase 4.3 接线后会变成 computed-from-IPC）
export interface TodayActivity {
  hourlyBars: number[]
  sessions: number
  messages: number
  projects: number
  toolsUsed: number
  peakWindow: string
  byProject: { name: string; sessions: number }[]
}
export const todayActivity: TodayActivity = reactive({
  hourlyBars: Array(24).fill(0),
  sessions: 0,
  messages: 0,
  projects: 0,
  toolsUsed: 0,
  peakWindow: '—',
  byProject: [],
})

// ============================================================
// 初始化 / 刷新
// ============================================================

let inited = false

/**
 * 启动时调用一次，把 sessions / projects / breakdown 拉过来。
 * stats / daemon 由对应 composable 自动轮询写回。
 *
 * 幂等 —— 多次调用只跑一次（除非 force=true）。
 */
export async function initMemexStore(force = false): Promise<void> {
  if (inited && !force) return
  inited = true
  const memex = useMemex()
  await Promise.allSettled([
    memex.listRecent(200).then((rows) => {
      sessions.splice(0, sessions.length, ...rows.map(rowToSession))
    }),
    memex.listProjects().then((ps) => {
      projects.splice(0, projects.length, ...ps.map(projectSummaryToProject))
    }),
    memex.getBreakdown().then(applyBreakdown),
    invoke<string>('get_system_username')
      .then((name) => {
        if (name && name.trim()) userName.value = name.trim()
      })
      .catch(() => {
        // 静默：非 Tauri 环境（单测、SSR）继续用默认 'User'
      }),
  ])
}

function applyBreakdown(b: StatsBreakdown): void {
  // 用 splice/赋值覆盖以保证响应式
  for (const k of Object.keys(breakdownByAdapter)) {
    delete breakdownByAdapter[k]
  }
  for (const [k, v] of Object.entries(b.by_adapter)) {
    breakdownByAdapter[k] = v
  }
}

/** 主动重拉 sessions（采集完成后用） */
export async function refreshSessions(limit = 200): Promise<void> {
  const memex = useMemex()
  try {
    const rows = await memex.listRecent(limit)
    sessions.splice(0, sessions.length, ...rows.map(rowToSession))
  } catch {
    /* swallow */
  }
}

/**
 * 增量加载更多 sessions（分页）。
 * - offset = 当前 sessions.length
 * - limit = pageSize
 * - 返回 { loaded, hasMore }
 *   - loaded = 实际新增条数
 *   - hasMore = 后端是否还有数据（loaded === pageSize 才认为可能还有）
 */
export async function loadMoreSessions(pageSize = 100): Promise<{ loaded: number; hasMore: boolean }> {
  const memex = useMemex()
  try {
    const rows = await memex.listRecent(pageSize, sessions.length)
    if (rows.length === 0) return { loaded: 0, hasMore: false }
    // 用 push 追加，保持已有列表中 drawer 选中态等不变
    sessions.push(...rows.map(rowToSession))
    return { loaded: rows.length, hasMore: rows.length === pageSize }
  } catch {
    return { loaded: 0, hasMore: false }
  }
}

/** 主动重拉 projects */
export async function refreshProjects(): Promise<void> {
  const memex = useMemex()
  try {
    const ps = await memex.listProjects()
    projects.splice(0, projects.length, ...ps.map(projectSummaryToProject))
  } catch {
    /* swallow */
  }
}

/** 主动重拉 breakdown（adapter 维度的会话数） */
export async function refreshBreakdown(): Promise<void> {
  const memex = useMemex()
  try {
    applyBreakdown(await memex.getBreakdown())
  } catch {
    /* swallow */
  }
}

// ============================================================
// L5「主题线索（Threads）」
// ============================================================
//
// 后端 IPC：
//   - list_threads(limit, offset) → ThreadRow[]
//   - get_thread_detail(thread_id) → ThreadDetail | null
//   - regenerate_threads() → number（新建/更新的 thread 数）
//
// store 只缓存"线索列表"和"当前打开的详情"。详情是按需拉的，因为我们想
// 让点击不同 thread 时立刻看到最新 session 列表，不靠列表内嵌的 stale 数据。

export const threads: ThreadRow[] = reactive([])

/** 主动重拉线索列表。失败时静默吞掉，沿用其它 refresh* 的模式。 */
export async function refreshThreads(limit = 100): Promise<void> {
  try {
    const rows = await invoke<ThreadRow[]>('list_threads', { limit, offset: 0 })
    threads.splice(0, threads.length, ...rows)
  } catch {
    /* swallow */
  }
}

/** 拉单条线索详情（包含命中的 session 列表，对应 LibrarySessionListItem 直接渲染）。 */
export async function fetchThreadDetail(threadId: number): Promise<ThreadDetail | null> {
  try {
    return await invoke<ThreadDetail | null>('get_thread_detail', { threadId })
  } catch {
    return null
  }
}

/**
 * 手动触发 LLM 重新聚类。返回新建/更新的 thread 数。
 * 阻塞调用（一次 LLM 调用），调用方负责显示 loading 状态。
 */
export async function regenerateThreads(): Promise<number> {
  try {
    const n = await invoke<number>('regenerate_threads')
    await refreshThreads()
    return n
  } catch (e) {
    throw e instanceof Error ? e : new Error(String(e))
  }
}
