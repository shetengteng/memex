/**
 * MCP 工具调用展示用的纯格式化函数。
 *
 * 抽出来的目的有二：
 * 1) 让 `McpActivityCard.vue` / `McpCallDetailDialog.vue` 共用同一份格式
 *    化逻辑，避免"今天 vs 昨天"等微差异；
 * 2) 把主卡控制在 project.mdc 约定的 300 行上限内（视图组件本身已经够
 *    密），把展示样式无关的纯函数挪到独立 module。
 *
 * 这里的函数都是纯函数，无副作用，可以直接单元测试。
 */

/** 把 RFC3339 字符串渲染成 `YYYY-MM-DD HH:MM:SS`（本地时区）。 */
export function formatFullTime(s: string): string {
  const d = new Date(s)
  if (Number.isNaN(d.getTime())) return s
  const y = d.getFullYear()
  const mo = String(d.getMonth() + 1).padStart(2, '0')
  const da = String(d.getDate()).padStart(2, '0')
  const h = String(d.getHours()).padStart(2, '0')
  const m = String(d.getMinutes()).padStart(2, '0')
  const sec = String(d.getSeconds()).padStart(2, '0')
  return `${y}-${mo}-${da} ${h}:${m}:${sec}`
}

/**
 * 把 Date 渲染成相对当前时间的中文表达。
 * 5s 内 → "刚刚"；60s 内 → "N 秒前"；60min 内 → "N 分钟前"；
 * 24h 内 → "N 小时前"；超过 → "N 天前"。
 */
export function formatRelative(d: Date): string {
  const sec = Math.floor((Date.now() - d.getTime()) / 1_000)
  if (sec < 5) return '刚刚'
  if (sec < 60) return `${sec} 秒前`
  const min = Math.floor(sec / 60)
  if (min < 60) return `${min} 分钟前`
  const hr = Math.floor(min / 60)
  if (hr < 24) return `${hr} 小时前`
  return `${Math.floor(hr / 24)} 天前`
}

/**
 * 把毫秒延迟格式化成人类可读字符串。
 * < 10 ms 最多保留一位小数（整数无小数点）；< 1 s 显示整数；其它转秒保留两位小数。
 *
 * 修复：平均延迟来自 sum/count 可能是 8.666666666666666 这样的浮点数，
 * 原实现直接 `${ms} ms` 会原样把小数全打出来。
 */
export function formatLatency(ms: number): string {
  if (ms < 10) {
    const rounded = Math.round(ms * 10) / 10
    return `${rounded} ms`
  }
  if (ms < 1_000) return `${Math.round(ms)} ms`
  return `${(ms / 1_000).toFixed(2)} s`
}

/** 0 显示占位符 "—"，否则千分位格式化。 */
export function asMetric(n: number): string {
  if (n === 0) return '—'
  return n.toLocaleString()
}

/**
 * 详情对话框里渲染 `arguments_json` / `result_json` 的归一化结果。
 *
 * - `display`：最终拼好给 `<pre>` 用的字符串
 * - `isJson`：解析成功 → true（前端可以套 syntax highlight），否则当成 plain text
 * - `truncated`：后端落库时被截断（含 `[truncated` 标记）
 * - `empty`：原始字段是 null / undefined / ""
 */
export interface PrettyPayload {
  display: string
  isJson: boolean
  truncated: boolean
  empty: boolean
}

/**
 * Result payload 的常见形态是 `{"text": "...实际返回..."}` 或 `{"error": "..."}`，
 * 这两种 wrapper 由 `crates/memex-mcp/src/server/tools.rs::handle_tool_call` 注入。
 * 详情对话框对人类阅读体验要求高，wrapper 一层等同噪音，因此识别后**直接拆掉一层**，
 * 把内层 text/error 字符串作为主体；如果内层又是合法 JSON 再二次缩进。
 */
function unwrapTextEnvelope(parsed: unknown): unknown {
  if (parsed == null || typeof parsed !== 'object') return parsed
  const obj = parsed as Record<string, unknown>
  const keys = Object.keys(obj)
  if (keys.length === 1 && (keys[0] === 'text' || keys[0] === 'error')) {
    const inner = obj[keys[0]]
    if (typeof inner !== 'string') return parsed
    // 内层可能本身就是 JSON 字符串（search_memory 这种返回 pretty-printed JSON）
    try {
      return JSON.parse(inner)
    } catch {
      return inner
    }
  }
  return parsed
}

/**
 * 把后端落库的 payload 字符串变成对话框里能直接 `<pre>` 的展示形式。
 *
 * 流程：
 * 1. null / "" → empty=true，display="(无内容)"
 * 2. 含 `[truncated` 标记 → truncated=true，且把标记移出主体保留供 UI 醒目展示
 * 3. 尝试 JSON.parse；成功后对 `{"text":...}` / `{"error":...}` wrapper 拆一层
 * 4. parse 失败（被截断、或后端给的就不是 JSON）→ 整段当 plain text 返回
 */
export function prettifyPayload(raw: string | null | undefined): PrettyPayload {
  if (raw == null || raw === '') {
    return { display: '(无内容)', isJson: false, truncated: false, empty: true }
  }

  // truncate marker 由 Rust 端 `truncate_payload` 注入，形如
  //   `…[truncated N bytes]`（前面有个换行）
  const truncateRe = /\n?…\[truncated \d+ bytes\]\s*$/
  const truncated = truncateRe.test(raw)
  const body = truncated ? raw.replace(truncateRe, '') : raw

  try {
    const parsed = JSON.parse(body)
    const unwrapped = unwrapTextEnvelope(parsed)
    const display =
      typeof unwrapped === 'string'
        ? unwrapped
        : JSON.stringify(unwrapped, null, 2)
    return { display, isJson: true, truncated, empty: false }
  } catch {
    return { display: body, isJson: false, truncated, empty: false }
  }
}
