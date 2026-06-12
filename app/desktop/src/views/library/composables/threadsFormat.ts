/**
 * Threads 视图共用的纯展示 formatters。
 * 拆出来主要是让子组件直接 import，不要重复实现。
 */

export const dateRangeFmt = (start?: string | null, end?: string | null) => {
  if (!start || !end) return ''
  const s = new Date(start)
  const e = new Date(end)
  const fmt = (d: Date) =>
    d.toLocaleDateString('zh-CN', { month: 'numeric', day: 'numeric' })
  const days = Math.max(1, Math.round((e.getTime() - s.getTime()) / 86_400_000))
  return `${fmt(s)} → ${fmt(e)} · ${days} 天`
}

export const durationDays = (start?: string | null, end?: string | null) => {
  if (!start || !end) return 0
  const ms = new Date(end).getTime() - new Date(start).getTime()
  return Math.max(1, Math.round(ms / 86_400_000))
}

export const timeFmt = (iso: string) =>
  new Date(iso).toLocaleString('zh-CN', {
    month: 'numeric',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  })

export const lastProjectName = (path: string) => {
  const parts = path.split('/').filter(Boolean)
  return parts[parts.length - 1] ?? path
}

export const adapterLabel = (a: string) => {
  if (a === 'claude_code') return 'Claude Code'
  if (a === 'cursor') return 'Cursor'
  if (a === 'codex') return 'Codex'
  if (a === 'opencode') return 'OpenCode'
  return a
}
