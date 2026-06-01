<script setup lang="ts">
import { ref, provide, onMounted, nextTick, watch } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Search, Settings, Activity } from 'lucide-vue-next'
import type { ViewName, Stats } from '@/types'
import { useMemex } from '@/composables/useMemex'
import { formatNumber } from '@/lib/utils'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import { TooltipProvider } from '@/components/ui/tooltip'
import SearchView from '@/views/SearchView.vue'
import SettingsView from '@/views/SettingsView.vue'
import StatusView from '@/views/StatusView.vue'
import SessionView from '@/views/SessionView.vue'

const currentView = ref<ViewName>('search')
const selectedSessionId = ref<string | null>(null)
const searchQuery = ref('')
const searchInputRef = ref<HTMLInputElement | null>(null)
const appWindow = getCurrentWindow()
const { getStats } = useMemex()
const stats = ref<Stats>({ sessions: 0, messages: 0, chunks: 0, db_exists: false })

function navigate(view: ViewName, sessionId?: string) {
  currentView.value = view
  if (sessionId) selectedSessionId.value = sessionId
}

function back() {
  if (currentView.value === 'session') {
    currentView.value = 'search'
  }
}

async function hidePopup() {
  currentView.value = 'search'
  await appWindow.hide()
}

function switchView(view: ViewName) {
  if (view === currentView.value && view !== 'session') return
  currentView.value = view
  if (view === 'search') {
    nextTick(() => searchInputRef.value?.focus())
  }
}

provide('navigate', navigate)
provide('back', back)

watch(currentView, (v) => {
  if (v === 'search') {
    nextTick(() => searchInputRef.value?.focus())
  }
})

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    if (currentView.value === 'session') {
      back()
    } else {
      hidePopup()
    }
  }
}

onMounted(async () => {
  appWindow.onFocusChanged(({ payload: focused }) => {
    if (!focused) hidePopup()
  })
  try {
    stats.value = await getStats()
  } catch { /* ignore */ }
})
</script>

<template>
  <TooltipProvider>
    <div class="flex h-screen w-screen items-start justify-center p-[10px]" style="background: transparent;">
    <div
      class="flex w-full flex-1 flex-col overflow-hidden rounded-xl border border-border/80 bg-card shadow-[0_8px_32px_-8px_rgba(15,23,42,0.12),0_4px_12px_-4px_rgba(15,23,42,0.08)]"
      style="max-height: calc(100vh - 20px);"
      @keydown="onKeydown"
      tabindex="0"
    >
      <!-- Search Bar -->
      <div v-if="currentView !== 'session'" class="flex items-center gap-2 border-b border-border px-3 py-2.5">
        <Search class="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
        <Input
          ref="searchInputRef"
          v-model="searchQuery"
          type="text"
          :placeholder="currentView === 'search' ? '搜索 AI 对话历史...' : '搜索...'"
          class="h-7 border-0 bg-transparent px-0 shadow-none focus-visible:ring-0"
          @focus="switchView('search')"
        />
        <kbd class="mono shrink-0 rounded border border-border bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">Esc</kbd>
      </div>

      <!-- View Content -->
      <div class="min-h-0 flex-1 overflow-hidden">
        <SearchView v-if="currentView === 'search'" :query="searchQuery" />
        <SettingsView v-else-if="currentView === 'settings'" />
        <StatusView v-else-if="currentView === 'status'" />
        <SessionView
          v-else-if="currentView === 'session'"
          :session-id="selectedSessionId ?? ''"
        />
      </div>

      <!-- Footer -->
      <Separator />
      <div class="flex items-center justify-between bg-muted/50 px-3.5 py-1.5">
        <span class="mono text-[10px] text-muted-foreground">
          {{ formatNumber(stats.sessions) }} sessions ·
          <span :class="stats.db_exists ? 'text-success' : 'text-muted-foreground'">●</span>
          {{ stats.db_exists ? 'healthy' : 'no db' }}
        </span>
        <div class="flex gap-0.5">
          <Button
            variant="ghost"
            size="icon"
            class="h-6 w-6"
            :class="{ 'bg-primary/10 text-primary': currentView === 'search' }"
            @click="switchView('search')"
            title="搜索"
          >
            <Search class="h-3.5 w-3.5" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            class="h-6 w-6"
            :class="{ 'bg-primary/10 text-primary': currentView === 'settings' }"
            @click="switchView('settings')"
            title="设置"
          >
            <Settings class="h-3.5 w-3.5" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            class="h-6 w-6"
            :class="{ 'bg-primary/10 text-primary': currentView === 'status' }"
            @click="switchView('status')"
            title="健康状态"
          >
            <Activity class="h-3.5 w-3.5" />
          </Button>
        </div>
      </div>
    </div>
    </div>
  </TooltipProvider>
</template>
