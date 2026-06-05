import { type ClassValue, clsx } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export function timeAgo(dateStr: string): string {
  const now = Date.now()
  const then = new Date(dateStr).getTime()
  const diffMs = now - then
  const diffMin = Math.floor(diffMs / 60000)
  const diffHr = Math.floor(diffMs / 3600000)
  const diffDay = Math.floor(diffMs / 86400000)

  if (diffMin < 1) return 'just now'
  if (diffMin < 60) return `${diffMin}m`
  if (diffHr < 24) return `${diffHr}h`
  if (diffDay < 7) return `${diffDay}d`
  return new Date(dateStr).toLocaleDateString()
}

const adapterMap: Record<string, { label: string; abbr: string; color: string; bg: string }> = {
  claude_code: { label: 'Claude Code', abbr: 'CC', color: 'text-adapter-claude', bg: 'bg-adapter-claude/12' },
  cursor:      { label: 'Cursor',      abbr: 'Cu', color: 'text-adapter-cursor', bg: 'bg-adapter-cursor/12' },
  codex:       { label: 'Codex',       abbr: 'Cx', color: 'text-adapter-codex',  bg: 'bg-adapter-codex/12' },
  opencode:    { label: 'OpenCode',    abbr: 'OC', color: 'text-adapter-opencode', bg: 'bg-adapter-opencode/12' },
  aider:       { label: 'Aider',       abbr: 'Ai', color: 'text-adapter-aider', bg: 'bg-adapter-aider/12' },
  continue_dev:{ label: 'Continue',    abbr: 'Cn', color: 'text-adapter-continue', bg: 'bg-adapter-continue/12' },
  cline:       { label: 'Cline',       abbr: 'Cl', color: 'text-adapter-cline', bg: 'bg-adapter-cline/12' },
}

export function adapterLabel(source: string): string {
  return adapterMap[source]?.label ?? source
}

export function adapterAbbr(source: string): string {
  return adapterMap[source]?.abbr ?? source.slice(0, 2).toUpperCase()
}

export function adapterColor(source: string): string {
  return adapterMap[source]?.color ?? 'text-muted-foreground'
}

export function adapterBg(source: string): string {
  return adapterMap[source]?.bg ?? 'bg-muted'
}

export function formatTime(dateStr: string | null): string {
  if (!dateStr) return ''
  const d = new Date(dateStr)
  const hh = String(d.getHours()).padStart(2, '0')
  const mm = String(d.getMinutes()).padStart(2, '0')
  return `${hh}:${mm}`
}

export function formatNumber(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1).replace(/\.0$/, '') + 'M'
  if (n >= 1_000) return (n / 1_000).toFixed(1).replace(/\.0$/, '') + 'K'
  return n.toLocaleString('en-US')
}

// 数据库里历史遗留的占位标题：opencode 在 session 创建瞬间会写
// "New session - 2026-01-23T..."；cursor 早期版本也会留下 "Conversation initiation"
// 之类的通用句。这些标题没有任何语义，UI 应当把它们当作"没标题"，
// 在 fallback 链里跳过、改用 first_user_message。
//
// 后端（opencode.rs / cursor/sqlite.rs）现在已经在 scan 阶段过滤这些 title，
// 但数据库里已经入库的历史会话仍然带着这些 title，所以前端这道防线必须保留。
const GENERIC_TITLE_PATTERNS: RegExp[] = [
  /^new session\s*-\s*\d{4}-\d{2}-\d{2}t/i,
  /^new session$/i,
  /^new conversation$/i,
  /^conversation initiation$/i,
  /^conversation start$/i,
  /^start the conversation$/i,
  /^start of the conversation$/i,
  /^开始对话$/,
  /^新对话$/,
  /^新的对话$/,
  /^继续讨论$/,
  /^prompts file discussion$/i,
  /^prompts from prompts\.txt$/i,
]

export function isPlaceholderTitle(s: string | null | undefined): boolean {
  if (!s) return true
  const t = s.trim()
  if (!t) return true
  return GENERIC_TITLE_PATTERNS.some((re) => re.test(t))
}

/** 返回干净有意义的 title；为空或匹配占位模板时返回 null。 */
export function meaningfulTitle(s: string | null | undefined): string | null {
  if (isPlaceholderTitle(s)) return null
  return s!.trim()
}
