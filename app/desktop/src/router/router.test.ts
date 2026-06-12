import { describe, expect, it } from 'vitest'
import { router } from './index'

const expectedRoutes: Record<string, { layout?: string; breadcrumb?: string[] }> = {
  '/today': { breadcrumb: ['今天'] },
  '/library': { breadcrumb: ['资料库'] },
  '/insights': { breadcrumb: ['洞察'] },
  '/connect': { breadcrumb: ['连接'] },
  '/settings': { breadcrumb: ['设置'] },
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
