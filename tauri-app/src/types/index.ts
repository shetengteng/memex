export interface Stats {
  sessions: number
  messages: number
  chunks: number
  db_exists: boolean
  summaries: number
  sessions_eligible_for_summary: number
  chunks_summarized: number
  llm_provider: string | null
}

/**
 * 单条会话的 IPC 形态，对齐后端 storage::db::SessionRow 在
 * `#[serde(rename_all = "camelCase")]` 下的 JSON 字段名。
 * 锁定形态见 crates/memex-core/src/storage/json_contract_tests.rs::test_session_row_json_fields。
 */
export interface SessionRow {
  id: string
  source: string
  projectPath: string | null
  title: string | null
  messageCount: number
  createdAt: string
  updatedAt: string
  summaryTitle: string | null
  firstUserMessage: string | null
  intent: string | null
}

/**
 * L5「主题线索」一行 —— `threads` 表 + 派生的 sessionCount + 卡片视图聚合字段。
 * 由后端 `list_threads` / `get_thread_detail` 返回，字段对齐 storage::db::ThreadRow
 * 在 `#[serde(rename_all = "camelCase")]` 下的 IPC 形态。
 *
 * `firstSessionAt` / `lastSessionAt` / `projects` / `adapters` 是由
 * thread_sessions + sessions join 后聚合得到，避免前端 N+1。
 */
export interface ThreadRow {
  id: number
  name: string
  summary: string
  sessionCount: number
  createdAt: string
  updatedAt: string
  firstSessionAt?: string | null
  lastSessionAt?: string | null
  projects?: string[]
  adapters?: string[]
}

/** 线索详情：基础信息 + 命中的 session 列表（复用 SessionRow 渲染）。 */
export interface ThreadDetail {
  thread: ThreadRow
  sessions: SessionRow[]
}

export interface SearchResult {
  chunk_id: number
  session_id: string
  message_id: string
  chunk_type: string
  content: string
  snippet: string
  rank: number
  match_reason: string
  adapter?: string
  project?: string
  timestamp?: string
}

/**
 * 会话详情的 IPC 形态，对齐后端 storage::db::SessionDetail 在
 * `#[serde(rename_all = "camelCase")]` 下的 JSON 字段名。
 */
export interface SessionDetail {
  id: string
  source: string
  projectPath: string | null
  title: string | null
  summary: string | null
  topics: string[]
  decisions: string[]
  filePath: string
  messageCount: number
  createdAt: string
  updatedAt: string
  messages: MessageRow[]
  intent: string | null
}

export interface MessageRow {
  id: string
  role: string
  content: string
  timestamp: string | null
}

/**
 * 资料库列表多维过滤（对齐后端 SessionListFilter）。
 *
 * 所有字段都是可选；后端把 None / 空数组 / 无法识别的字符串都视作"不过滤"。
 * 注意：后端用 `#[serde(deny_unknown_fields)]`，前端传任何笔误（如 `adapter`
 * 单数）都会让 invoke 立即报错——这是设计上的早失败，避免 UI 静默拿到错误结果。
 */
export interface SessionListFilter {
  adapters?: string[]
  projects?: string[]
  time?: 'today' | '7d' | '30d' | '90d' | 'all'
  summary?: 'all' | 'done' | 'pending'
  query?: string
  sort?: 'recent' | 'duration' | 'messages'
}

export interface StatsBreakdown {
  by_adapter: Record<string, number>
  by_project: Record<string, number>
  recent_7d_sessions: number
  recent_7d_messages: number
  recent_30d_sessions: number
  recent_30d_messages: number
}

export interface TimelineEntry {
  date: string
  adapter: string
  sessions: number
  messages: number
}

/**
 * `list_projects` IPC 返回的项目聚合行，对齐后端 ProjectSummary 在
 * `#[serde(rename_all = "camelCase")]` 下的 JSON 形态。
 * 锁定形态见 tauri-app/src-tauri/tests/ipc_contract.rs::project_summary_contract。
 */
