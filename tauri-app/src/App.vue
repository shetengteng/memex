<script setup lang="ts">
import { ref, computed, provide, onMounted, onUnmounted, nextTick, watch } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { listen } from '@tauri-apps/api/event'
import { Search, Settings, Activity, LayoutDashboard, Home, AlertTriangle, Copy, ExternalLink, Terminal, Sparkles, RefreshCw } from 'lucide-vue-next'
import { DialogRoot, DialogPortal, DialogOverlay, DialogContent, DialogTitle, DialogDescription } from 'reka-ui'
import { openUrl } from '@tauri-apps/plugin-opener'
import type { ViewName, Stats } from '@/types'
import { useMemex } from '@/composables/useMemex'
import { useI18n } from '@/i18n'
import { formatNumber } from '@/lib/utils'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { Tooltip, TooltipContent, TooltipTrigger, TooltipProvider } from '@/components/ui/tooltip'
import SearchView from '@/views/search/index.vue'
import SettingsView from '@/views/settings/index.vue'
import StatusView from '@/views/status/index.vue'
import SessionView from '@/views/session/index.vue'

const currentView = ref<ViewName>('search')
const selectedSessionId = ref<string | null>(null)
const searchQuery = ref('')
const searchInputRef = ref<HTMLInputElement | null>(null)
const appWindow = getCurrentWindow()
const { t } = useI18n()
const { getStats, getConfig, batchSummarize } = useMemex()
const stats = ref<Stats>({ sessions: 0, messages: 0, chunks: 0, db_exists: false, summaries: 0, sessions_eligible_for_summary: 0, chunks_summarized: 0, llm_provider: null })

interface SummaryProgress { current: number; total: number; done: boolean }

const batchRunning = ref(false)
const batchProgress = ref<SummaryProgress | null>(null)
let unlistenSummaryProgress: (() => void) | null = null

async function handleBatchSummarize() {
  if (batchRunning.value) return
  batchRunning.value = true
  batchProgress.value = null
  try {
    unlistenSummaryProgress = await listen<SummaryProgress>('summary-progress', (event) => {
      batchProgress.value = event.payload
      if (event.payload.done) {
        batchRunning.value = false
        refreshStats().catch(() => {})
      }
    })
    const total = await batchSummarize()
    if (total === 0) {
      batchRunning.value = false
    }
  } catch (e) {
    console.error('popup batch summarize failed:', e)
    batchRunning.value = false
  }
}

// ----- Ollama startup check -----
type OllamaSetupKind = 'not_installed' | 'no_model'
const ollamaDialogOpen = ref(false)
const ollamaSetupKind = ref<OllamaSetupKind>('not_installed')
const ollamaConfiguredModel = ref('qwen2.5')
const ollamaCmdCopied = ref(false)
const ollamaBrewCopied = ref(false)

const OLLAMA_SETUP_DISMISSED_KEY = 'ollama_setup_dismissed'

async function checkOllamaOnStartup() {
  try {
    const dismissed = await getConfig(OLLAMA_SETUP_DISMISSED_KEY)
    if (dismissed === 'true') return

    const model = await getConfig('llm.ollama_model')
    if (model) ollamaConfiguredModel.value = model.replace(/:.*$/, '')

    const resp = await fetch('http://127.0.0.1:11434/api/tags', { signal: AbortSignal.timeout(3000) })
    if (!resp.ok) {
      ollamaSetupKind.value = 'not_installed'
      ollamaDialogOpen.value = true
      return
    }
    const data = await resp.json() as { models?: { name: string }[] }
    const models = data.models ?? []
    if (models.length === 0) {
      ollamaSetupKind.value = 'no_model'
      ollamaDialogOpen.value = true
    }
  } catch {
    ollamaSetupKind.value = 'not_installed'
    ollamaDialogOpen.value = true
  }
}

function dismissOllamaDialog() {
  ollamaDialogOpen.value = false
}

async function dismissOllamaForever() {
  ollamaDialogOpen.value = false
  try {
    const { setConfig } = useMemex()
    await setConfig(OLLAMA_SETUP_DISMISSED_KEY, 'true')
  } catch { /* best-effort */ }
}

