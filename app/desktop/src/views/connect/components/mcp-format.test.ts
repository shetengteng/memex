import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  asMetric,
  formatFullTime,
  formatLatency,
  formatRelative,
  prettifyPayload,
} from './mcp-format'

describe('formatFullTime', () => {
  it('renders RFC3339 with zero-padded YMD and HMS', () => {
    expect(formatFullTime('2026-06-11T03:04:05Z')).toMatch(
      /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$/,
    )
  })

  it('uses two-digit padding for single-digit fields', () => {
    const out = formatFullTime('2026-01-02T03:04:05Z')
    expect(out).toMatch(/-01-02 /)
  })

  it('returns the raw string when input is unparseable', () => {
    expect(formatFullTime('not a date')).toBe('not a date')
  })
})

describe('formatRelative', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-06-11T12:00:00Z'))
  })
  afterEach(() => {
    vi.useRealTimers()
  })

  it('shows "刚刚" within 5 seconds', () => {
    expect(formatRelative(new Date('2026-06-11T11:59:58Z'))).toBe('刚刚')
  })

  it('shows seconds within a minute', () => {
    expect(formatRelative(new Date('2026-06-11T11:59:40Z'))).toBe('20 秒前')
  })

  it('shows minutes within an hour', () => {
    expect(formatRelative(new Date('2026-06-11T11:30:00Z'))).toBe('30 分钟前')
  })

  it('shows hours within a day', () => {
    expect(formatRelative(new Date('2026-06-11T06:00:00Z'))).toBe('6 小时前')
  })

  it('shows days beyond 24 hours', () => {
    expect(formatRelative(new Date('2026-06-09T12:00:00Z'))).toBe('2 天前')
  })
})

describe('formatLatency', () => {
  it('keeps sub-10 ms integers without decimal point', () => {
    expect(formatLatency(3)).toBe('3 ms')
    expect(formatLatency(9)).toBe('9 ms')
  })

  it('rounds sub-10 ms floats to at most one decimal', () => {
    // 这是核心 bug：来自 sum/count 的 8.666666666666666 不能原样打印
    expect(formatLatency(8.666666666666666)).toBe('8.7 ms')
    expect(formatLatency(0.5)).toBe('0.5 ms')
    expect(formatLatency(0.45)).toBe('0.5 ms')
    expect(formatLatency(0.04)).toBe('0 ms')
  })

  it('rounds sub-second to integer ms', () => {
    expect(formatLatency(123)).toBe('123 ms')
    expect(formatLatency(999)).toBe('999 ms')
  })

  it('converts seconds with two decimals', () => {
    expect(formatLatency(1_000)).toBe('1.00 s')
    expect(formatLatency(1_234)).toBe('1.23 s')
    expect(formatLatency(45_678)).toBe('45.68 s')
  })
})

describe('asMetric', () => {
  it('renders em-dash for zero', () => {
    expect(asMetric(0)).toBe('—')
  })

  it('renders thousands separators for non-zero', () => {
    expect(asMetric(1)).toBe('1')
    expect(asMetric(1_234)).toBe('1,234')
    expect(asMetric(1_234_567)).toBe('1,234,567')
  })
})

describe('prettifyPayload', () => {
  it('treats null / empty as empty payload', () => {
    expect(prettifyPayload(null)).toEqual({
      display: '(无内容)',
      isJson: false,
      truncated: false,
      empty: true,
    })
    expect(prettifyPayload('').empty).toBe(true)
    expect(prettifyPayload(undefined).empty).toBe(true)
  })

  it('pretty-prints valid JSON with 2-space indent', () => {
    const out = prettifyPayload('{"query":"memex","limit":5}')
    expect(out.isJson).toBe(true)
    expect(out.truncated).toBe(false)
    expect(out.display).toBe('{\n  "query": "memex",\n  "limit": 5\n}')
  })

  it('unwraps {"text": "..."} envelope to inner string', () => {
    const out = prettifyPayload('{"text":"hello world"}')
    expect(out.isJson).toBe(true)
    expect(out.display).toBe('hello world')
  })

  it('unwraps {"text": "<nested json>"} by parsing inner JSON', () => {
    const inner = JSON.stringify([{ id: 'abc', title: 'hi' }])
    const wrapped = JSON.stringify({ text: inner })
    const out = prettifyPayload(wrapped)
    expect(out.isJson).toBe(true)
    expect(out.display).toContain('"id": "abc"')
    expect(out.display).toContain('"title": "hi"')
  })

  it('unwraps {"error": "..."} envelope identically', () => {
    const out = prettifyPayload('{"error":"session not found"}')
    expect(out.display).toBe('session not found')
  })

  it('does not unwrap non-single-key objects', () => {
    const out = prettifyPayload('{"text":"hi","extra":1}')
    expect(out.display).toContain('"text"')
    expect(out.display).toContain('"extra"')
  })

  it('flags truncated marker and strips it from display body', () => {
    const truncated = '{"text":"partial"\n…[truncated 4096 bytes]'
    const out = prettifyPayload(truncated)
    expect(out.truncated).toBe(true)
    // Marker should not appear in the rendered body
    expect(out.display).not.toMatch(/\[truncated/)
  })

  it('falls back to plain text when JSON.parse fails', () => {
    const out = prettifyPayload('not json {{{')
    expect(out.isJson).toBe(false)
    expect(out.truncated).toBe(false)
    expect(out.display).toBe('not json {{{')
  })

  it('truncated + invalid-after-cut still surfaces a body', () => {
    const broken = '{"text":"hello world", "limit"\n…[truncated 999 bytes]'
    const out = prettifyPayload(broken)
    expect(out.truncated).toBe(true)
    expect(out.isJson).toBe(false)
    // Truncate marker stripped, original prefix retained
    expect(out.display).toContain('hello world')
    expect(out.display).not.toMatch(/\[truncated/)
  })
})
