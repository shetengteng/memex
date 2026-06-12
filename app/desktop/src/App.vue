<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { SidebarInset, SidebarProvider } from '@/components/ui/sidebar'
import AppSidebar from '@/components/shell/AppSidebar.vue'
import SiteHeader from '@/components/shell/SiteHeader.vue'
import CommandPalette from '@/components/shell/CommandPalette.vue'
import OllamaSetupDialog from '@/components/shell/OllamaSetupDialog.vue'
import { Toaster } from '@/components/ui/sonner'
import { useCommandPalette } from '@/composables/useCommandPalette'
import { initMemexStore, refreshSessions, refreshProjects, refreshBreakdown } from '@/stores/memex'
import { useStats } from '@/composables/useStats'
import { useDaemon } from '@/composables/useDaemon'
import { resetScanState } from '@/composables/useScanState'

const route = useRoute()
const router = useRouter()
const palette = useCommandPalette()

let windowLabel: string | null = null
try {
  windowLabel = getCurrentWindow().label
} catch {
  windowLabel = null
}
const isTrayPopupWindow = windowLabel === 'tray-popup'
const bareLayout = computed(() => isTrayPopupWindow || route.meta?.layout === 'bare')

if (!isTrayPopupWindow) {
  useStats()
  useDaemon()
}

const SIDEBAR_WIDTH_KEY = 'memex.sidebar.width'
const MIN_WIDTH = 200
const MAX_WIDTH = 480
const DEFAULT_WIDTH = 288

const sidebarWidth = ref(DEFAULT_WIDTH)

const unlisteners: UnlistenFn[] = []

// memex://session/<id> / memex://search / memex://projects / memex://goto/<page>
function handleDeepLink(raw: string) {
  try {
    const u = new URL(raw)
    if (u.protocol !== 'memex:') return
    const host = u.hostname || u.pathname.replace(/^\/+/, '').split('/')[0] || ''
    if (host === 'session') {
      const id = u.pathname.replace(/^\/+/, '').split('/').filter(Boolean).pop()
      if (id) router.push(`/library?session=${id}`)
      return
    }
    if (host === 'search') {
      palette.open()
      return
    }
    if (host === 'projects') {
      router.push('/library')
      return
    }
    if (host === 'goto') {
      const page = u.pathname.replace(/^\/+/, '').split('/').filter(Boolean)[0] || ''
      const allow = new Set(['today', 'library', 'insights', 'connect', 'settings'])
      if (allow.has(page)) router.push(`/${page}`)
    }
  } catch {
    /* ignore */
  }
}

onMounted(async () => {
  const v = localStorage.getItem(SIDEBAR_WIDTH_KEY)
  const n = v ? Number.parseInt(v, 10) : NaN
  if (Number.isFinite(n)) sidebarWidth.value = Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, n))

  // tray-popup 是裸布局，不挂全局事件监听（CommandPalette / Toaster 都没 mount）
  if (bareLayout.value) return

  void initMemexStore()

  // Cold-start 场景：用户用 `open memex://...` 启动应用，listen 注册前 backend 已经 pending。
  // 这里把 pending 拉一次，确保不丢链接。
  try {
    const pending = await invoke<string | null>('take_pending_deep_link')
    if (pending) handleDeepLink(pending)
  } catch {
    /* ignore — 非 Tauri 环境或命令未注册 */
  }

  unlisteners.push(
    await listen('open-command-palette', () => palette.open()),
    await listen<string>('navigate', (e) => {
      if (typeof e.payload === 'string' && e.payload) router.push(e.payload)
    }),
    await listen<string>('deep-link', (e) => handleDeepLink(e.payload)),
    // 采集完成 / 摘要完成 → 把 reactive store 里的 sessions/projects/breakdown 重拉
    await listen('reset-complete', () => {
      resetScanState()
      void refreshSessions()
      void refreshProjects()
      void refreshBreakdown()
    }),
    await listen('summary-progress', () => {
      void refreshSessions()
    }),
  )
})

let dragging = false
function startDrag(e: MouseEvent) {
  e.preventDefault()
  dragging = true
  document.body.style.cursor = 'col-resize'
  document.body.style.userSelect = 'none'
  window.addEventListener('mousemove', onDrag)
  window.addEventListener('mouseup', stopDrag)
}
function onDrag(e: MouseEvent) {
  if (!dragging) return
  const w = Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, e.clientX))
  sidebarWidth.value = w
}
function stopDrag() {
  if (!dragging) return
  dragging = false
  document.body.style.cursor = ''
  document.body.style.userSelect = ''
  window.removeEventListener('mousemove', onDrag)
  window.removeEventListener('mouseup', stopDrag)
  localStorage.setItem(SIDEBAR_WIDTH_KEY, String(sidebarWidth.value))
}

onBeforeUnmount(() => {
  window.removeEventListener('mousemove', onDrag)
  window.removeEventListener('mouseup', stopDrag)
  for (const un of unlisteners) un()
})

function resetSidebarWidth() {
  sidebarWidth.value = DEFAULT_WIDTH
  localStorage.setItem(SIDEBAR_WIDTH_KEY, String(DEFAULT_WIDTH))
}
</script>

<template>
  <RouterView v-if="bareLayout" />
  <SidebarProvider
    v-else
    class="h-svh overflow-hidden"
    :style="{
      '--sidebar-width': sidebarWidth + 'px',
      '--header-height': 'calc(var(--spacing) * 12)',
    }"
  >
    <AppSidebar />
    <div
      class="group/resize fixed top-0 z-30 hidden h-svh w-1 cursor-col-resize select-none transition-colors hover:bg-primary/40 md:block"
      :style="{ left: sidebarWidth - 2 + 'px' }"
      @mousedown="startDrag"
      @dblclick="resetSidebarWidth"
    />
    <SidebarInset class="min-h-0 overflow-hidden">
      <SiteHeader />
      <RouterView />
    </SidebarInset>
    <CommandPalette />
    <OllamaSetupDialog />
    <Toaster position="bottom-right" close-button rich-colors />
  </SidebarProvider>
</template>
