<script setup lang="ts">
import { ref, computed, onUnmounted } from 'vue'
import { RefreshCw, Sparkles } from 'lucide-vue-next'
import { listen } from '@tauri-apps/api/event'
import type { Stats, StatsBreakdown, TimelineEntry, SessionRow, SummaryProgress } from '@/types'
import { formatNumber, adapterLabel, timeAgo } from '@/lib/utils'
import { useMemex } from '@/composables/useMemex'
import { useI18n } from '@/i18n'
import { Button } from '@/components/ui/button'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'

const { t } = useI18n()

const props = defineProps<{
  stats: Stats | null
  breakdown: StatsBreakdown | null
  timeline: TimelineEntry[]
  recentSessions: SessionRow[]
  refreshing?: boolean
}>()

const emit = defineEmits<{
  refresh: []
  navigateProjects: []
  openSession: [sessionId: string]
}>()

const { batchSummarize } = useMemex()
const batchRunning = ref(false)
const batchProgress = ref<SummaryProgress | null>(null)
const batchError = ref('')

let unlistenProgress: (() => void) | null = null

async function handleBatchSummarize() {
  batchRunning.value = true
  batchProgress.value = null
  batchError.value = ''

  unlistenProgress = await listen<SummaryProgress>('summary-progress', (event) => {
    batchProgress.value = event.payload
    if (event.payload.done) {
      batchRunning.value = false
      emit('refresh')
    }
  })

  try {
    const total = await batchSummarize()
    if (total === 0) {
      batchError.value = t('overview.summary.all_done')
      batchRunning.value = false
    }
  } catch (e: unknown) {
    batchError.value = e instanceof Error ? e.message : String(e)
    batchRunning.value = false
  }
}

onUnmounted(() => { unlistenProgress?.() })

const adapterColors: Record<string, string> = {
  claude_code: '#3b82f6', cursor: '#a78bfa', codex: '#22d3ee',
  opencode: '#22c55e', aider: '#f59e0b', continue_dev: '#4ade80', cline: '#ec4899',
}

const TIMELINE_DAYS = 30

function localDateKey(d: Date): string {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}

const timelineDates = computed(() => {
  const grouped = new Map<string, { sessions: number; messages: number }>()
  for (const e of props.timeline) {
    const prev = grouped.get(e.date) ?? { sessions: 0, messages: 0 }
    grouped.set(e.date, { sessions: prev.sessions + e.sessions, messages: prev.messages + e.messages })
  }
  const today = new Date()
  today.setHours(0, 0, 0, 0)
  const out: { date: string; sessions: number; messages: number }[] = []
  for (let i = TIMELINE_DAYS - 1; i >= 0; i--) {
    const d = new Date(today)
    d.setDate(today.getDate() - i)
    const key = localDateKey(d)
    const v = grouped.get(key) ?? { sessions: 0, messages: 0 }
    out.push({ date: key, sessions: v.sessions, messages: v.messages })
  }
  return out
})

const timelineMax = computed(() => Math.max(...timelineDates.value.map((d) => d.sessions), 1))

// Bars often span 1..700; on a linear scale the long-tail days collapse
// to invisible 1-pixel slivers. log1p flattens the dynamic range so a
// 5-session day still reads as a real bar next to a 700-session day.
const timelineLogMax = computed(() => Math.log1p(timelineMax.value))

function barHeightPercent(sessions: number): number {
  if (sessions <= 0) return 0
  const ratio = Math.log1p(sessions) / timelineLogMax.value
  // floor at 12% so any non-zero day is legible
  return Math.max(ratio * 100, 12)
}

const timelineTotal = computed(() => timelineDates.value.reduce((acc, d) => acc + d.sessions, 0))
const timelineActiveDays = computed(() => timelineDates.value.filter((d) => d.sessions > 0).length)
const timelinePeakDay = computed(() => timelineDates.value.reduce((best, d) => (d.sessions > best.sessions ? d : best), { date: '', sessions: 0, messages: 0 }))

