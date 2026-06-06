<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { SidebarInset, SidebarProvider } from '@/components/ui/sidebar'
import AppSidebar from '@/components/shell/AppSidebar.vue'
import SiteHeader from '@/components/shell/SiteHeader.vue'
import CommandPalette from '@/components/shell/CommandPalette.vue'
import { Toaster } from '@/components/ui/sonner'

const SIDEBAR_WIDTH_KEY = 'memex.sidebar.width'
const MIN_WIDTH = 200
const MAX_WIDTH = 480
const DEFAULT_WIDTH = 288

const sidebarWidth = ref(DEFAULT_WIDTH)

onMounted(() => {
  const v = localStorage.getItem(SIDEBAR_WIDTH_KEY)
  const n = v ? Number.parseInt(v, 10) : NaN
  if (Number.isFinite(n)) sidebarWidth.value = Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, n))
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
})
</script>

<template>
  <SidebarProvider
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
      @dblclick="sidebarWidth = DEFAULT_WIDTH; localStorage.setItem(SIDEBAR_WIDTH_KEY, String(DEFAULT_WIDTH))"
    />
    <SidebarInset class="min-h-0 overflow-hidden">
      <SiteHeader />
      <RouterView />
    </SidebarInset>
    <CommandPalette />
    <Toaster position="bottom-right" />
  </SidebarProvider>
</template>
