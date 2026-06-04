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

export interface SessionRow {
  id: string
  source: string
  project_path: string | null
  title: string | null
  message_count: number
  created_at: string
  updated_at: string
  summary_title: string | null
  first_user_message: string | null
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

export interface SessionDetail {
  id: string
  source: string
  project_path: string | null
  title: string | null
  summary: string | null
  topics: string[]
  decisions: string[]
  file_path: string
  message_count: number
  created_at: string
  updated_at: string
  messages: MessageRow[]
}

export interface MessageRow {
  id: string
  role: string
  content: string
  timestamp: string | null
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

export interface ProjectSummary {
  project_path: string
  name: string
  session_count: number
  message_count: number
  last_title: string | null
  last_updated: string
  by_adapter: Record<string, number>
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

export interface WorkloadReport {
  days: number
  heatmap: WorkloadHeatmapCell[]
  by_adapter: WorkloadBucket[]
  by_project: WorkloadProjectBucket[]
  overall: WorkloadOverall
}
