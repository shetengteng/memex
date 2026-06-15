import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createMemoryHistory, createRouter } from 'vue-router'
import HelpView from './index.vue'

vi.mock('@/i18n', () => ({
  useI18n: () => ({ t: (k: string) => k }),
}))

const TAB_VALUES = [
  'quickstart',
  'integrations',
  'mcp',
  'skills',
  'context',
  'privacy',
  'troubleshooting',
] as const

let headerHost: HTMLElement | null = null

beforeEach(() => {
  // SiteHeader 在主 app shell 提供 #memex-header-center 给 Teleport，
  // 但单测里没有 shell，需要手动放一个，否则 Vue 会拒绝 mount。
  headerHost = document.createElement('div')
  headerHost.id = 'memex-header-center'
  document.body.appendChild(headerHost)
})

afterEach(() => {
  if (headerHost && headerHost.parentNode) headerHost.parentNode.removeChild(headerHost)
  headerHost = null
})

function makeHarness(initialPath = '/help') {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/help', name: 'help', component: HelpView },
      { path: '/:rest(.*)*', name: 'catchall', component: { template: '<div />' } },
    ],
  })
  router.push(initialPath)
  return router
}

describe('HelpView', () => {
  it('mounts with quickstart tab by default and renders its markdown', async () => {
    const router = makeHarness()
    await router.isReady()
    const wrapper = mount(HelpView, {
      global: { plugins: [router] },
      attachTo: document.body,
    })
    await flushPromises()

    expect(document.body.innerHTML).toContain('快速开始')
    expect(document.body.innerHTML).toContain('Memex 是一个')
    wrapper.unmount()
  })

  it('falls back to quickstart when ?tab is invalid', async () => {
    const router = makeHarness('/help?tab=does-not-exist')
    await router.isReady()
    const wrapper = mount(HelpView, {
      global: { plugins: [router] },
      attachTo: document.body,
    })
    await flushPromises()

    expect(document.body.innerHTML).toContain('快速开始')
    wrapper.unmount()
  })

  it('renders one panel per known tab', async () => {
    const router = makeHarness()
    await router.isReady()
    const wrapper = mount(HelpView, {
      global: { plugins: [router] },
      attachTo: document.body,
    })
    await flushPromises()

    const panels = document.body.querySelectorAll('[role="tabpanel"]')
    expect(panels.length).toBe(TAB_VALUES.length)
    wrapper.unmount()
  })
})
