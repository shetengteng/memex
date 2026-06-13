/**
 * Threads 视图共用的纯展示 formatters。
 * 拆出来主要是让子组件直接 import，不要重复实现。
 *
 * i18n 适配：日期 / 时间格式化函数支持可选 `locale`（默认 'zh-CN'，保持向后兼容
 * 与单测稳定性）；带「天」后缀的 dateRangeFmt 还接受可选 `daysSuffix`，
 * 调用方传入翻译后的 `天` / `days`，模板里再用 t() 拼装即可。
 */

export const dateRangeFmt = (
  start?: string | null,
  end?: string | null,
  locale: string = 'zh-CN',
  daysSuffix: string = '天',
) => {
  if (!start || !end) return ''
  const s = new Date(start)
  const e = new Date(end)
  const fmt = (d: Date) =>
    d.toLocaleDateString(locale, { month: 'numeric', day: 'numeric' })
  const days = Math.max(1, Math.round((e.getTime() - s.getTime()) / 86_400_000))
  return `${fmt(s)} → ${fmt(e)} · ${days} ${daysSuffix}`
}

export const durationDays = (start?: string | null, end?: string | null) => {
  if (!start || !end) return 0
  const ms = new Date(end).getTime() - new Date(start).getTime()
  return Math.max(1, Math.round(ms / 86_400_000))
}

export const timeFmt = (iso: string, locale: string = 'zh-CN') =>
  new Date(iso).toLocaleString(locale, {
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
