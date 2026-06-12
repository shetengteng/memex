/**
 * `sessionFilters` 纯函数单测。
 *
 * 重点覆盖两个回归场景：
 *  1. fTime 过滤实际生效（之前 library/index.vue 漏接，UI 上选时间窗口对结果无影响）
 *  2. 分组的"今天"以真实当前时间为基准，而非数据中最新一条 session 的当天
 *     （之前会把数据库最新一条 — 即使是几个月前 — 错误归到"今天"桶里）
 */
import { describe, expect, it } from 'vitest'
import {
  computeTimeLowerBound,
  filterAndSortSessions,
  groupSessionsByDate,
  type SessionFilters,
} from './sessionFilters'
import type { Session } from '@/stores/memex'

const NOW = new Date('2026-06-09T15:30:00+08:00') // Asia/Shanghai

function mkSession(over: Partial<Session> & { startedAt: string }): Session {
  return {
    id: `s-${over.startedAt}`,
    adapter: 'cursor',
    workspace: 'memex',
    project: 'memex',
    durationMin: 10,
    messages: 5,
    title: 'untitled',
    topics: [],
    l2Done: false,
    intent: undefined,
    ...over,
  }
}

const baseFilters: SessionFilters = {
  adapters: [],
  projects: [],
  summary: 'all',
  query: '',
  time: 'all',
  sort: 'recent',
}

describe('computeTimeLowerBound', () => {
  it("'all' returns null (no filter)", () => {
    expect(computeTimeLowerBound('all', NOW)).toBeNull()
  })

  it("'today' returns today 00:00:00 in local time", () => {
    const lb = computeTimeLowerBound('today', NOW)!
    expect(lb).not.toBeNull()
    expect(lb.getHours()).toBe(0)
    expect(lb.getMinutes()).toBe(0)
    expect(lb.getDate()).toBe(NOW.getDate())
  })

  it("'7d' returns 6 days before today 00:00 (today inclusive = 7 calendar days)", () => {
    const lb = computeTimeLowerBound('7d', NOW)!
    const today0 = new Date(NOW)
    today0.setHours(0, 0, 0, 0)
    const expected = new Date(today0.getTime() - 6 * 24 * 60 * 60 * 1000)
    expect(lb.getTime()).toBe(expected.getTime())
  })

  it("'30d' / '90d' return N-1 days before today 00:00", () => {
    const day = 24 * 60 * 60 * 1000
    const today0 = new Date(NOW)
    today0.setHours(0, 0, 0, 0)
    expect(computeTimeLowerBound('30d', NOW)!.getTime()).toBe(today0.getTime() - 29 * day)
    expect(computeTimeLowerBound('90d', NOW)!.getTime()).toBe(today0.getTime() - 89 * day)
  })
})

describe('filterAndSortSessions — fTime regression', () => {
  it("BUG #1 fix: 'today' filter excludes any session not from today (real today, not data-relative)", () => {
    // 真实今天是 2026-06-09。数据库里最新一条是 2025-05-26（几个月前）。
    // 旧实现：fTime 完全没生效，过滤后这条还在；UI 显示"今天 1"。
    // 新实现：'today' 必须严格按真实今天过滤掉。
    const xs = [
      mkSession({ startedAt: '2025-05-26T20:06:00+08:00', title: 'say hello in one word' }),
      mkSession({ startedAt: '2026-06-09T09:00:00+08:00', title: 'real today' }),
    ]
    const out = filterAndSortSessions(xs, { ...baseFilters, time: 'today' }, NOW)
    expect(out).toHaveLength(1)
    expect(out[0].title).toBe('real today')
  })

  it("'7d' filter keeps sessions in the last 7 calendar days inclusive", () => {
    const xs = [
      mkSession({ startedAt: '2026-06-09T10:00:00+08:00', title: 'today' }), // today
      mkSession({ startedAt: '2026-06-03T10:00:00+08:00', title: 'd-6 boundary' }), // boundary IN
      mkSession({ startedAt: '2026-06-02T23:59:00+08:00', title: 'd-7 just out' }), // OUT
      mkSession({ startedAt: '2025-05-26T20:06:00+08:00', title: 'months ago' }), // OUT
    ]
    const out = filterAndSortSessions(xs, { ...baseFilters, time: '7d' }, NOW)
    const titles = out.map((s) => s.title)
    expect(titles).toContain('today')
    expect(titles).toContain('d-6 boundary')
    expect(titles).not.toContain('d-7 just out')
    expect(titles).not.toContain('months ago')
  })

  it("'all' time filter keeps everything", () => {
    const xs = [
      mkSession({ startedAt: '2026-06-09T10:00:00+08:00' }),
      mkSession({ startedAt: '2025-05-26T20:06:00+08:00' }),
      mkSession({ startedAt: '2024-01-01T00:00:00+08:00' }),
    ]
    const out = filterAndSortSessions(xs, { ...baseFilters, time: 'all' }, NOW)
    expect(out).toHaveLength(3)
  })

  it('combines time filter with adapter / project / summary / query filters', () => {
    const xs = [
      mkSession({
        startedAt: '2026-06-09T10:00:00+08:00',
        adapter: 'cursor',
        project: 'memex',
        title: 'fix bug',
        l2Done: true,
      }),
      mkSession({
        startedAt: '2026-06-09T11:00:00+08:00',
        adapter: 'claude_code',
        project: 'memex',
        title: 'fix bug',
        l2Done: true,
      }),
      mkSession({
        startedAt: '2026-06-09T12:00:00+08:00',
        adapter: 'cursor',
        project: 'other',
        title: 'fix bug',
        l2Done: false,
      }),
    ]
    const out = filterAndSortSessions(
      xs,
      {
        ...baseFilters,
        adapters: ['cursor'],
        projects: ['memex'],
        summary: 'done',
        query: 'bug',
        time: 'today',
      },
      NOW,
    )
    expect(out).toHaveLength(1)
    expect(out[0].adapter).toBe('cursor')
    expect(out[0].project).toBe('memex')
  })

  it("default sort 'recent' orders by startedAt DESC", () => {
    const xs = [
      mkSession({ startedAt: '2026-06-09T08:00:00+08:00', title: 'A' }),
      mkSession({ startedAt: '2026-06-09T12:00:00+08:00', title: 'B' }),
      mkSession({ startedAt: '2026-06-09T10:00:00+08:00', title: 'C' }),
    ]
    const out = filterAndSortSessions(xs, { ...baseFilters, time: 'today' }, NOW)
    expect(out.map((s) => s.title)).toEqual(['B', 'C', 'A'])
  })

  it('does not mutate the input array', () => {
    const xs = [
      mkSession({ startedAt: '2026-06-09T10:00:00+08:00', title: 'a' }),
      mkSession({ startedAt: '2026-06-09T11:00:00+08:00', title: 'b' }),
    ]
    const before = xs.map((s) => s.title).join(',')
    filterAndSortSessions(xs, { ...baseFilters, sort: 'duration' }, NOW)
    expect(xs.map((s) => s.title).join(',')).toBe(before)
  })
})

