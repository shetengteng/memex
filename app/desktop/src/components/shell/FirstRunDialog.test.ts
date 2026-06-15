import { describe, expect, it, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import FirstRunDialog from './FirstRunDialog.vue'

const ipcMocks = vi.hoisted(() => ({
  getConfig: vi.fn(),
  setConfig: vi.fn(),
  ideListStatus: vi.fn(),
  skillListStatus: vi.fn(),
  hookListStatus: vi.fn(),
  ideUninstall: vi.fn(),
  skillUninstall: vi.fn(),
  hookUninstall: vi.fn(),
}))

vi.mock('@/composables/useMemex', () => ({ useMemex: () => ipcMocks }))
vi.mock('vue-sonner', () => ({ toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() } }))
vi.mock('@/i18n', () => ({
  useI18n: () => ({
    t: (key: string, params?: Record<string, unknown>) => {
      const dict: Record<string, string> = {
        'connect.ide.first_run.title': '检测到 IDE 残留集成',
        'connect.ide.first_run.body': '残留 {count} 项',
        'connect.ide.first_run.action.keep': '保留现有集成',
        'connect.ide.first_run.action.clean': '全部清空',
        'connect.ide.action.resetting': '清理中…',
        'connect.ide.toast.reset.success': '已清理 {count} 项',
        'connect.ide.toast.reset.partial': '部分失败：{err}',
      }
      return (dict[key] ?? key).replace(/\{(\w+)\}/g, (_, k) => String(params?.[k] ?? `{${k}}`))
    },
  }),
}))

const stubs = {
  Dialog: { template: '<div v-if="open"><slot/></div>', props: ['open'] },
  DialogContent: { template: '<div><slot/></div>' },
  DialogTitle: { template: '<h3><slot/></h3>' },
  DialogDescription: { template: '<p><slot/></p>' },
  Button: {
    template: '<button :disabled="disabled" @click="$emit(\'click\')"><slot/></button>',
    props: ['disabled'],
    emits: ['click'],
  },
}

describe('FirstRunDialog · 首次启动检测', () => {
  beforeEach(() => {
    Object.values(ipcMocks).forEach((fn) => fn.mockReset())
  })

  /// 已经标记 handled=true → 不调三个 list status，根本不弹。
  it('first_run_handled=true 时跳过检测，不弹窗', async () => {
    ipcMocks.getConfig.mockResolvedValueOnce('true')
    const wrapper = mount(FirstRunDialog, { global: { stubs } })
    await flushPromises()
    expect(ipcMocks.ideListStatus).not.toHaveBeenCalled()
    expect(wrapper.text()).not.toContain('检测到 IDE 残留集成')
  })

  /// 没残留 → 不弹，但要悄悄打 ack 标记，下次跳过。
  it('无残留时悄悄打标记 first_run_handled=true，不弹窗', async () => {
    ipcMocks.getConfig.mockResolvedValueOnce(null)
    ipcMocks.ideListStatus.mockResolvedValueOnce([
      { ide: 'cursor', config_path: '', config_exists: true, installed: false, command: null },
    ])
    ipcMocks.skillListStatus.mockResolvedValueOnce([
      { ide: 'cursor', dest_path: '', installed: false, size: null },
    ])
    ipcMocks.hookListStatus.mockResolvedValueOnce([
      { ide: 'cursor', supported: false, installed: false, config_path: '', wrapper_path: null },
    ])

    const wrapper = mount(FirstRunDialog, { global: { stubs } })
    await flushPromises()
    expect(wrapper.text()).not.toContain('检测到 IDE 残留集成')
    expect(ipcMocks.setConfig).toHaveBeenCalledWith('app.first_run_handled', 'true')
  })

  /// 有残留 → 弹窗 + 显示 count。
  it('检测到残留时弹窗，显示残留数量', async () => {
    ipcMocks.getConfig.mockResolvedValueOnce(null)
    ipcMocks.ideListStatus.mockResolvedValueOnce([
      { ide: 'cursor', config_path: '', config_exists: true, installed: true, command: 'memex' },
    ])
    ipcMocks.skillListStatus.mockResolvedValueOnce([
      { ide: 'cursor', dest_path: '', installed: true, size: 4096 },
    ])
    ipcMocks.hookListStatus.mockResolvedValueOnce([
      { ide: 'cursor', supported: true, installed: true, config_path: '', wrapper_path: '/x' },
    ])

    const wrapper = mount(FirstRunDialog, { global: { stubs } })
    await flushPromises()
    // cursor 三项都 installed → leftoverCount=3
    expect(wrapper.text()).toContain('残留 3 项')
  })

  /// "保留" → 不调 uninstall，写 first_run_handled=true。
  it('点击保留按钮：不卸载，仅写 ack 标记', async () => {
    ipcMocks.getConfig.mockResolvedValueOnce(null)
    ipcMocks.ideListStatus.mockResolvedValueOnce([
      { ide: 'cursor', config_path: '', config_exists: true, installed: true, command: 'x' },
    ])
    ipcMocks.skillListStatus.mockResolvedValueOnce([])
    ipcMocks.hookListStatus.mockResolvedValueOnce([])
    ipcMocks.setConfig.mockResolvedValue(undefined)

    const wrapper = mount(FirstRunDialog, { global: { stubs } })
    await flushPromises()
    const buttons = wrapper.findAll('button')
    const keepBtn = buttons.find((b) => b.text().includes('保留'))
    await keepBtn!.trigger('click')
    await flushPromises()

    expect(ipcMocks.ideUninstall).not.toHaveBeenCalled()
    expect(ipcMocks.setConfig).toHaveBeenCalledWith('app.first_run_handled', 'true')
  })

  /// "全部清空" → 逐项 uninstall + 写 ack 标记。
  it('点击清空按钮：批量卸载已 installed 的项', async () => {
    ipcMocks.getConfig.mockResolvedValueOnce(null)
    ipcMocks.ideListStatus.mockResolvedValueOnce([
      { ide: 'cursor', config_path: '', config_exists: true, installed: true, command: 'x' },
      { ide: 'codex', config_path: '', config_exists: true, installed: false, command: null },
    ])
    ipcMocks.skillListStatus.mockResolvedValueOnce([
      { ide: 'cursor', dest_path: '', installed: true, size: 4096 },
      { ide: 'codex', dest_path: '', installed: false, size: null },
    ])
    ipcMocks.hookListStatus.mockResolvedValueOnce([
      { ide: 'cursor', supported: true, installed: false, config_path: '', wrapper_path: null },
    ])
    ipcMocks.ideUninstall.mockResolvedValue({ ide: 'cursor', installed: false })
    ipcMocks.skillUninstall.mockResolvedValue({ ide: 'cursor', installed: false })
    ipcMocks.setConfig.mockResolvedValue(undefined)

    const wrapper = mount(FirstRunDialog, { global: { stubs } })
    await flushPromises()
    const buttons = wrapper.findAll('button')
    const cleanBtn = buttons.find((b) => b.text().includes('全部清空'))
    await cleanBtn!.trigger('click')
    await flushPromises()

    expect(ipcMocks.ideUninstall).toHaveBeenCalledWith('cursor')
    expect(ipcMocks.skillUninstall).toHaveBeenCalledWith('cursor')
    expect(ipcMocks.ideUninstall).not.toHaveBeenCalledWith('codex')
    expect(ipcMocks.hookUninstall).not.toHaveBeenCalled() // 没 installed 的不调
    expect(ipcMocks.setConfig).toHaveBeenCalledWith('app.first_run_handled', 'true')
  })
})
