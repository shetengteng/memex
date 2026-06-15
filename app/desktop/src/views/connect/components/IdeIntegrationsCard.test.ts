import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import IdeIntegrationsCard from './IdeIntegrationsCard.vue'

const ipcMocks = vi.hoisted(() => ({
  ideListStatus: vi.fn(),
  skillListStatus: vi.fn(),
  hookListStatus: vi.fn(),
  ideInstall: vi.fn(),
  ideUninstall: vi.fn(),
  skillInstall: vi.fn(),
  skillUninstall: vi.fn(),
  hookInstall: vi.fn(),
  hookUninstall: vi.fn(),
}))

const eventMocks = vi.hoisted(() => {
  const handlers = new Map<string, (e: { payload: unknown }) => void>()
  return {
    handlers,
    listen: vi.fn(async (name: string, cb: (e: { payload: unknown }) => void) => {
      handlers.set(name, cb)
      return () => handlers.delete(name)
    }),
    emit(name: string, payload: unknown = null) {
      handlers.get(name)?.({ payload })
    },
  }
})

const toastMocks = vi.hoisted(() => ({
  success: vi.fn(),
  error: vi.fn(),
  message: vi.fn(),
}))

vi.mock('@/composables/useMemex', () => ({
  useMemex: () => ipcMocks,
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: eventMocks.listen,
}))

vi.mock('vue-sonner', () => ({
  toast: toastMocks,
}))

describe('IdeIntegrationsCard', () => {
  beforeEach(() => {
    Object.values(ipcMocks).forEach((fn) => fn.mockReset())
    Object.values(toastMocks).forEach((fn) => fn.mockReset())
    eventMocks.handlers.clear()
    eventMocks.listen.mockClear()
    ipcMocks.ideListStatus.mockResolvedValue([
      {
        ide: 'cursor',
        config_path: '~/.cursor/mcp.json',
        config_exists: true,
        installed: true,
        command: 'memex',
      },
      {
        ide: 'claude_code',
        config_path: '~/.claude.json',
        config_exists: true,
        installed: false,
        command: null,
      },
    ])
    ipcMocks.skillListStatus.mockResolvedValue([
      { ide: 'cursor', dest_path: '~/.cursor/skills', installed: true, size: 4096 },
      { ide: 'claude_code', dest_path: '~/.claude/skills', installed: false, size: null },
    ])
    ipcMocks.hookListStatus.mockResolvedValue([
      {
        ide: 'cursor',
        supported: false,
        installed: false,
        config_path: '',
        wrapper_path: null,
      },
      {
        ide: 'claude_code',
        supported: true,
        installed: true,
        config_path: '~/.claude/hooks.json',
        wrapper_path: '/tmp/wrap',
      },
    ])
  })

  const stubs = {
    IdeDot: true,
    Badge: { template: '<span><slot/></span>' },
    Tooltip: { template: '<span><slot/></span>' },
    TooltipTrigger: { template: '<span><slot/></span>' },
    TooltipContent: { template: '<span><slot/></span>' },
    Card: { template: '<div><slot/></div>' },
    Separator: { template: '<div></div>' },
    Switch: {
      template: '<input type="checkbox" />',
      props: ['modelValue', 'disabled'],
      emits: ['update:modelValue'],
    },
    Button: {
      template: '<button :disabled="disabled" @click="$emit(\'click\')"><slot/></button>',
      props: ['disabled'],
      emits: ['click'],
    },
  }

  it('loads ide / skill / hook status on mount', async () => {
    const wrapper = mount(IdeIntegrationsCard, { global: { stubs } })
    await flushPromises()
    expect(ipcMocks.ideListStatus).toHaveBeenCalledOnce()
    expect(ipcMocks.skillListStatus).toHaveBeenCalledOnce()
    expect(ipcMocks.hookListStatus).toHaveBeenCalledOnce()
    const text = wrapper.text()
    expect(text).toContain('Cursor')
    expect(text).toContain('Claude Code')
    expect(text).toContain('1 / 2 已接入')
  })

  it('shows empty state when nothing detected', async () => {
    ipcMocks.ideListStatus.mockResolvedValueOnce([])
    ipcMocks.skillListStatus.mockResolvedValueOnce([])
    ipcMocks.hookListStatus.mockResolvedValueOnce([])
    const wrapper = mount(IdeIntegrationsCard, { global: { stubs } })
    await flushPromises()
    expect(wrapper.text()).toContain('未检测到可接入的 IDE')
  })

  it('reloads status when backend broadcasts reset-complete', async () => {
    mount(IdeIntegrationsCard, { global: { stubs } })
    await flushPromises()
    expect(ipcMocks.ideListStatus).toHaveBeenCalledTimes(1)

    eventMocks.emit('reset-complete')
    await flushPromises()

    expect(ipcMocks.ideListStatus).toHaveBeenCalledTimes(2)
    expect(ipcMocks.skillListStatus).toHaveBeenCalledTimes(2)
    expect(ipcMocks.hookListStatus).toHaveBeenCalledTimes(2)
  })

  it('surfaces backend error via toast instead of swallowing it', async () => {
    ipcMocks.ideListStatus.mockRejectedValueOnce({
      kind: 'NotFound',
      message: '找不到 memex CLI',
    })
    mount(IdeIntegrationsCard, { global: { stubs } })
    await flushPromises()
    expect(toastMocks.error).toHaveBeenCalledTimes(1)
    expect(toastMocks.error.mock.calls[0]?.[0]).toContain('IDE: 找不到 memex CLI')
  })

  /// 重置全部按钮：用户确认后批量调三种 uninstall。
  /// happy-dom 默认不挂 window.confirm，这里手动挂一个 mock 函数。
  it('resetAll 逐项卸载所有 installed 集成，确认 false 不动', async () => {
    const confirmMock = vi.fn().mockReturnValue(false)
    // happy-dom: window.confirm 不存在；直接挂一个属性供组件读
    Object.defineProperty(window, 'confirm', { value: confirmMock, configurable: true, writable: true })

    const wrapper = mount(IdeIntegrationsCard, { global: { stubs } })
    await flushPromises()

    const allButtons = wrapper.findAll('button')
    const resetBtn = allButtons.find((b) => b.text().includes('重置全部'))
    expect(resetBtn).toBeDefined()
    await resetBtn!.trigger('click')
    await flushPromises()
    expect(ipcMocks.ideUninstall).not.toHaveBeenCalled()
    expect(ipcMocks.skillUninstall).not.toHaveBeenCalled()
    expect(ipcMocks.hookUninstall).not.toHaveBeenCalled()

    confirmMock.mockReturnValue(true)
    ipcMocks.ideUninstall.mockResolvedValue({ ide: 'cursor', installed: false })
    ipcMocks.skillUninstall.mockResolvedValue({ ide: 'cursor', installed: false })
    ipcMocks.hookUninstall.mockResolvedValue({ ide: 'claude_code', installed: false })

    await resetBtn!.trigger('click')
    await flushPromises()

    // mock 数据：cursor 装了 mcp+skill，claude_code 装了 hook，共 3 项
    expect(ipcMocks.ideUninstall).toHaveBeenCalledWith('cursor')
    expect(ipcMocks.skillUninstall).toHaveBeenCalledWith('cursor')
    expect(ipcMocks.hookUninstall).toHaveBeenCalledWith('claude_code')
    // 重置完会再 loadStatus → ideListStatus 应该被调第二次（mount 一次 + reset 后一次）
    expect(ipcMocks.ideListStatus).toHaveBeenCalledTimes(2)
  })
})
