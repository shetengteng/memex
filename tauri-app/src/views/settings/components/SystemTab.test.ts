import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import SystemTab from './SystemTab.vue'

const ipcMocks = vi.hoisted(() => ({
  cliStatus: vi.fn(),
  cliInstall: vi.fn(),
  cliUninstall: vi.fn(),
  runDoctor: vi.fn(),
  checkForUpdates: vi.fn(),
}))

vi.mock('@/composables/useMemex', () => ({
  useMemex: () => ipcMocks,
}))

// daemon 卡片不是这个文件的测试重点：mock 成永远 ready，避免 "检查中…" 字样污染断言。
const daemonMock = vi.hoisted(() => ({
  status: { value: { running: true, pid: 1234, port: 7878, http_ok: true, started_at: '2026-06-07T10:00:00Z' } },
  loading: { value: false },
  lastError: { value: null },
  refresh: vi.fn(),
  restart: vi.fn().mockResolvedValue({ running: true, pid: 1234, port: 7878, http_ok: true, started_at: '2026-06-07T10:00:00Z' }),
}))
vi.mock('@/composables/useDaemon', () => ({
  useDaemon: () => daemonMock,
  daemonState: daemonMock,
}))

vi.mock('@tauri-apps/api/app', () => ({
  getVersion: vi.fn().mockResolvedValue('0.3.4'),
}))

vi.mock('@tauri-apps/plugin-opener', () => ({
  openUrl: vi.fn().mockResolvedValue(undefined),
}))

vi.mock('vue-sonner', () => ({
  toast: { success: vi.fn(), error: vi.fn(), message: vi.fn() },
}))

describe('SystemTab', () => {
  beforeEach(() => {
    Object.values(ipcMocks).forEach((fn) => fn.mockReset())
    ipcMocks.cliStatus.mockResolvedValue({
      path_contains_target_dir: true,
      path: '/usr/local/bin:/bin',
      target_dir: '/usr/local/bin/memex',
      installed: true,
      path_export_hint: 'export PATH="/usr/local/bin:$PATH"',
    })
    ipcMocks.runDoctor.mockResolvedValue({
      data_dir: '~/.memex',
      config_present: true,
      report: {
        db_exists: true,
        schema_version: 12,
        session_count: 6500,
        message_count: 180000,
        chunk_count: 6300,
        source_count: 5,
        fts_ok: true,
        adapters: [],
      },
      cursor_probe: { status: 'ok', composer_count: 2148, db_path: '/tmp/cursor.db' },
    })
    ipcMocks.checkForUpdates.mockResolvedValue({
      latest_tag: 'v0.3.4',
      html_url: 'https://example.com/release',
    })
  })

  const stubs = {
    Card: { template: '<div><slot/></div>' },
    CardAction: { template: '<div><slot/></div>' },
    CardContent: { template: '<div><slot/></div>' },
    CardDescription: { template: '<span><slot/></span>' },
    CardFooter: { template: '<div><slot/></div>' },
    CardHeader: { template: '<div><slot/></div>' },
    CardTitle: { template: '<div><slot/></div>' },
    Badge: { template: '<span><slot/></span>' },
    Separator: { template: '<div></div>' },
    Button: {
      template: '<button :disabled="disabled" @click="$emit(\'click\')"><slot/></button>',
      props: ['disabled'],
      emits: ['click'],
    },
  }

  it('loads cli status / doctor / version on mount', async () => {
    const wrapper = mount(SystemTab, { global: { stubs } })
    await flushPromises()
    expect(ipcMocks.cliStatus).toHaveBeenCalled()
    expect(ipcMocks.runDoctor).toHaveBeenCalled()
    const text = wrapper.text()
    expect(text).toContain('v0.3.4')
    expect(text).toContain('Schema v12')
    expect(text).toContain('2,148 composers')
  })

  it('doctor 还在 loading 时显示"检查中…"占位而不是"—"或硬编码 ~/.memex', async () => {
    let resolveDoctor!: (v: any) => void
    ipcMocks.runDoctor.mockReturnValueOnce(
      new Promise((r) => {
        resolveDoctor = r
      }),
    )
    const wrapper = mount(SystemTab, { global: { stubs } })
    await flushPromises()
    // doctor 还没 resolve，应当看到"检查中…"
    expect(wrapper.text()).toContain('检查中…')
    // 不应该误导用户显示假的 ~/.memex
    expect(wrapper.text()).not.toContain('~/.memex')
    // resolve 后切回真实数据
    resolveDoctor({
      data_dir: '~/.memex',
      config_present: true,
      report: {
        db_exists: true,
        schema_version: 12,
        session_count: 1,
        message_count: 1,
        chunk_count: 1,
        source_count: 1,
        fts_ok: true,
        adapters: [],
      },
      cursor_probe: { status: 'ok', composer_count: 0, db_path: '/tmp/x.db' },
    })
    await flushPromises()
    expect(wrapper.text()).toContain('~/.memex')
    expect(wrapper.text()).not.toContain('检查中…')
  })

  it('reports "已是最新" when latest_tag matches current version', async () => {
    const wrapper = mount(SystemTab, { global: { stubs } })
    await flushPromises()
    const buttons = wrapper.findAll('button')
    const checkBtn = buttons.find((b) => b.text().includes('检查更新'))!
    await checkBtn.trigger('click')
    await flushPromises()
    expect(ipcMocks.checkForUpdates).toHaveBeenCalled()
    expect(wrapper.text()).toContain('已是最新版本')
  })
})