function goToSettingsFromDialog() {
  ollamaDialogOpen.value = false
  currentView.value = 'settings'
}

async function copyOllamaCmd() {
  try {
    await navigator.clipboard.writeText(`ollama pull ${ollamaConfiguredModel.value}`)
    ollamaCmdCopied.value = true
    setTimeout(() => { ollamaCmdCopied.value = false }, 1500)
  } catch { /* ignore */ }
}

async function copyBrewCmdStartup() {
  try {
    await navigator.clipboard.writeText('brew install ollama')
    ollamaBrewCopied.value = true
    setTimeout(() => { ollamaBrewCopied.value = false }, 1500)
  } catch { /* ignore */ }
}

async function openOllamaWebsite() {
  try { await openUrl('https://ollama.com/download') } catch { /* ignore */ }
}

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

function goHome() {
  searchQuery.value = ''
  selectedSessionId.value = null
  currentView.value = 'search'
  nextTick(() => searchInputRef.value?.focus())
}

const navValue = computed<string>(() => {
  if (currentView.value === 'search') return 'home'
  return currentView.value
})

function onNav(v: unknown) {
  if (typeof v !== 'string') return
  switch (v) {
    case 'home': goHome(); break
    case 'settings': switchView('settings'); break
    case 'status': switchView('status'); break
  }
}