const projectCount = computed(() => {
  if (!props.breakdown) return 0
  return Object.keys(props.breakdown.by_project).length
})

const adapterEntries = computed(() => {
  if (!props.breakdown) return []
  return Object.entries(props.breakdown.by_adapter).sort((a, b) => b[1] - a[1])
})

const totalSessions = computed(() => props.stats?.sessions ?? 0)

function summaryLine(s: SessionRow): string {
  const c = (s.summary_title ?? s.title ?? s.first_user_message ?? '').trim()
  if (!c) return '—'
  return c.length > 90 ? c.slice(0, 90) + '…' : c
}

function projectName(p: string): string {
  return p.split('/').filter(Boolean).pop() ?? p
}

const topProjects = computed<Array<[string, number]>>(() => {
  if (!props.breakdown) return []
  return Object.entries(props.breakdown.by_project)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 10)
})
</script>

<template>
  <header class="mb-6 flex items-end justify-between">
    <div>
      <h2 class="text-2xl font-bold tracking-tight">{{ t('dashboard.title') }}</h2>
      <p class="mt-1 text-xs text-muted-foreground">{{ t('dashboard.subtitle', { count: projectCount }) }}</p>
    </div>
    <Button variant="ghost" size="sm" @click="emit('refresh')" :disabled="refreshing" class="h-8">
      <RefreshCw class="mr-1.5 h-3.5 w-3.5" :class="{ 'animate-spin': refreshing }" />
      {{ t('common.refresh') }}
    </Button>
  </header>

  <!-- KPI strip -->
  <section class="grid grid-cols-[1.4fr_1fr_1fr_1fr] gap-x-8 gap-y-1 border-y border-border py-5">
    <div>
      <div class="text-xs font-medium text-muted-foreground">{{ t('overview.kpi.total_sessions') }}</div>
      <div class="mt-1 flex items-baseline gap-2">
        <span class="text-3xl font-bold tabular-nums">{{ formatNumber(stats?.sessions ?? 0) }}</span>
        <span class="text-xs text-muted-foreground">{{ formatNumber(stats?.messages ?? 0) }} {{ t('overview.kpi.messages_suffix') }}</span>
      </div>
    </div>
    <Button
      variant="ghost"
      class="group h-auto justify-start rounded-md px-2 py-1 text-left hover:bg-muted/50"
      @click="emit('navigateProjects')"
    >
      <div class="w-full">
        <div class="text-xs font-medium text-muted-foreground">{{ t('overview.kpi.projects') }}</div>
        <div class="mt-1 flex items-baseline gap-2">
          <span class="text-3xl font-bold tabular-nums text-primary group-hover:underline">{{ projectCount }}</span>
          <span class="text-xs text-muted-foreground">{{ t('overview.kpi.view_all') }}</span>
        </div>
      </div>
    </Button>
    <div>
      <div class="text-xs font-medium text-muted-foreground">{{ t('overview.kpi.last_7d') }}</div>
      <div class="mt-1 flex items-baseline gap-2">
        <span class="text-3xl font-bold tabular-nums">{{ formatNumber(breakdown?.recent_7d_sessions ?? 0) }}</span>
        <span class="text-xs text-muted-foreground">{{ formatNumber(breakdown?.recent_7d_messages ?? 0) }} {{ t('overview.kpi.msg_short') }}</span>
      </div>
    </div>
    <div>
      <div class="text-xs font-medium text-muted-foreground">{{ t('overview.kpi.last_30d') }}</div>
      <div class="mt-1 flex items-baseline gap-2">
        <span class="text-3xl font-bold tabular-nums">{{ formatNumber(breakdown?.recent_30d_sessions ?? 0) }}</span>
        <span class="text-xs text-muted-foreground">{{ formatNumber(breakdown?.recent_30d_messages ?? 0) }} {{ t('overview.kpi.msg_short') }}</span>
      </div>
    </div>
  </section>

  <!-- Timeline -->
  <section class="mt-8">
    <div class="mb-3 flex items-baseline justify-between">
      <h3 class="text-sm font-semibold">{{ t('overview.timeline.title') }}</h3>
      <span class="text-xs text-muted-foreground">
        {{ t('overview.timeline.meta', { days: TIMELINE_DAYS, total: formatNumber(timelineTotal), days_active: timelineActiveDays }) }}
        <template v-if="timelinePeakDay.sessions > 0">
          {{ t('overview.timeline.peak', { count: formatNumber(timelinePeakDay.sessions), date: timelinePeakDay.date }) }}
        </template>
      </span>
    </div>
    <div class="flex h-40 items-end gap-[3px]">
      <div v-for="d in timelineDates" :key="d.date" class="group relative flex h-full flex-1 flex-col items-center justify-end">
        <div
          class="w-full rounded-sm transition-colors"
          :class="d.sessions > 0 ? 'bg-primary/40 group-hover:bg-primary' : 'bg-muted'"
          :style="{ height: d.sessions > 0 ? barHeightPercent(d.sessions) + '%' : '4px' }"
        />
        <div class="pointer-events-none absolute bottom-full z-10 mb-1.5 hidden whitespace-nowrap rounded-md border border-border bg-popover px-2 py-1.5 text-xs shadow-md group-hover:block">
          <div class="font-semibold tabular-nums">{{ d.date }}</div>
          <div v-if="d.sessions > 0" class="mt-0.5 text-muted-foreground">{{ t('overview.timeline.tooltip_sessions', { sessions: d.sessions, messages: d.messages }) }}</div>
          <div v-else class="mt-0.5 text-muted-foreground">{{ t('overview.timeline.no_activity') }}</div>
        </div>
      </div>
    </div>
    <div class="mt-1 flex justify-between text-[10px] text-muted-foreground">
      <span>{{ timelineDates[0]?.date }}</span>
      <span>{{ timelineDates[Math.floor(timelineDates.length / 2)]?.date }}</span>
      <span>{{ timelineDates[timelineDates.length - 1]?.date }}</span>
    </div>
  </section>

  <!-- By IDE / Tool -->
  <section v-if="breakdown" class="mt-6">
    <h4 class="mb-3 text-sm font-semibold">{{ t('overview.adapters.title') }}</h4>
    <div class="space-y-2">
      <div v-for="[name, count] in adapterEntries" :key="name" class="flex items-center gap-3 text-sm">
        <span class="h-2.5 w-2.5 shrink-0 rounded-sm" :style="{ background: adapterColors[name] ?? '#71717a' }" />
        <span class="w-28 shrink-0 truncate text-xs font-medium">{{ adapterLabel(name) }}</span>
        <div class="flex-1">
          <div class="h-2 overflow-hidden rounded-full bg-muted">
            <div class="h-full rounded-full transition-all" :style="{ width: (totalSessions > 0 ? count / totalSessions * 100 : 0) + '%', background: adapterColors[name] ?? '#71717a' }" />
          </div>
        </div>
        <span class="mono w-24 shrink-0 text-right text-xs text-muted-foreground">{{ count }} ({{ totalSessions > 0 ? Math.round(count / totalSessions * 100) : 0 }}%)</span>
      </div>
    </div>
  </section>

  <!-- Top Projects -->
  <section v-if="breakdown" class="mt-8">
    <h4 class="mb-3 text-sm font-semibold">{{ t('overview.projects.title') }}</h4>
    <div class="space-y-2">
      <div v-for="[name, count] in topProjects" :key="name" class="flex items-center gap-3 text-sm">
        <Tooltip>
          <TooltipTrigger as-child>
            <span class="w-32 shrink-0 truncate text-xs font-medium">{{ projectName(name) }}</span>
          </TooltipTrigger>
          <TooltipContent side="top">{{ name }}</TooltipContent>
        </Tooltip>
        <div class="flex-1">
          <div class="h-2 overflow-hidden rounded-full bg-muted">
            <div class="h-full rounded-full bg-primary/60 transition-all" :style="{ width: (totalSessions > 0 ? count / totalSessions * 100 : 0) + '%' }" />
          </div>
        </div>
        <span class="mono w-12 shrink-0 text-right text-xs text-muted-foreground">{{ count }}</span>
      </div>
    </div>
  </section>

  <!-- Recent Sessions -->
  <section v-if="recentSessions.length" class="mt-8">
    <h4 class="mb-3 text-sm font-semibold">{{ t('overview.recent.title') }}</h4>
    <div class="divide-y divide-border/60">
      <Button
        v-for="s in recentSessions"
        :key="s.id"
        variant="ghost"
        class="grid h-auto w-full grid-cols-[110px_1fr_auto] items-center gap-3 rounded-none px-0 py-2.5 text-left hover:bg-accent/40"
        @click="emit('openSession', s.id)"
      >
        <span class="flex items-center gap-2 text-xs">
          <span class="h-2 w-2 shrink-0 rounded-full" :style="{ background: adapterColors[s.source] ?? '#71717a' }" />
          <span class="truncate font-medium">{{ projectName(s.project_path ?? '—') }}</span>
        </span>
        <span class="truncate text-xs font-normal text-muted-foreground">{{ summaryLine(s) }}</span>
        <span class="mono shrink-0 text-xs font-normal text-muted-foreground">{{ timeAgo(s.updated_at) }}</span>
      </Button>
    </div>
  </section>

  <!-- LLM Summary -->
  <section v-if="stats" class="mt-8">
    <div class="mb-3 flex items-center justify-between">
      <div class="flex items-baseline gap-3">
        <h4 class="text-sm font-semibold">{{ t('overview.summary.title') }}</h4>
        <span class="mono text-xs text-muted-foreground">
          {{ stats.summaries }} / {{ stats.sessions }} ·
          <span :class="stats.llm_provider ? 'text-primary' : 'text-warning'">{{ stats.llm_provider ?? t('overview.summary.provider_none') }}</span>
        </span>
      </div>
      <Button
        v-if="stats.llm_provider && stats.summaries < stats.sessions"
        variant="outline"
        size="sm"
        class="h-7 text-xs"
        :disabled="batchRunning"
        @click="handleBatchSummarize"
      >
        <Sparkles v-if="!batchRunning" class="mr-1 h-3 w-3" />
        <RefreshCw v-else class="mr-1 h-3 w-3 animate-spin" />
        {{ batchRunning ? t('overview.summary.generating') : t('overview.summary.generate_missing', { count: stats.sessions - stats.summaries }) }}
      </Button>
    </div>
    <div v-if="stats.llm_provider" class="h-1.5 overflow-hidden rounded-full bg-muted">
      <div class="h-full rounded-full bg-primary transition-all" :style="{ width: (stats.sessions > 0 ? stats.summaries / stats.sessions * 100 : 0) + '%' }" />
    </div>
    <div v-if="batchProgress" class="mt-2">
      <div class="mb-1 flex items-center justify-between text-xs text-muted-foreground">
        <span>{{ t('overview.summary.processed', { current: batchProgress.current, total: batchProgress.total }) }}</span>
        <span>{{ Math.round(batchProgress.current / batchProgress.total * 100) }}%</span>
      </div>
      <div class="h-1.5 overflow-hidden rounded-full bg-muted">
        <div
          class="h-full rounded-full bg-success transition-all duration-300"
          :style="{ width: (batchProgress.current / batchProgress.total * 100) + '%' }"
        />
      </div>
      <p v-if="batchProgress.done" class="mt-1 text-xs text-success">{{ t('overview.summary.done') }}</p>
    </div>
    <p v-if="batchError" class="mt-2 text-xs text-destructive">{{ batchError }}</p>
  </section>
</template>
