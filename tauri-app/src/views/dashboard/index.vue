<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { Stats, StatsBreakdown, TimelineEntry, SessionRow, SessionDetail } from '@/types'
import { useMemex } from '@/composables/useMemex'
import { TooltipProvider } from '@/components/ui/tooltip'
import DashSidebar from './components/DashSidebar.vue'
import OverviewTab from './components/OverviewTab.vue'
import SessionsTab from './components/SessionsTab.vue'
import ProjectsTab from './components/ProjectsTab.vue'
import ReportsTab from './components/ReportsTab.vue'
import ReflectTab from './components/ReflectTab.vue'
import WorkloadTab from './components/WorkloadTab.vue'
import SearchTab from './components/SearchTab.vue'
import SessionDetailTab from './components/SessionDetailTab.vue'
import type { DashTab } from './components/DashSidebar.vue'

const { getStats, getBreakdown, getTimeline, listRecent, getSession } = useMemex()

const tab = ref<DashTab>('overview')
const stats = ref<Stats | null>(null)
const breakdown = ref<StatsBreakdown | null>(null)
const timeline = ref<TimelineEntry[]>([])
const sessions = ref<SessionRow[]>([])
const sessionsLoading = ref(false)
const detailSession = ref<SessionDetail | null>(null)
const detailLoading = ref(false)
const refreshing = ref(false)
const sessionFilter = ref('')
const sessionMessagesFilter = ref<'all' | 'invalid' | 'valid'>('all')
let refreshTimer: ReturnType<typeof setInterval> | null = null

async function loadDashboard() {
  const [s, bd, tl, ss] = await Promise.all([
    getStats().catch(() => null),
    getBreakdown().catch(() => null),
    getTimeline(30).catch(() => []),
    sessions.value.length === 0 ? listRecent(200, 0).catch(() => []) : Promise.resolve(null),
  ])
  if (s) stats.value = s
  if (bd) breakdown.value = bd
  timeline.value = tl
  if (ss) sessions.value = ss
}

async function loadSessions() {
  sessionsLoading.value = true
  try { sessions.value = await listRecent(200, 0) } catch { /* ignore */ }
  sessionsLoading.value = false
}

async function openSessionDetail(sessionId: string) {
  detailLoading.value = true
  tab.value = 'session-detail'
  try { detailSession.value = await getSession(sessionId) } catch { detailSession.value = null }
  detailLoading.value = false
}

async function manualRefresh() {
  refreshing.value = true
  await loadDashboard()
  refreshing.value = false
}

function switchTab(t: DashTab) {
  if (t !== 'sessions') {
    sessionFilter.value = ''
    sessionMessagesFilter.value = 'all'
  }
  tab.value = t
  if (t === 'sessions' && sessions.value.length === 0) loadSessions()
}

function filterByProject(projectName: string) {
  sessionFilter.value = projectName
  sessionMessagesFilter.value = 'all'
  tab.value = 'sessions'
  if (sessions.value.length === 0) loadSessions()
}

// OverviewTab 的「X 个无效会话」徽章点进来 —— 切到 Sessions tab 并把过滤器锁到 invalid
function showInvalidSessions() {
  sessionFilter.value = ''
  sessionMessagesFilter.value = 'invalid'
  tab.value = 'sessions'
  if (sessions.value.length === 0) loadSessions()
}

function handleDeepLinkUrl(raw: string) {
  try {
    const u = new URL(raw)
    if (u.protocol !== 'memex:') return
    const host = u.hostname || u.pathname.replace(/^\/+/, '').split('/')[0] || ''
    if (host === 'session') {
      const id = u.pathname.replace(/^\/+/, '').split('/').filter(Boolean).pop()
      if (id) openSessionDetail(id)
    } else if (host === 'search') {
      tab.value = 'search'
    } else if (host === 'projects') {
      tab.value = 'projects'
    }
  } catch (e) {
    console.warn('invalid deep link:', raw, e)
  }
}

let unlistenDeepLink: UnlistenFn | null = null

onMounted(async () => {
  await loadDashboard()
  refreshTimer = setInterval(loadDashboard, 15_000)

  unlistenDeepLink = await listen<string>('deep-link', (event) => {
    handleDeepLinkUrl(event.payload)
  })

  try {
    const pending = await invoke<string | null>('take_pending_deep_link')
    if (pending) handleDeepLinkUrl(pending)
  } catch { /* ignore */ }
})

onUnmounted(() => {
  if (refreshTimer) clearInterval(refreshTimer)
  unlistenDeepLink?.()
})
</script>

<template>
  <TooltipProvider>
  <div class="flex h-full w-full">
    <DashSidebar :active-tab="tab" @switch-tab="switchTab" />

    <div class="flex-1 overflow-y-auto p-6">
      <div class="mx-auto w-full max-w-7xl">
        <OverviewTab
          v-if="tab === 'overview'"
          :stats="stats"
          :breakdown="breakdown"
          :timeline="timeline"
          :recent-sessions="sessions.slice(0, 10)"
          :refreshing="refreshing"
          @refresh="manualRefresh"
          @navigate-projects="switchTab('projects')"
          @open-session="openSessionDetail"
          @show-invalid-sessions="showInvalidSessions"
        />
        <SessionsTab
          v-else-if="tab === 'sessions'"
          :sessions="sessions"
          :loading="sessionsLoading"
          :initial-filter="sessionFilter"
          :initial-messages-filter="sessionMessagesFilter"
          @open-session="openSessionDetail"
        />
        <ProjectsTab v-else-if="tab === 'projects'" @open-session="openSessionDetail" @filter-sessions="filterByProject" />
        <ReportsTab v-else-if="tab === 'reports'" />
        <ReflectTab v-else-if="tab === 'reflect'" />
        <WorkloadTab v-else-if="tab === 'workload'" />
        <SearchTab v-else-if="tab === 'search'" @open-session="openSessionDetail" />
        <SessionDetailTab v-else-if="tab === 'session-detail'" :session="detailSession" :loading="detailLoading" @back="tab = 'sessions'" @refresh-session="openSessionDetail" />
      </div>
    </div>
  </div>
  </TooltipProvider>
</template>
