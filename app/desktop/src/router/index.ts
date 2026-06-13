import { createRouter, createWebHashHistory, type RouteRecordRaw } from 'vue-router'

// breadcrumb / title 存的是 i18n key（不是字面文案），SiteHeader 渲染时会
// 通过 translate() 解析成当前 locale 的实际文本，所以切换语言能整页跟随。
const routes: RouteRecordRaw[] = [
  { path: '/', redirect: '/today' },
  {
    path: '/today',
    name: 'today',
    component: () => import('@/views/today/index.vue'),
    meta: { title: 'nav.today', breadcrumb: ['nav.today'] },
  },
  {
    path: '/library',
    name: 'library',
    component: () => import('@/views/library/index.vue'),
    meta: { title: 'nav.library', breadcrumb: ['nav.library'] },
  },
  {
    path: '/insights',
    name: 'insights',
    component: () => import('@/views/insights/index.vue'),
    meta: { title: 'nav.insights', breadcrumb: ['nav.insights'] },
  },
  {
    path: '/connect',
    name: 'connect',
    component: () => import('@/views/connect/index.vue'),
    meta: { title: 'nav.connect', breadcrumb: ['nav.connect'] },
  },
  {
    path: '/settings',
    name: 'settings',
    component: () => import('@/views/settings/index.vue'),
    meta: { title: 'nav.settings', breadcrumb: ['nav.settings'] },
  },
  {
    path: '/logs',
    name: 'logs',
    component: () => import('@/views/logs/index.vue'),
    meta: { title: 'nav.logs', breadcrumb: ['nav.settings', 'nav.logs'] },
  },
  {
    path: '/tray-popup',
    name: 'tray-popup',
    component: () => import('@/views/tray-popup/index.vue'),
    meta: { title: 'Tray', layout: 'bare' },
  },
]

export const router = createRouter({
  history: createWebHashHistory(),
  routes,
})
