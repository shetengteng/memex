<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import type { Stats, StatsBreakdown, TimelineEntry, SessionRow, SessionDetail } from '@/types'
import { useMemex } from '@/composables/useMemex'
import DashSidebar from './components/DashSidebar.vue'
import OverviewTab from './components/OverviewTab.vue'
import SessionsTab from './components/SessionsTab.vue'
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
let refreshTimer: ReturnType<typeof setInterval> | null = null

async function loadDashboard() {
  const [s, bd, tl] = await Promise.all([
    getStats().catch(() => null),
    getBreakdown().catch(() => null),
    getTimeline(30).catch(() => []),
  ])
  if (s) stats.value = s
  if (bd) breakdown.value = bd
  timeline.value = tl
}

async function loadSessions() {
  sessionsLoading.value = true
  try { sessions.value = await listRecent(100, 0) } catch { /* ignore */ }
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
  tab.value = t
  if (t === 'sessions' && sessions.value.length === 0) loadSessions()
}

onMounted(async () => {
  await loadDashboard()
  refreshTimer = setInterval(loadDashboard, 15_000)
})

onUnmounted(() => { if (refreshTimer) clearInterval(refreshTimer) })
</script>

<template>
  <div class="flex h-full w-full">
    <DashSidebar :active-tab="tab" @switch-tab="switchTab" />

    <div class="flex-1 overflow-y-auto p-6">
      <div class="mx-auto max-w-5xl">
        <OverviewTab
          v-if="tab === 'overview'"
          :stats="stats"
          :breakdown="breakdown"
          :timeline="timeline"
          :refreshing="refreshing"
          @refresh="manualRefresh"
          @navigate-projects="switchTab('sessions')"
        />
        <SessionsTab v-else-if="tab === 'sessions'" :sessions="sessions" :loading="sessionsLoading" @open-session="openSessionDetail" />
        <SearchTab v-else-if="tab === 'search'" @open-session="openSessionDetail" />
        <SessionDetailTab v-else-if="tab === 'session-detail'" :session="detailSession" :loading="detailLoading" @back="tab = 'sessions'" />
      </div>
    </div>
  </div>
</template>
