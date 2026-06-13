import { createApp } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import './style.css'
import App from './App.vue'
import { router } from './router'
import { initTheme } from './composables/useTheme'
import { initSurface } from './composables/useSurface'
import { syncLocaleFromBackend } from './i18n'

/**
 * 把任何 unhandled JS error / promise rejection 直接呈现在屏幕上。
 *
 * 历史上有过这种情况：release Tauri webview 没 devtools，应用启动早期某条
 * import 失败或 onMounted 抛错就整个空白页，用户除了「打开是空白页」啥都
 * 看不到，工程侧也只能凭经验猜。这个 overlay 不替代 toast / 业务错误，
 * 只兜底「app 根本没渲染出来」这一类灾难场景：渲染一个固定定位、可滚动、
 * 高 z-index 的红色面板，把 stack 直接打印出来。
 *
 * 当 #app 已经有子节点（说明 Vue 至少跑起来过一次）时，不打整版面板，只
 * 写一行小字到 console —— 业务里有自己的 toast 系统，不需要 overlay 抢戏。
 */
function showStartupErrorOverlay(label: string, detail: string): void {
  if (typeof document === 'undefined') return
  const app = document.getElementById('app')
  if (app != null && app.children.length > 0) {
    // 应用已经 mount 成功，运行时错误不打整版面板，让业务 toast 接管。
    console.error(`[${label}]`, detail)
    return
  }
  const id = 'memex-startup-error-overlay'
  let el = document.getElementById(id) as HTMLPreElement | null
  if (el == null) {
    el = document.createElement('pre')
    el.id = id
    el.style.cssText = [
      'position:fixed',
      'inset:0',
      'margin:0',
      'padding:24px',
      'background:#1a1a1a',
      'color:#ff6b6b',
      'font:12px/1.55 ui-monospace,Menlo,monospace',
      'white-space:pre-wrap',
      'overflow:auto',
      'z-index:2147483647',
    ].join(';')
    document.body?.appendChild(el)
  }
  el.textContent = `${el.textContent ?? ''}\n[${label}]\n${detail}\n`.trim()
}

window.addEventListener('error', (e) => {
  const detail = e.error?.stack ?? e.message ?? String(e)
  showStartupErrorOverlay('window.error', detail)
})
window.addEventListener('unhandledrejection', (e) => {
  const reason = e.reason
  const detail =
    reason instanceof Error ? (reason.stack ?? reason.message) : String(reason)
  showStartupErrorOverlay('unhandledrejection', detail)
})

initTheme()
void initSurface()

// 通过 Tauri 窗口 label 兜底纠正首屏 URL（防御 hash url 在某些场景丢失的问题）。
// 关键：直接操作 window.location.hash，必须在 createRouter 走完初始化（即 app.use(router) 触发的 init）之前完成。
// 不能用 router.replace 替代，因为 router.replace 依赖 app.use(router) 后才有 history 实例（参考 vuejs/vue-router#795）。
function bootstrapByWindowLabel(): void {
  try {
    const label = getCurrentWindow().label
    if (label === 'tray-popup') {
      // 1. 路由对齐：只在 tray-popup 窗口强制对齐；main 窗口让 router 默认行为接管（空 hash → /today）
      if (window.location.hash !== '#/tray-popup') {
        window.location.hash = '/tray-popup'
      }
      // 2. 透明背景：tauri.conf.json 设了 transparent: true 但 style.css 中 body 默认 bg-background（白色）会盖住。
      //    在 mount 前直接给 html/body 加 transparent class，避免短暂白屏闪烁
      document.documentElement.classList.add('tray-popup-window')
    }
  } catch {
    // 不在 Tauri 环境（vitest / SSR / 浏览器调试）静默跳过
  }
}

bootstrapByWindowLabel()

const app = createApp(App).use(router)

// Vue 组件 setup / render 阶段抛错时，Vue 不会冒泡到 window.onerror，得在
// app.config.errorHandler 里转发到同一个 overlay，否则 setup 里的 throw
// 会让首屏白屏却没有任何痕迹。
app.config.errorHandler = (err, _vm, info) => {
  const detail = err instanceof Error ? (err.stack ?? err.message) : String(err)
  showStartupErrorOverlay(`Vue:${info}`, detail)
}

router.isReady().then(() => app.mount('#app'))

// 拉一次后台落库的 locale；新窗口启动时所有副本都看到同样的语言。
// 不 await，避免阻塞首屏；切换到正确语言时 Vue 的响应式会自动 re-render。
syncLocaleFromBackend()