export interface ProjectSummary {
  projectPath: string
  name: string
  sessionCount: number
  messageCount: number
  lastTitle: string | null
  lastUpdated: string
  byAdapter: Record<string, number>
}

export interface SummaryProgress {
  current: number
  total: number
  session_id: string
  success: boolean
  done: boolean
}

export interface AggregateSummary {
  id: number
  scope_type: 'daily' | 'weekly' | 'project' | string
  scope_key: string
  title: string | null
  summary: string
  topics: string[]
  decisions: string[]
  session_count: number
  created_at: string
}

export interface DaemonStatus {
  running: boolean
  pid: number | null
  port: number | null
  http_ok: boolean
  started_at: string | null
}

export interface CliStatus {
  path_contains_target_dir: boolean
  path: string
  target_dir: string
  installed: boolean
  path_export_hint: string
}

export interface LlmTestResult {
  ok: boolean
  latency_ms: number
  error: string | null
  response_text: string | null
  models_available?: string[]
  key_source?: string
}

export interface AdapterStatus {
  name: string
  file_count: number
  last_scan: string | null
}

export interface DoctorReport {
  db_exists: boolean
  schema_version: number | null
  session_count: number
  message_count: number
  chunk_count: number
  source_count: number
  fts_ok: boolean
  adapters: AdapterStatus[]
}

export type CursorProbe =
  | { status: 'ok'; composer_count: number; db_path: string }
  | { status: 'not_found'; db_path: string }
  | { status: 'permission_denied'; db_path: string; message: string }
  | { status: 'error'; db_path: string; message: string }

export interface DoctorRunResult {
  data_dir: string
  config_present: boolean
  report: DoctorReport
  cursor_probe: CursorProbe
}

export interface ResetReport {
  removed_files: number
  removed_bytes: number
}

export interface SystemResetResult {
  mode: 'index' | 'all'
  report: ResetReport
}

export interface LlmProvider {
  id: string
  name: string
  kind: string
  baseUrl: string
  model: string
  apiKey: string
  enabled: boolean
  isDefault: boolean
  status: string
  latencyMs: number | null
  updatedAt: string
}

export interface ProviderTestResult {
  ok: boolean
  latencyMs: number
  error: string | null
  responseText: string | null
}

export type ViewName = 'search' | 'settings' | 'status' | 'session' | 'dashboard'

export interface ReflectEntry {
  scope_key: string
  title: string | null
  digest_count: number
  created_at: string
}

export interface ReflectDetail {
  scope_key: string
  title: string | null
  markdown: string
  patterns: string[]
  open_loops: string[]
  digest_count: number
  created_at: string
}

export interface ReflectRunResult {
  scope_key: string
  period_label: string
  digest_count: number
  markdown: string
  shipped: string[]
  patterns: string[]
  open_loops: string[]
}

export interface WorkloadHeatmapCell {
  weekday: number // 0=Mon, 6=Sun
  hour: number // 0..23
  sessions: number
  messages: number
}

export interface WorkloadBucket {
  key: string
  sessions: number
  messages: number
}

export interface WorkloadProjectBucket {
  project_path: string
  name: string
  sessions: number
  messages: number
}

export interface WorkloadOverall {
  sessions: number
  messages: number
  active_days: number
  peak_day: string | null
  peak_day_sessions: number
}

export interface WorkloadDailyEntry {
  date: string // YYYY-MM-DD
  sessions: number
  messages: number
}

export interface WorkloadReport {
  days: number
  daily: WorkloadDailyEntry[]
  heatmap: WorkloadHeatmapCell[]
  by_adapter: WorkloadBucket[]
  by_project: WorkloadProjectBucket[]
  overall: WorkloadOverall
}

export interface IdeStatus {
  ide: string
  config_path: string
  config_exists: boolean
  installed: boolean
  command: string | null
}

export interface SkillStatus {
  ide: string
  dest_path: string
  installed: boolean
  size: number | null
}

export interface HookStatus {
  ide: string
  supported: boolean
  installed: boolean
  config_path: string
  wrapper_path: string | null
}

export interface UpdateInfo {
  latest_tag: string
  html_url: string
}
