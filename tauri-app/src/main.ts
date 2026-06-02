import { createApp } from 'vue'
import App from './App.vue'
import DashboardView from './views/dashboard/index.vue'
import { syncLocaleFromBackend } from './i18n'
import './styles/main.css'

const isDashboard = window.location.hash === '#/dashboard'
createApp(isDashboard ? DashboardView : App).mount('#app')

// 拉一次后台落库的 locale；新窗口启动时所有副本都看到同样的语言。
// 不 await，避免阻塞首屏；切换到正确语言时 Vue 的响应式会自动 re-render。
syncLocaleFromBackend()
