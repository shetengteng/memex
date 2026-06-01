<script setup lang="ts">
import { ref, computed, onUnmounted } from 'vue'
import { RefreshCw, Sparkles } from 'lucide-vue-next'
import { listen } from '@tauri-apps/api/event'
import type { Stats, StatsBreakdown, TimelineEntry, SessionRow, SummaryProgress } from '@/types'
import { formatNumber, adapterLabel, timeAgo, adapterColor, adapterBg } from '@/lib/utils'
import { useMemex } from '@/composables/useMemex'
import { Button } from '@/components/ui/button'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'

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
      batchError.value = 'All sessions already have summaries'
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

const timelineMax = computed(() => {
  const grouped = new Map<string, number>()
  for (const e of props.timeline) {
    grouped.set(e.date, (grouped.get(e.date) ?? 0) + e.sessions)
  }
  return Math.max(...Array.from(grouped.values()), 1)
})

const timelineDates = computed(() => {
  const grouped = new Map<string, { sessions: number; messages: number }>()
  for (const e of props.timeline) {
    const prev = grouped.get(e.date) ?? { sessions: 0, messages: 0 }
    grouped.set(e.date, { sessions: prev.sessions + e.sessions, messages: prev.messages + e.messages })
  }
  return Array.from(grouped.entries())
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([date, v]) => ({ date, ...v }))
})

const projectCount = computed(() => {
  if (!props.breakdown) return 0
  return Object.keys(props.breakdown.by_project).length
})

const adapterEntries = computed(() => {
  if (!props.breakdown) return []
  return Object.entries(props.breakdown.by_adapter).sort((a, b) => b[1] - a[1])
})

const totalSessions = computed(() => props.stats?.sessions ?? 0)

const typeEntries = computed(() => {
  const ai = props.stats?.sessions ?? 0
  return [
    { name: 'AI Session', count: ai, color: '#3b82f6' },
  ]
})
</script>