async function openDashboard() {
  try {
    const existing = await WebviewWindow.getByLabel('dashboard')
    if (existing) {
      try {
        await existing.show()
        await existing.unminimize()
        await existing.setFocus()
        return
      } catch (e) {
        console.warn('reuse dashboard failed, recreating:', e)
        try { await existing.close() } catch {}
      }
    }
    const url = import.meta.env.DEV
      ? 'http://localhost:1420/#/dashboard'
      : 'index.html#/dashboard'
    const wv = new WebviewWindow('dashboard', {
      title: 'Memex Dashboard',
      url,
      width: 1100,
      height: 720,
      minWidth: 800,
      minHeight: 500,
      center: true,
      decorations: true,
      resizable: true,
      transparent: false,
      visible: true,
      focus: true,
    })
    wv.once('tauri://created', () => {
      wv.show().catch(() => {})
      wv.setFocus().catch(() => {})
    })
    wv.once('tauri://error', (e) => {
      console.error('Dashboard window creation failed:', e)
    })
  } catch (e) {
    console.error('openDashboard error:', e)
  } finally {
    try { await hidePopup() } catch {}
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

let statsTimer: ReturnType<typeof setInterval> | null = null

async function refreshStats() {
  try { stats.value = await getStats() } catch { /* ignore */ }
}

const summaryProgress = computed(() => {
  if (!stats.value.llm_provider) return null
  // 分母用「够资格生成摘要的 session 数」而不是全量 sessions —
  // 只有 1 条消息（甚至 0 条）的会话客观上无法生成摘要，
  // 算进分母只会让 UI 永远卡在 < 100%（实测会停在 98%）
  const total = stats.value.sessions_eligible_for_summary
  const done = stats.value.summaries
  if (total === 0) return null
  return { done, total, pct: Math.round((done / total) * 100) }
})

const pendingSummaryCount = computed(() =>
  Math.max(stats.value.sessions_eligible_for_summary - stats.value.summaries, 0),
)

// 仅当：有 LLM provider + 有未生成摘要的可摘要会话 时，「生成」按钮才可点击。
const canTriggerSummary = computed(
  () => !!stats.value.llm_provider && pendingSummaryCount.value > 0,
)

onMounted(async () => {
  appWindow.onFocusChanged(({ payload: focused }) => {
    if (!focused) hidePopup()
  })
  await refreshStats()
  statsTimer = setInterval(refreshStats, 10_000)
  checkOllamaOnStartup()
})

onUnmounted(() => {
  if (statsTimer) clearInterval(statsTimer)
  unlistenSummaryProgress?.()
})
</script>

<template>
  <TooltipProvider>
    <!-- Ollama Setup Dialog -->
    <DialogRoot v-model:open="ollamaDialogOpen">
      <DialogPortal>
        <DialogOverlay class="fixed inset-0 z-50 bg-black/50 data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0" />
        <DialogContent class="fixed left-1/2 top-1/2 z-50 w-[90vw] max-w-md -translate-x-1/2 -translate-y-1/2 rounded-xl border border-border bg-card p-5 shadow-xl">
          <div class="flex items-start gap-3">
            <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-amber-500/10">
              <AlertTriangle class="h-4.5 w-4.5 text-amber-500" />
            </div>
            <div class="flex-1 space-y-3">
              <DialogTitle class="text-sm font-semibold leading-snug">
                {{ ollamaSetupKind === 'not_installed' ? t('ollama_setup.title_not_installed') : t('ollama_setup.title_no_model') }}
              </DialogTitle>
              <DialogDescription class="text-xs leading-relaxed text-muted-foreground">
                {{ ollamaSetupKind === 'not_installed' ? t('ollama_setup.desc_not_installed') : t('ollama_setup.desc_no_model') }}
              </DialogDescription>

              <!-- Ollama not installed: show download + brew -->
              <div v-if="ollamaSetupKind === 'not_installed'" class="space-y-2">
                <Button variant="default" size="sm" class="h-7 gap-1 text-xs" @click="openOllamaWebsite">
                  <ExternalLink class="h-3 w-3" />
                  {{ t('ollama_setup.install_ollama') }}
                </Button>
                <div class="flex items-center gap-1.5 text-[11px] text-muted-foreground">
                  <span>{{ t('ollama_setup.brew_hint') }}</span>
                  <code class="rounded bg-muted px-1.5 py-0.5 font-mono text-[11px]">brew install ollama</code>
                  <button class="inline-flex cursor-pointer items-center gap-0.5 text-[11px] text-primary hover:underline" @click="copyBrewCmdStartup">
                    <Copy class="h-3 w-3" />
                    {{ ollamaBrewCopied ? t('common.copied') : t('common.copy') }}
                  </button>
                </div>
              </div>

              <!-- Model not installed: show pull command -->
              <div v-else class="space-y-2">
                <p class="text-[11px] font-medium text-muted-foreground">
                  {{ t('ollama_setup.recommended_model') }}: <strong class="text-foreground">{{ ollamaConfiguredModel }}</strong>
                </p>
                <div class="flex items-center gap-1.5">
                  <div class="flex items-center gap-1 rounded-md border border-border bg-muted px-2 py-1">
                    <Terminal class="h-3 w-3 text-muted-foreground" />
                    <code class="font-mono text-[11px]">ollama pull {{ ollamaConfiguredModel }}</code>
                  </div>
                  <Button variant="ghost" size="sm" class="h-7 px-2 text-[11px]" @click="copyOllamaCmd">
                    <Copy class="mr-0.5 h-3 w-3" />
                    {{ ollamaCmdCopied ? t('common.copied') : t('common.copy') }}
                  </Button>
                </div>
                <p class="text-[11px] leading-relaxed text-muted-foreground">
                  {{ t('ollama_setup.other_models') }}
                </p>
              </div>
            </div>
          </div>

          <!-- Actions -->
          <div class="mt-4 flex items-center justify-between border-t border-border/40 pt-3">
            <button class="cursor-pointer text-[11px] text-muted-foreground hover:text-foreground hover:underline" @click="dismissOllamaForever">
              {{ t('ollama_setup.dont_show') }}
            </button>
            <div class="flex items-center gap-2">
              <Button variant="ghost" size="sm" class="h-7 text-xs" @click="dismissOllamaDialog">
                {{ t('ollama_setup.dismiss') }}
              </Button>
              <Button variant="default" size="sm" class="h-7 text-xs" @click="goToSettingsFromDialog">
                {{ t('ollama_setup.go_settings') }}
              </Button>
            </div>
          </div>
        </DialogContent>
      </DialogPortal>
    </DialogRoot>

    <div class="flex h-screen w-screen flex-col p-[10px]" style="background: transparent;">
    <div
      class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border border-border/80 bg-card shadow-[0_8px_32px_-8px_rgba(15,23,42,0.12),0_4px_12px_-4px_rgba(15,23,42,0.08)]"
      @keydown="onKeydown"
      tabindex="0"
    >
      <!-- Search Bar -->
      <div v-if="currentView !== 'session'" class="flex items-center gap-2 border-b border-border px-3 py-3">
        <Search class="h-4 w-4 shrink-0 text-muted-foreground" />
        <Input
          ref="searchInputRef"
          v-model="searchQuery"
          type="text"
          :placeholder="t('search.placeholder')"
          class="h-8 border-0 bg-transparent px-0 text-sm shadow-none focus-visible:ring-0"
          @focus="switchView('search')"
        />
        <kbd class="mono shrink-0 rounded border border-border bg-muted px-1.5 py-0.5 text-xs text-muted-foreground">Esc</kbd>
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
      <div class="flex items-center justify-between bg-muted/50 px-4 py-2.5">
        <span class="mono flex items-center gap-1 text-xs text-muted-foreground">
          {{ formatNumber(stats.sessions) }} · 
          <span :class="stats.db_exists ? 'text-success' : 'text-muted-foreground'">●</span>
          {{ stats.db_exists ? t('common.ready') : t('status.db.not_initialized') }}
          <template v-if="stats.llm_provider">
            · <span class="text-primary">{{ stats.llm_provider }}</span>
            <span v-if="summaryProgress" :title="`${summaryProgress.done}/${summaryProgress.total}`">
              {{ summaryProgress.pct }}%
            </span>
            <Tooltip v-if="canTriggerSummary || batchRunning">
              <TooltipTrigger as-child>
                <button
                  class="ml-0.5 inline-flex h-5 w-5 cursor-pointer items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-primary disabled:cursor-not-allowed disabled:opacity-50"
                  :disabled="batchRunning || !canTriggerSummary"
                  :aria-label="t('popup.summary.generate_tooltip')"
                  @click="handleBatchSummarize"
                >
                  <RefreshCw v-if="batchRunning" class="h-3 w-3 animate-spin" />
                  <Sparkles v-else class="h-3 w-3" />
                </button>
              </TooltipTrigger>
              <TooltipContent side="top">
                <template v-if="batchRunning && batchProgress">
                  {{ t('overview.summary.processed', { current: batchProgress.current, total: batchProgress.total }) }}
                </template>
                <template v-else-if="batchRunning">
                  {{ t('overview.summary.generating') }}
                </template>
                <template v-else>
                  {{ t('popup.summary.generate_tooltip_with_count', { count: pendingSummaryCount }) }}
                </template>
              </TooltipContent>
            </Tooltip>
          </template>
        </span>
        <div class="flex items-center gap-2">
          <ToggleGroup
            type="single"
            size="xl"
            :model-value="navValue"
            @update:model-value="onNav"
          >
            <Tooltip>
              <TooltipTrigger as-child>
                <ToggleGroupItem value="home" :aria-label="t('nav.home')">
                  <Home class="h-5 w-5" />
                </ToggleGroupItem>
              </TooltipTrigger>
              <TooltipContent side="top">{{ t('nav.home') }}</TooltipContent>
            </Tooltip>
            <Tooltip>
              <TooltipTrigger as-child>
                <ToggleGroupItem value="settings" :aria-label="t('nav.settings')">
                  <Settings class="h-5 w-5" />
                </ToggleGroupItem>
              </TooltipTrigger>
              <TooltipContent side="top">{{ t('nav.settings') }}</TooltipContent>
            </Tooltip>
            <Tooltip>
              <TooltipTrigger as-child>
                <ToggleGroupItem value="status" :aria-label="t('nav.status')">
                  <Activity class="h-5 w-5" />
                </ToggleGroupItem>
              </TooltipTrigger>
              <TooltipContent side="top">{{ t('nav.status') }}</TooltipContent>
            </Tooltip>
          </ToggleGroup>
          <Separator orientation="vertical" class="h-7" />
          <Tooltip>
            <TooltipTrigger as-child>
              <Button
                variant="ghost"
                size="icon"
                class="h-11 w-11"
                @click="openDashboard"
              >
                <LayoutDashboard class="h-5 w-5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="top">{{ t('nav.dashboard') }}</TooltipContent>
          </Tooltip>
        </div>
      </div>
    </div>
    </div>
  </TooltipProvider>
</template>
