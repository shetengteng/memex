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
  if (diffMin < 60) return `${diffMin}m ago`
  if (diffHr < 24) return `${diffHr}h ago`
  if (diffDay < 7) return `${diffDay}d ago`
  return new Date(dateStr).toLocaleDateString()
}

export function adapterLabel(source: string): string {
  const map: Record<string, string> = {
    claude_code: 'Claude',
    cursor: 'Cursor',
    codex: 'Codex',
    opencode: 'OpenCode',
  }
  return map[source] ?? source
}

export function adapterColor(source: string): string {
  const map: Record<string, string> = {
    claude_code: 'bg-adapter-claude',
    cursor: 'bg-adapter-cursor',
    codex: 'bg-adapter-codex',
    opencode: 'bg-adapter-opencode',
  }
  return map[source] ?? 'bg-muted'
}