<template>
  <div class="mb-5 flex items-center justify-between">
    <h2 class="text-xl font-bold tracking-tight">Dashboard</h2>
    <Button variant="outline" size="sm" @click="emit('refresh')" :disabled="refreshing">
      <RefreshCw class="mr-1.5 h-3.5 w-3.5" :class="{ 'animate-spin': refreshing }" />
      Refresh Data
    </Button>
  </div>

  <!-- Stat Cards -->
  <div class="grid grid-cols-4 gap-3">
    <div class="rounded-lg border border-border bg-card p-4">
      <div class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Total Sessions</div>
      <div class="mt-2 text-2xl font-bold">{{ formatNumber(stats?.sessions ?? 0) }}</div>
      <div class="mt-1 text-[11px] text-muted-foreground">AI: {{ formatNumber(stats?.sessions ?? 0) }}</div>
    </div>
    <div class="cursor-pointer rounded-lg border border-border bg-card p-4 transition-colors hover:border-primary/30" @click="emit('navigateProjects')">
      <div class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Projects</div>
      <div class="mt-2 text-2xl font-bold">{{ projectCount }}</div>
      <div class="mt-1 text-[11px] text-muted-foreground">click to view</div>
    </div>
    <div class="rounded-lg border border-border bg-card p-4">
      <div class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Last 7 Days</div>
      <div class="mt-2 text-2xl font-bold">{{ formatNumber(breakdown?.recent_7d_sessions ?? 0) }}</div>
      <div class="mt-1 text-[11px] text-muted-foreground">{{ formatNumber(breakdown?.recent_7d_messages ?? 0) }} messages</div>
    </div>
    <div class="rounded-lg border border-border bg-card p-4">
      <div class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Last 30 Days</div>
      <div class="mt-2 text-2xl font-bold">{{ formatNumber(breakdown?.recent_30d_sessions ?? 0) }}</div>
      <div class="mt-1 text-[11px] text-muted-foreground">{{ formatNumber(breakdown?.recent_30d_messages ?? 0) }} messages</div>
    </div>
  </div>

  <!-- Timeline -->
  <div class="mt-5 rounded-lg border border-border bg-card p-5">
    <h3 class="mb-4 text-sm font-semibold">Daily Activity (last 30 days)</h3>
    <div v-if="timelineDates.length" class="flex h-36 items-end gap-px">
      <div v-for="d in timelineDates" :key="d.date" class="group relative flex flex-1 flex-col items-center">
        <div
          class="w-full rounded-t bg-primary/50 transition-all group-hover:bg-primary"
          :style="{ height: Math.max((d.sessions / timelineMax) * 100, 3) + '%', minHeight: '2px' }"
        />
        <div class="absolute bottom-full z-10 mb-1 hidden whitespace-nowrap rounded border border-border bg-popover px-2 py-1.5 text-[10px] shadow-lg group-hover:block">
          <div class="font-semibold">{{ d.date }}</div>
          <div>{{ d.sessions }} AI sessions</div>
          <div class="text-muted-foreground">{{ d.messages }} messages</div>
        </div>
        <div class="mt-1 origin-top-left -rotate-45 text-[8px] text-muted-foreground opacity-0 group-hover:opacity-100">
          {{ d.date.slice(5) }}
        </div>
      </div>
    </div>
    <div v-else class="flex h-36 items-center justify-center text-xs text-muted-foreground">No activity data</div>
  </div>

  <!-- Breakdown Row -->
  <div v-if="breakdown" class="mt-5 grid grid-cols-2 gap-3">
    <!-- By IDE/Tool -->
    <div class="rounded-lg border border-border bg-card p-4">
      <h4 class="mb-3 text-sm font-semibold">By IDE / Tool</h4>
      <div class="space-y-2.5">
        <div v-for="[name, count] in adapterEntries" :key="name" class="flex items-center gap-2.5 text-xs">
          <span class="h-3 w-3 shrink-0 rounded" :style="{ background: adapterColors[name] ?? '#71717a' }" />
          <span class="w-20 shrink-0 truncate font-medium">{{ adapterLabel(name) }}</span>
          <div class="flex-1">
            <div class="h-2.5 overflow-hidden rounded-full bg-muted">
              <div class="h-full rounded-full transition-all" :style="{ width: (totalSessions > 0 ? count / totalSessions * 100 : 0) + '%', background: adapterColors[name] ?? '#71717a' }" />
            </div>
          </div>
          <span class="w-20 shrink-0 text-right font-mono text-muted-foreground">{{ count }} ({{ totalSessions > 0 ? Math.round(count / totalSessions * 100) : 0 }}%)</span>
        </div>
      </div>
    </div>
    <!-- By Type -->
    <div class="rounded-lg border border-border bg-card p-4">
      <h4 class="mb-3 text-sm font-semibold">By Type</h4>
      <div class="space-y-2.5">
        <div v-for="item in typeEntries" :key="item.name" class="flex items-center gap-2.5 text-xs">
          <span class="h-3 w-3 shrink-0 rounded" :style="{ background: item.color }" />
          <span class="w-20 shrink-0 truncate font-medium">{{ item.name }}</span>
          <div class="flex-1">
            <div class="h-2.5 overflow-hidden rounded-full bg-muted">
              <div class="h-full rounded-full transition-all" :style="{ width: '100%', background: item.color }" />
            </div>
          </div>
          <span class="w-20 shrink-0 text-right font-mono text-muted-foreground">{{ item.count }} (100%)</span>
        </div>
      </div>
    </div>
  </div>

  <!-- Top Projects -->
  <div v-if="breakdown" class="mt-5 rounded-lg border border-border bg-card p-4">
    <h4 class="mb-3 text-sm font-semibold">Top Projects</h4>
    <div class="space-y-2.5">
      <div v-for="[name, count] in Object.entries(breakdown.by_project).sort((a, b) => b[1] - a[1]).slice(0, 10)" :key="name" class="flex items-center gap-2.5 text-xs">
        <span class="h-3 w-3 shrink-0 rounded bg-primary/60" />
        <Tooltip>
          <TooltipTrigger as-child>
            <span class="w-28 shrink-0 truncate font-medium">{{ name.split('/').pop() }}</span>
          </TooltipTrigger>
          <TooltipContent side="top">{{ name }}</TooltipContent>
        </Tooltip>
        <div class="flex-1">
          <div class="h-2.5 overflow-hidden rounded-full bg-muted">
            <div class="h-full rounded-full bg-primary/60 transition-all" :style="{ width: (totalSessions > 0 ? count / totalSessions * 100 : 0) + '%' }" />
          </div>
        </div>
        <span class="w-10 shrink-0 text-right font-mono text-muted-foreground">{{ count }}</span>
      </div>
    </div>
  </div>

  <!-- Recent Sessions -->
  <div v-if="recentSessions.length" class="mt-5">
    <h4 class="mb-3 text-sm font-semibold">Recent Sessions</h4>
    <div class="overflow-hidden rounded-lg border border-border">
      <table class="w-full text-sm">
        <thead>
          <tr class="border-b border-border bg-muted/50">
            <th class="px-4 py-2 text-left text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Project</th>
            <th class="px-4 py-2 text-left text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Tool</th>
            <th class="px-4 py-2 text-left text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Summary</th>
            <th class="px-4 py-2 text-left text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Time</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="s in recentSessions"
            :key="s.id"
            class="cursor-pointer border-b border-border transition-colors hover:bg-accent"
            @click="emit('openSession', s.id)"
          >
            <td class="px-4 py-2 text-xs font-semibold">{{ s.project_path?.split('/').pop() ?? '-' }}</td>
            <td class="px-4 py-2">
              <span class="inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-semibold" :class="[adapterBg(s.source), adapterColor(s.source)]">
                {{ adapterLabel(s.source) }}
              </span>
            </td>
            <td class="max-w-[280px] truncate px-4 py-2 text-xs text-muted-foreground">{{ s.title ?? '-' }}</td>
            <td class="px-4 py-2 text-xs text-muted-foreground">{{ timeAgo(s.updated_at) }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>

  <!-- LLM Status -->
  <div v-if="stats" class="mt-5 rounded-lg border border-border bg-card p-4">
    <div class="mb-2 flex items-center justify-between">
      <h4 class="text-sm font-semibold">LLM Summary</h4>
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
        {{ batchRunning ? 'Generating...' : `Generate Missing (${stats.sessions - stats.summaries})` }}
      </Button>
    </div>
    <div class="flex items-center gap-4 text-xs">
      <div>
        <span class="text-muted-foreground">Provider: </span>
        <span :class="stats.llm_provider ? 'text-primary font-medium' : 'text-warning'">
          {{ stats.llm_provider ?? 'disabled' }}
        </span>
      </div>
      <div>
        <span class="text-muted-foreground">Summaries: </span>
        <span class="font-medium">{{ stats.summaries }}/{{ stats.sessions }}</span>
      </div>
      <div>
        <span class="text-muted-foreground">Chunks: </span>
        <span class="font-medium">{{ formatNumber(stats.chunks) }}</span>
      </div>
      <div v-if="stats.llm_provider" class="ml-auto">
        <div class="h-2.5 w-32 overflow-hidden rounded-full bg-muted">
          <div class="h-full rounded-full bg-primary transition-all" :style="{ width: (stats.sessions > 0 ? stats.summaries / stats.sessions * 100 : 0) + '%' }" />
        </div>
      </div>
    </div>
    <!-- Batch progress -->
    <div v-if="batchProgress" class="mt-3">
      <div class="mb-1 flex items-center justify-between text-xs text-muted-foreground">
        <span>{{ batchProgress.current }}/{{ batchProgress.total }} sessions processed</span>
        <span>{{ Math.round(batchProgress.current / batchProgress.total * 100) }}%</span>
      </div>
      <div class="h-2 overflow-hidden rounded-full bg-muted">
        <div
          class="h-full rounded-full bg-primary transition-all duration-300"
          :style="{ width: (batchProgress.current / batchProgress.total * 100) + '%' }"
        />
      </div>
      <p v-if="batchProgress.done" class="mt-1 text-xs text-green-500">All done!</p>
    </div>
    <p v-if="batchError" class="mt-2 text-xs text-destructive">{{ batchError }}</p>
  </div>
</template>
