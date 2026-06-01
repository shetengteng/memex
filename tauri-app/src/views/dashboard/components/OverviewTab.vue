<script setup lang="ts">
import { computed } from 'vue'
import { RefreshCw } from 'lucide-vue-next'
import type { Stats, StatsBreakdown, TimelineEntry } from '@/types'
import { formatNumber, adapterLabel } from '@/lib/utils'
import { Button } from '@/components/ui/button'

const props = defineProps<{
  stats: Stats | null
  breakdown: StatsBreakdown | null
  timeline: TimelineEntry[]
  refreshing?: boolean
}>()

const emit = defineEmits<{
  refresh: []
  navigateProjects: []
}>()

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

  <!-- Breakdown -->
  <div v-if="breakdown" class="mt-5 grid grid-cols-2 gap-3">
    <!-- By IDE/Tool -->
    <div class="rounded-lg border border-border bg-card p-4">
      <h4 class="mb-3 text-sm font-semibold">By IDE / Tool</h4>
      <div class="space-y-2.5">
        <div v-for="[name, count] in adapterEntries" :key="name" class="flex items-center gap-2.5 text-xs">
          <span class="h-3 w-3 shrink-0 rounded" :style="{ background: adapterColors[name] ?? '#71717a' }" />
          <span class="min-w-0 truncate font-medium">{{ adapterLabel(name) }}</span>
          <div class="flex-1">
            <div class="h-2 overflow-hidden rounded-full bg-muted">
              <div class="h-full rounded-full transition-all" :style="{ width: (count / totalSessions * 100) + '%', background: adapterColors[name] ?? '#71717a' }" />
            </div>
          </div>
          <span class="shrink-0 font-mono text-muted-foreground">{{ count }} ({{ Math.round(count / totalSessions * 100) }}%)</span>
        </div>
      </div>
    </div>
    <!-- Top Projects -->
    <div class="rounded-lg border border-border bg-card p-4">
      <h4 class="mb-3 text-sm font-semibold">Top Projects</h4>
      <div class="space-y-2.5">
        <div v-for="[name, count] in Object.entries(breakdown.by_project).sort((a, b) => b[1] - a[1]).slice(0, 10)" :key="name" class="flex items-center gap-2.5 text-xs">
          <span class="h-3 w-3 shrink-0 rounded bg-primary/60" />
          <span class="min-w-0 truncate font-medium" :title="name">{{ name.split('/').pop() }}</span>
          <div class="flex-1">
            <div class="h-2 overflow-hidden rounded-full bg-muted">
              <div class="h-full rounded-full bg-primary/60 transition-all" :style="{ width: (count / totalSessions * 100) + '%' }" />
            </div>
          </div>
          <span class="shrink-0 font-mono text-muted-foreground">{{ count }}</span>
        </div>
      </div>
    </div>
  </div>

  <!-- LLM Status -->
  <div v-if="stats" class="mt-5 rounded-lg border border-border bg-card p-4">
    <h4 class="mb-2 text-sm font-semibold">LLM Summary</h4>
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
        <div class="h-2 w-32 overflow-hidden rounded-full bg-muted">
          <div class="h-full rounded-full bg-primary transition-all" :style="{ width: (stats.sessions > 0 ? stats.summaries / stats.sessions * 100 : 0) + '%' }" />
        </div>
      </div>
    </div>
  </div>
</template>
