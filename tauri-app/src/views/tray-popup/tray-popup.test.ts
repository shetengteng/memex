import { describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { ref } from 'vue'

// Mock Tauri APIs（jsdom 里没有 window 实例）
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    onFocusChanged: vi.fn(),
    hide: vi.fn().mockResolvedValue(undefined),
    show: vi.fn().mockResolvedValue(undefined),
    setFocus: vi.fn().mockResolvedValue(undefined),
  }),
}))

vi.mock('@tauri-apps/api/event', () => ({
  emit: vi.fn().mockResolvedValue(undefined),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}))

// Mock stores/memex 让测试可控（用普通对象/数组，匹配真实 store 的 reactive 形状）
vi.mock('@/stores/memex', () => {
  return {
    sessions: [
      { id: 's1', adapter: 'cursor', title: 'Session 1', startedAt: new Date().toISOString() },
      { id: 's2', adapter: 'claude_code', title: 'Session 2', startedAt: new Date().toISOString() },
    ],
    totals: { sessions: 42, messages: 1000, projects: 0 },
    daemonStatus: {
      running: true,
      startedAt: '',
      adapterActive: 1,
      adapterTotal: 7,
      llmProvider: 'ollama',
      llmModel: 'qwen2.5',
      llmHealth: 'ok',
      storage: '',
      ftsHealth: 'ok',
      lastIngest: '',
    },
    ADAPTER_MAP: {
      cursor: { id: 'cursor', label: 'Cursor', abbr: 'Cu' },
      claude_code: { id: 'claude_code', label: 'Claude Code', abbr: 'CC' },
    },
    refreshSessions: vi.fn().mockResolvedValue(undefined),
    daemon: ref({ running: true, started_at: '', pid: 12345 }),
  }
})

vi.mock('@/composables/useDaemon', () => ({
  useDaemon: () => ({
    restart: vi.fn().mockResolvedValue({ running: true, started_at: '', pid: 99999 }),
    loading: ref(false),
  }),
}))

vi.mock('@/composables/useMemex', () => ({
  useMemex: () => ({
    getStats: vi.fn().mockResolvedValue({
      sessions: 42,
      messages: 1000,
      chunks: 0,
      db_exists: true,
      summaries: 0,
      sessions_eligible_for_summary: 0,
      chunks_summarized: 0,
      llm_provider: 'ollama',
      llm_model: 'qwen2.5:7b',
    }),
  }),
}))

import TrayPopup from './index.vue'
import { invoke } from '@tauri-apps/api/core'

describe('TrayPopup view', () => {
  const baseStubs = {
    Button: {
      template: '<button :disabled="disabled" @click="$emit(\'click\', $event)"><slot /></button>',
      props: ['disabled', 'variant', 'size'],
      emits: ['click'],
    },
  }

  it('mounts without throwing', () => {
    const wrapper = mount(TrayPopup, {
      global: { stubs: baseStubs },
    })
    expect(wrapper.exists()).toBe(true)
  })

  it('renders recent sessions from mock', () => {
    const wrapper = mount(TrayPopup, { global: { stubs: baseStubs } })
    expect(wrapper.text()).toContain('Session 1')
    expect(wrapper.text()).toContain('Session 2')
  })

  it('shows total sessions count in header', () => {
    const wrapper = mount(TrayPopup, { global: { stubs: baseStubs } })
    expect(wrapper.text()).toContain('42')
  })

  it('shows LLM model name from daemonStatus', () => {
    const wrapper = mount(TrayPopup, { global: { stubs: baseStubs } })
    expect(wrapper.text()).toContain('qwen2.5')
  })

  it('点 session 条目会 invoke show_main_window 并把 navigate 路径作为参数传给后端（修复 popup 无效点击 bug）', async () => {
    vi.mocked(invoke).mockClear()

    const wrapper = mount(TrayPopup, { global: { stubs: baseStubs } })
    const sessionButtons = wrapper.findAll('button').filter((b) => b.text().includes('Session'))
    expect(sessionButtons.length).toBeGreaterThan(0)
    await sessionButtons[0].trigger('click')

    // 现在后端 show_main_window 一并负责 emit('navigate', path)，前端只需 invoke 一次
    expect(vi.mocked(invoke)).toHaveBeenCalledWith(
      'show_main_window',
      expect.objectContaining({ navigate: expect.stringContaining('/library?session=') }),
    )
  })

  it('header 下方渲染 daemon 状态卡片 + 重启按钮（之前老 popup 的 hero 卡片回归）', () => {
    const wrapper = mount(TrayPopup, { global: { stubs: baseStubs } })
    const text = wrapper.text()
    // 状态卡片的固定文案
    expect(text).toContain('后台服务')
    // 状态文案：mock daemon.running=true 时显示"运行中 (pid xxx)"
    expect(text).toContain('运行中')
    // 重启按钮存在
    const restartBtn = wrapper.findAll('button').find((b) => b.text().includes('重启'))
    expect(restartBtn).toBeTruthy()
    expect(restartBtn!.exists()).toBe(true)
  })
})
