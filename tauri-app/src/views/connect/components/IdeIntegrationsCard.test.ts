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

vi.mock('@/composables/useMemex', () => ({
  useMemex: () => ipcMocks,
}))

vi.mock('vue-sonner', () => ({
  toast: { success: vi.fn(), error: vi.fn(), message: vi.fn() },
}))

describe('IdeIntegrationsCard', () => {
  beforeEach(() => {
    Object.values(ipcMocks).forEach((fn) => fn.mockReset())
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
})
