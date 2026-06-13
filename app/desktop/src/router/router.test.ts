import { describe, expect, it } from 'vitest'
import { router } from './index'

// breadcrumb 现在存 i18n key（SiteHeader 渲染时调 translate() 解析），不再是字面中文。
const expectedRoutes: Record<string, { layout?: string; breadcrumb?: string[] }> = {
  '/today': { breadcrumb: ['nav.today'] },
  '/library': { breadcrumb: ['nav.library'] },
  '/insights': { breadcrumb: ['nav.insights'] },
  '/connect': { breadcrumb: ['nav.connect'] },
  '/settings': { breadcrumb: ['nav.settings'] },
  '/tray-popup': { layout: 'bare' },
}

describe('router', () => {
  it('uses hash history (Tauri webview-friendly)', () => {
    // createWebHashHistory 在 jsdom/happy-dom 下 base = '/#'
    expect(router.options.history.base).toMatch(/#/)
  })

  it('exposes all expected top-level routes', () => {
    const paths = router.getRoutes().map((r) => r.path)
    for (const p of Object.keys(expectedRoutes)) {
      expect(paths).toContain(p)
    }
  })

  it('redirects / to /today', async () => {
    await router.push('/')
    await router.isReady()
    expect(router.currentRoute.value.path).toBe('/today')
  })

  it('navigates to /tray-popup with bare layout meta', async () => {
    await router.push('/tray-popup')
    await router.isReady()
    expect(router.currentRoute.value.path).toBe('/tray-popup')
    expect(router.currentRoute.value.meta.layout).toBe('bare')
  })

  it('marks tray-popup as bare layout', () => {
    const tray = router.getRoutes().find((r) => r.path === '/tray-popup')
    expect(tray).toBeDefined()
    expect(tray?.meta?.layout).toBe('bare')
  })

  it('main routes have breadcrumbs (sidebar layout)', () => {
    for (const [path, expected] of Object.entries(expectedRoutes)) {
      if (!expected.breadcrumb) continue
      const r = router.getRoutes().find((x) => x.path === path)
      expect(r?.meta?.breadcrumb).toEqual(expected.breadcrumb)
    }
  })
})
