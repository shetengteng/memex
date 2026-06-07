import { createApp } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import './style.css'
import App from './App.vue'
import { router } from './router'
import { initTheme } from './composables/useTheme'
import { syncLocaleFromBackend } from './i18n'

initTheme()

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
router.isReady().then(() => app.mount('#app'))

// 拉一次后台落库的 locale；新窗口启动时所有副本都看到同样的语言。
// 不 await，避免阻塞首屏；切换到正确语言时 Vue 的响应式会自动 re-render。
syncLocaleFromBackend()
