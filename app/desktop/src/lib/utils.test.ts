import { describe, expect, it } from 'vitest'
import {
  adapterAbbr,
  adapterLabel,
  cn,
  formatNumber,
  humanizeBackendError,
  isNoisePrompt,
  isPlaceholderTitle,
  meaningfulPrompt,
  meaningfulTitle,
  parseBackendError,
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

describe('noise prompt detection', () => {
  it('recognises claude_code agent role templates as noise', () => {
    expect(isNoisePrompt('=== Role ===\n你是 xxx agent')).toBe(true)
    expect(isNoisePrompt('  === Task ===  body')).toBe(true)
    expect(isNoisePrompt('=== Skills (advisory) ===')).toBe(true)
    expect(isNoisePrompt('=== System ===\n...')).toBe(true)
  })

  it('accepts real user prompts', () => {
    expect(isNoisePrompt('修一下登录')).toBe(false)
    expect(isNoisePrompt('帮我设计 schema')).toBe(false)
  })

  it('treats empty / whitespace-only as noise', () => {
    expect(isNoisePrompt(null)).toBe(true)
    expect(isNoisePrompt(undefined)).toBe(true)
    expect(isNoisePrompt('')).toBe(true)
    expect(isNoisePrompt('   ')).toBe(true)
  })

  it('meaningfulPrompt returns trimmed value or null', () => {
    expect(meaningfulPrompt('  hi there ')).toBe('hi there')
    expect(meaningfulPrompt('=== Role ===\nfoo')).toBeNull()
    expect(meaningfulPrompt(null)).toBeNull()
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

  it('结构化 not_found 错误转中文 "未找到：xxx"', () => {
    const r = humanizeBackendError({ kind: 'not_found', message: 'session abc' })
    expect(r.friendly).toBe('未找到：session abc')
    expect(r.action).toBeUndefined()
  })

  it('结构化 not_found 空 message 兜底', () => {
    const r = humanizeBackendError({ kind: 'not_found', message: '' })
    expect(r.friendly).toBe('未找到所需的资源。')
  })

  it('结构化 validation 错误转中文', () => {
    const r = humanizeBackendError({ kind: 'validation', message: 'limit must be > 0' })
    expect(r.friendly).toBe('输入有误：limit must be > 0')
  })

  it('结构化 backend 错误的 message 走正则匹配（保留旧文案）', () => {
    const r = humanizeBackendError({
      kind: 'backend',
      message: 'No LLM provider available. Enable Ollama or configure a custom LLM provider.',
    })
    expect(r.friendly).toContain('LLM 服务')
    expect(r.action?.route).toBe('/settings')
  })

  it('结构化 io 错误未命中正则时返回原 message 兜底', () => {
    const r = humanizeBackendError({ kind: 'io', message: 'permission denied' })
    expect(r.friendly).toBe('permission denied')
    expect(r.action).toBeUndefined()
  })

  it('未知 kind 字段降级为 backend', () => {
    const r = humanizeBackendError({ kind: 'mystery', message: 'something' })
    expect(r.friendly).toBe('something')
  })
})

describe('parseBackendError', () => {
  it('结构化对象 → 原样返回', () => {
    const r = parseBackendError({ kind: 'io', message: 'bad disk' })
    expect(r).toEqual({ kind: 'io', message: 'bad disk' })
  })

  it('字符串错误 → kind 默认 backend', () => {
    const r = parseBackendError('something failed')
    expect(r).toEqual({ kind: 'backend', message: 'something failed' })
  })

  it('null / undefined 兜底为空 message + backend kind', () => {
    expect(parseBackendError(null)).toEqual({ kind: 'backend', message: '' })
    expect(parseBackendError(undefined)).toEqual({ kind: 'backend', message: '' })
  })

  it('未知 kind 自动降级为 backend', () => {
    const r = parseBackendError({ kind: 'wat', message: 'm' })
    expect(r.kind).toBe('backend')
  })
})
