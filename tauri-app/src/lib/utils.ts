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

export function formatNumber(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1).replace(/\.0$/, '') + 'M'
  if (n >= 1_000) return (n / 1_000).toFixed(1).replace(/\.0$/, '') + 'K'
  return n.toLocaleString('en-US')
}
