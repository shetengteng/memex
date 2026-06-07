import { describe, expect, it } from 'vitest'
import {
  adapterAbbr,
  adapterLabel,
  cn,
  formatNumber,
  humanizeBackendError,
  isPlaceholderTitle,
  meaningfulTitle,
} from './utils'

describe('cn', () => {
  it('merges class names', () => {
    expect(cn('a', 'b')).toBe('a b')
    expect(cn('p-2', false && 'hidden', 'p-4')).toBe('p-4') // tailwind-merge 抑制冲突
  })

  it('handles falsy values', () => {
    expect(cn('a', null, undefined, '')).toBe('a')
  })
})

describe('formatNumber', () => {
  it('compacts thousands', () => {
    expect(formatNumber(999)).toBe('999')
    expect(formatNumber(1000)).toBe('1K')
    expect(formatNumber(1234)).toBe('1.2K')
    expect(formatNumber(1_000_000)).toBe('1M')
  })
})

describe('adapter helpers', () => {
  it('label / abbr for known adapters', () => {
    expect(adapterLabel('claude_code')).toBe('Claude Code')
    expect(adapterAbbr('claude_code')).toBe('CC')
    expect(adapterLabel('cursor')).toBe('Cursor')
  })
  it('label fallback for unknown adapter', () => {
    expect(adapterLabel('whatever')).toBe('whatever')
    expect(adapterAbbr('whatever')).toBe('WH')
  })
})

describe('placeholder title detection', () => {
  it('treats empty / placeholder titles as placeholder', () => {
    expect(isPlaceholderTitle(null)).toBe(true)
    expect(isPlaceholderTitle('')).toBe(true)
    expect(isPlaceholderTitle('   ')).toBe(true)
    expect(isPlaceholderTitle('New session - 2026-01-23T12:00:00Z')).toBe(true)
    expect(isPlaceholderTitle('New session')).toBe(true)
    expect(isPlaceholderTitle('新对话')).toBe(true)
    expect(isPlaceholderTitle('开始对话')).toBe(true)
    expect(isPlaceholderTitle('Conversation initiation')).toBe(true)
  })

  it('accepts real titles', () => {
    expect(isPlaceholderTitle('Memex menubar shadcn 重构原型')).toBe(false)
    expect(isPlaceholderTitle('How to set up Tauri tray')).toBe(false)
  })

  it('meaningfulTitle trims real titles, drops placeholders', () => {
    expect(meaningfulTitle('  hello  ')).toBe('hello')
    expect(meaningfulTitle('New session')).toBeNull()
    expect(meaningfulTitle(null)).toBeNull()
  })
})

describe('humanizeBackendError', () => {
  it('No LLM provider 报错给中文 + 跳设置 action', () => {
    const r = humanizeBackendError(
      'No LLM provider available. Enable Ollama or configure a custom LLM provider.',
    )
    expect(r.friendly).toContain('LLM 服务')
    expect(r.action).toEqual({ label: '去设置', route: '/settings' })
  })

  it('Ollama connection refused 友好化', () => {
    const r = humanizeBackendError('connection refused: 127.0.0.1:11434')
    expect(r.friendly).toContain('Ollama')
    expect(r.action?.route).toBe('/settings')
  })

  it('401 unauthorized 给 Key 失效提示', () => {
    const r = humanizeBackendError('HTTP 401 Unauthorized')
    expect(r.friendly).toContain('API Key')
    expect(r.action?.route).toBe('/settings')
  })

  it('at least N messages 给消息太少提示，无 action', () => {
    const r = humanizeBackendError('at least 2 messages required')
    expect(r.friendly).toContain('至少需要 2 条消息')
    expect(r.action).toBeUndefined()
  })

  it('未识别错误兜底返回原文', () => {
    const r = humanizeBackendError('weird unexpected error xyz')
    expect(r.friendly).toBe('weird unexpected error xyz')
    expect(r.action).toBeUndefined()
  })

  it('null/undefined/空串兜底成 "未知错误"', () => {
    expect(humanizeBackendError(null).friendly).toBe('未知错误')
    expect(humanizeBackendError(undefined).friendly).toBe('未知错误')
    expect(humanizeBackendError('').friendly).toBe('未知错误')
  })
})
