/**
 * `library/index.vue` 的过滤 + 排序 + 分组逻辑（纯函数，无状态）。
 *
 * 拆出来的目的：
 *  1. 让 503 行的 index.vue 向项目"≤ 300 行"规约靠拢
 *  2. 纯函数易做单测，避免 mount 整个组件 + stub store/router
 *  3. 时间过滤 / 分组基准两条 buggy 逻辑得以隔离修复
 *
 * 行为契约：
 *  - 时间窗口（`fTime`）以**真实当前时间**为基准，**不是**结果集中最新一条 session
 *  - 分组的"今天 / 昨天 / 本周"同样以真实当前时间为基准
 *  - `now` 参数注入是为了单测可控；运行时调用方传 `new Date()` 或不传走默认
 */

import type { Session } from '@/stores/memex'

export type TimeFilter = 'today' | '7d' | '30d' | '90d' | 'all'
export type SummaryFilter = 'all' | 'done' | 'pending'
export type SortKey = 'recent' | 'duration' | 'messages'

export interface SessionFilters {
  adapters: string[]
  projects: string[]
  summary: SummaryFilter
  query: string
  time: TimeFilter
  sort: SortKey
}

export interface SessionGroup {
  key: 'today' | 'yesterday' | 'week' | 'earlier'
  label: string
  sessions: Session[]
}

const MS_PER_DAY = 24 * 60 * 60 * 1000

function startOfDay(d: Date): Date {
  const r = new Date(d)
  r.setHours(0, 0, 0, 0)
  return r
}

/**
 * 计算 `time` 过滤的下界（含），返回 null 表示不过滤。
 *
 * 语义：
 *  - 'today'  → 今天 00:00:00
 *  - '7d'     → 6 天前的 00:00:00（含今天 = 7 个完整日历日）
 *  - '30d'    → 29 天前的 00:00:00
 *  - '90d'    → 89 天前的 00:00:00
 *  - 'all'    → null
 */
export function computeTimeLowerBound(time: TimeFilter, now: Date): Date | null {
  if (time === 'all') return null
  const today0 = startOfDay(now)
  if (time === 'today') return today0
  const days = time === '7d' ? 7 : time === '30d' ? 30 : 90
  return new Date(today0.getTime() - (days - 1) * MS_PER_DAY)
}

/**
 * 过滤 + 排序。纯函数，不修改输入数组。
 *
 * 过滤顺序（成本由低到高）：
 *  1. fAdapters / fProjects / fSummary —— O(1) 字段相等
 *  2. fTime —— O(1) 时间比较
 *  3. query —— O(n) 字符串构造 + indexOf
 */
export function filterAndSortSessions(
  sessions: readonly Session[],
  filters: SessionFilters,
  now: Date = new Date(),
): Session[] {
  let xs = sessions.slice()

  if (filters.adapters.length) {
    xs = xs.filter((s) => filters.adapters.includes(s.adapter))
  }
  if (filters.projects.length) {
    xs = xs.filter((s) => filters.projects.includes(s.project))
  }
  if (filters.summary === 'done') xs = xs.filter((s) => s.l2Done)
  else if (filters.summary === 'pending') xs = xs.filter((s) => !s.l2Done)

  const lowerBound = computeTimeLowerBound(filters.time, now)
  if (lowerBound) {
    const lb = lowerBound.getTime()
    xs = xs.filter((s) => {
      const t = Date.parse(s.startedAt)
      return Number.isFinite(t) && t >= lb
    })
  }

  const q = filters.query.trim().toLowerCase()
  if (q) {
    xs = xs.filter((s) =>
      `${s.title} ${s.project} ${s.topics.join(' ')} ${s.intent ?? ''}`
        .toLowerCase()
        .includes(q),
    )
  }

  if (filters.sort === 'duration') {
    xs.sort((a, b) => b.durationMin - a.durationMin)
  } else if (filters.sort === 'messages') {
    xs.sort((a, b) => b.messages - a.messages)
  } else {
    xs.sort((a, b) => Date.parse(b.startedAt) - Date.parse(a.startedAt))
  }

  return xs
}

/**
 * 按真实"今天"分组。纯函数，不修改输入。
 *
 * 分桶规则（以 `now` 当天 00:00 = `today0` 为基准）：
 *  - today    : startedAt >= today0
 *  - yesterday: today0 - 1d <= startedAt < today0
 *  - week     : today0 - 6d <= startedAt < today0 - 1d  (含今天的最近 7 天 - 今天 - 昨天)
 *  - earlier  : startedAt < today0 - 6d
 *
 * **关键修正**：基准 `today0` 来自 `now`（真实当前时间），**不是**输入数据中
 * 最新一条 session 的 startedAt。这样即使数据库最新一条是几个月前的，
 * 也不会被错误地归到"今天"桶里。
 */
export function groupSessionsByDate(
  sessions: readonly Session[],
  now: Date = new Date(),
): SessionGroup[] {
  if (!sessions.length) return []

  const today0 = startOfDay(now)
  const yest0 = new Date(today0.getTime() - MS_PER_DAY)
  const week0 = new Date(today0.getTime() - 6 * MS_PER_DAY)

  const buckets: Record<SessionGroup['key'], Session[]> = {
    today: [],
    yesterday: [],
    week: [],
    earlier: [],
  }

  for (const s of sessions) {
    const t = Date.parse(s.startedAt)
    if (!Number.isFinite(t)) {
      buckets.earlier.push(s)
      continue
    }
    const d = new Date(t)
    if (d >= today0) buckets.today.push(s)
    else if (d >= yest0) buckets.yesterday.push(s)
    else if (d >= week0) buckets.week.push(s)
    else buckets.earlier.push(s)
  }

  const groups: SessionGroup[] = []
  if (buckets.today.length) groups.push({ key: 'today', label: '今天', sessions: buckets.today })
  if (buckets.yesterday.length)
    groups.push({ key: 'yesterday', label: '昨天', sessions: buckets.yesterday })
  if (buckets.week.length) groups.push({ key: 'week', label: '本周', sessions: buckets.week })
  if (buckets.earlier.length)
    groups.push({ key: 'earlier', label: '更早', sessions: buckets.earlier })
  return groups
}