describe('groupSessionsByDate — today boundary regression', () => {
  it('BUG #2 fix: months-old session never falls into "today" bucket', () => {
    // 真实今天 2026-06-09。输入只有一条 2025-05-26 的 session。
    // 旧实现：ref0 = max(startedAt) = 5/26 0:00，那条 d >= ref0 → 进 today 桶 → "今天 1"。
    // 新实现：ref0 = NOW 0:00 = 6/9 0:00，那条进 earlier 桶。
    const xs = [
      mkSession({ startedAt: '2025-05-26T20:06:00+08:00', title: 'say hello in one word' }),
    ]
    const groups = groupSessionsByDate(xs, NOW)
    expect(groups).toHaveLength(1)
    expect(groups[0].key).toBe('earlier')
    expect(groups[0].sessions).toHaveLength(1)
  })

  it('correctly buckets today / yesterday / week / earlier', () => {
    const xs = [
      mkSession({ startedAt: '2026-06-09T10:00:00+08:00', title: 'today' }),
      mkSession({ startedAt: '2026-06-08T20:00:00+08:00', title: 'yesterday' }),
      mkSession({ startedAt: '2026-06-05T12:00:00+08:00', title: 'week-3d' }),
      mkSession({ startedAt: '2026-06-03T01:00:00+08:00', title: 'week-edge' }), // today - 6d
      mkSession({ startedAt: '2026-06-02T23:00:00+08:00', title: 'earlier-just' }), // today - 7d
      mkSession({ startedAt: '2025-05-26T20:06:00+08:00', title: 'much-earlier' }),
    ]
    const groups = groupSessionsByDate(xs, NOW)
    const byKey = Object.fromEntries(groups.map((g) => [g.key, g.sessions.map((s) => s.title)]))

    expect(byKey.today).toEqual(['today'])
    expect(byKey.yesterday).toEqual(['yesterday'])
    expect(byKey.week).toEqual(expect.arrayContaining(['week-3d', 'week-edge']))
    expect(byKey.week.length).toBe(2)
    expect(byKey.earlier).toEqual(expect.arrayContaining(['earlier-just', 'much-earlier']))
    expect(byKey.earlier.length).toBe(2)
  })

  it('returns groups in the canonical order today → yesterday → week → earlier', () => {
    const xs = [
      mkSession({ startedAt: '2025-05-26T20:00:00+08:00', title: 'much-earlier' }),
      mkSession({ startedAt: '2026-06-09T10:00:00+08:00', title: 'today' }),
      mkSession({ startedAt: '2026-06-05T10:00:00+08:00', title: 'week' }),
      mkSession({ startedAt: '2026-06-08T10:00:00+08:00', title: 'yesterday' }),
    ]
    const groups = groupSessionsByDate(xs, NOW)
    expect(groups.map((g) => g.key)).toEqual(['today', 'yesterday', 'week', 'earlier'])
  })

  it('skips empty buckets', () => {
    const xs = [mkSession({ startedAt: '2025-05-26T20:00:00+08:00', title: 'old' })]
    const groups = groupSessionsByDate(xs, NOW)
    expect(groups.map((g) => g.key)).toEqual(['earlier'])
  })

  it('returns empty array on empty input', () => {
    expect(groupSessionsByDate([], NOW)).toEqual([])
  })

  it('puts unparseable startedAt into "earlier" bucket as fallback', () => {
    const xs = [mkSession({ startedAt: 'not-a-date', title: 'broken' })]
    const groups = groupSessionsByDate(xs, NOW)
    expect(groups[0].key).toBe('earlier')
    expect(groups[0].sessions[0].title).toBe('broken')
  })
})
