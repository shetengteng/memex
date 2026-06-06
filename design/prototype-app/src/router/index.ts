import { createRouter, createWebHashHistory, type RouteRecordRaw } from 'vue-router'

const routes: RouteRecordRaw[] = [
  { path: '/', redirect: '/today' },
  {
    path: '/today',
    name: 'today',
    component: () => import('@/views/today/index.vue'),
    meta: { title: '今天', breadcrumb: ['今天'] },
  },
  {
    path: '/library',
    name: 'library',
    component: () => import('@/views/library/index.vue'),
    meta: { title: '资料库', breadcrumb: ['资料库'] },
  },
  {
    path: '/insights',
    name: 'insights',
    component: () => import('@/views/insights/index.vue'),
    meta: { title: '洞察', breadcrumb: ['洞察'] },
  },
  {
    path: '/connect',
    name: 'connect',
    component: () => import('@/views/connect/index.vue'),
    meta: { title: '连接', breadcrumb: ['连接'] },
  },
  {
    path: '/settings',
    name: 'settings',
    component: () => import('@/views/settings/index.vue'),
    meta: { title: '设置', breadcrumb: ['设置'] },
  },
]

export const router = createRouter({
  history: createWebHashHistory(),
  routes,
})
