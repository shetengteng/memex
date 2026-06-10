import { describe, expect, it } from 'vitest'
import { buildAdapterUsage } from './adapterUsage'

const id = (s: string) => s

describe('buildAdapterUsage', () => {
  it('returns empty array for no input', () => {
    expect(buildAdapterUsage([], id)).toEqual([])
  })

  it('filters out zero-session adapters', () => {
    const rows = buildAdapterUsage(
      [
        { key: 'cursor', sessions: 100 },
        { key: 'ghost', sessions: 0 },
      ],
      id,
    )
    expect(rows.map((r) => r.id)).toEqual(['cursor'])
  })

  it('marks the single adapter as 100% on both width and share', () => {
    const [row] = buildAdapterUsage([{ key: 'cursor', sessions: 42 }], id)
    expect(row.widthPct).toBe(100)
    expect(row.sharePct).toBe(100)
  })

  /**
   * 这是 bug 修复的回归用例 —— 用户报告里的真实数据：
   *   Cursor 795, Claude 248, OpenCode 116, Codex 13
   * 旧逻辑把 widthPct 当数字渲染，于是 Cursor 显示 "100%"，让用户以为
   * "Cursor 占了全部 100%"。新逻辑应当区分两套百分比：
   *   widthPct: 100 / 31 / 15 / 2  （相对最大值，给 bar 视觉对比用）
   *   sharePct: 68 / 21 / 10 / 1   （相对总和，给文本"占比"用）
   */
  it('separates widthPct (vs max) and sharePct (vs total) for the reported data', () => {
    const rows = buildAdapterUsage(
      [
        { key: 'cursor', sessions: 795 },
        { key: 'claude_code', sessions: 248 },
        { key: 'opencode', sessions: 116 },
        { key: 'codex', sessions: 13 },
      ],
      id,
    )
    expect(rows.map((r) => [r.id, r.widthPct, r.sharePct])).toEqual([
      ['cursor', 100, 68],
      ['claude_code', 31, 21],
      ['opencode', 15, 10],
      ['codex', 2, 1],
    ])
    // sharePct 总和应当近似 100（rounding 误差 ±2）
    const sharePctSum = rows.reduce((acc, r) => acc + r.sharePct, 0)
    expect(sharePctSum).toBeGreaterThanOrEqual(98)
    expect(sharePctSum).toBeLessThanOrEqual(102)
  })

  it('sorts by count desc and truncates to topN', () => {
    const rows = buildAdapterUsage(
      [
        { key: 'a', sessions: 10 },
        { key: 'b', sessions: 50 },
        { key: 'c', sessions: 30 },
      ],
      id,
      2,
    )
    expect(rows.map((r) => r.id)).toEqual(['b', 'c'])
  })

  it('uses labelOf to map ids to display labels', () => {
    const labels: Record<string, string> = { cursor: 'Cursor', xcode: 'Xcode' }
    const rows = buildAdapterUsage(
      [{ key: 'cursor', sessions: 5 }],
      (k) => labels[k] ?? k,
    )
    expect(rows[0].label).toBe('Cursor')
  })
})
