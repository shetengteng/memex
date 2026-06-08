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

/**
 * 识别"第一条 user 消息"里 IDE agent 框架塞进来的 system prompt 模板。
 *
 * 典型来源：claude_code workflow agent 把整段
 * `=== Role === / === Skills === / === Task ===` 写进 user message 的 content
 * 字段。这类内容对用户无意义，不能当 fallback 标题或 intent 展示。
 *
 * 后端 `crates/memex-core/src/context/builder.rs::is_noise_prompt` 有等价逻辑，
 * 任何一边变动都建议同步另一边。
 */
const NOISE_PROMPT_PREFIXES = [
  '=== Role',
  '=== Task',
  '=== Skills',
  '=== System',
  '=== Goal',
]

export function isNoisePrompt(s: string | null | undefined): boolean {
  if (!s) return true
  const trimmed = s.trimStart()
  if (NOISE_PROMPT_PREFIXES.some((p) => trimmed.startsWith(p))) {
    return true
  }
  // 极短或只有标点 / 空白的 fallback。
  const visible = trimmed.replace(/\s+/g, '')
  return visible.length < 2
}

/** 返回干净有意义的 user prompt 预览；匹配 noise 模板时返回 null。 */
export function meaningfulPrompt(s: string | null | undefined): string | null {
  if (isNoisePrompt(s)) return null
  return s!.trim()
}

/**
 * 后端错误（Rust `Result::Err(String)`）原文常常是英文工程化的，对终端用户不友好。
 * 这里把已知的几类常见错误匹配成中文友好版本，并附带提示用户怎么解决。
 *
 * 设计：
 * - 匹配命中：返回 { friendly: 中文提示, action?: { label, route } }
 * - 不命中：返回 { friendly: 原文 } 兜底
 *
 * 调用方在 toast.error / 错误面板里直接展示 friendly，可选的 action 给"去设置"按钮。
 */
export interface FriendlyBackendError {
  friendly: string
  action?: { label: string; route: string }
}

export function humanizeBackendError(e: unknown): FriendlyBackendError {
  const raw = String(e ?? '').trim()

  // No LLM provider available. Enable Ollama or configure a custom LLM provider.
  if (/no llm provider available/i.test(raw)) {
    return {
      friendly: '当前没有可用的 LLM 服务。请先在设置中启用 Ollama 或配置 Claude API。',
      action: { label: '去设置', route: '/settings' },
    }
  }

  // Ollama not running / connection refused
  if (/connection refused|ollama.*(unreachable|not running|not found)/i.test(raw)) {
    return {
      friendly: '无法连接 Ollama 服务，请确认 ollama serve 已启动。',
      action: { label: '去设置', route: '/settings' },
    }
  }

  // 401 / 403 — API key 无效
  if (/401|403|unauthorized|forbidden|invalid.*api.*key/i.test(raw)) {
    return {
      friendly: 'LLM API Key 无效或权限不足，请在设置中重新配置。',
      action: { label: '去设置', route: '/settings' },
    }
  }

  // 至少需要 N 条消息
  if (/at least \d+ messages?|too few messages/i.test(raw)) {
    return { friendly: '会话消息太少，至少需要 2 条消息才能生成摘要。' }
  }

  return { friendly: raw || '未知错误' }
}
