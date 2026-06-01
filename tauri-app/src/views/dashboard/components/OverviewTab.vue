<script setup lang="ts">
import { computed } from 'vue'
import type { Stats, StatsBreakdown, TimelineEntry } from '@/types'
import { formatNumber, adapterLabel } from '@/lib/utils'

const props = defineProps<{
  stats: Stats | null
  breakdown: StatsBreakdown | null
  timeline: TimelineEntry[]
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
</script>

<template>
  <h2 class="mb-5 text-xl font-bold tracking-tight">Dashboard</h2>

  <div class="grid grid-cols-4 gap-3">
    <div class="rounded-lg border border-border bg-card p-4">
      <div class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Sessions</div>
      <div class="mt-2 text-2xl font-bold">{{ formatNumber(stats?.sessions ?? 0) }}</div>
    </div>
    <div class="rounded-lg border border-border bg-card p-4">
      <div class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Messages</div>
      <div class="mt-2 text-2xl font-bold">{{ formatNumber(stats?.messages ?? 0) }}</div>
    </div>
    <div class="rounded-lg border border-border bg-card p-4">
      <div class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Chunks</div>
      <div class="mt-2 text-2xl font-bold">{{ formatNumber(stats?.chunks ?? 0) }}</div>
    </div>
    <div class="rounded-lg border border-border bg-card p-4">
      <div class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">LLM Summaries</div>
      <div class="mt-2 text-2xl font-bold text-primary">{{ stats?.summaries ?? 0 }}<span class="text-sm text-muted-foreground">/{{ stats?.sessions ?? 0 }}</span></div>
      <div v-if="stats?.llm_provider" class="mt-1 text-[11px] text-muted-foreground">
        <span class="mr-1 inline-block h-1.5 w-1.5 rounded-full bg-green-500" />{{ stats.llm_provider }}
      </div>
    </div>
  </div>

  <div class="mt-5 rounded-lg border border-border bg-card p-5">
    <h3 class="mb-4 text-sm font-semibold">Daily Activity (last 30 days)</h3>
    <div v-if="timelineDates.length" class="flex h-32 items-end gap-px">
      <div v-for="d in timelineDates" :key="d.date" class="group relative flex flex-1 flex-col items-center">
        <div
          class="w-full rounded-t bg-primary/50 transition-all group-hover:bg-primary"
          :style="{ height: Math.max((d.sessions / timelineMax) * 100, 3) + '%', minHeight: '2px' }"
        />
        <div class="absolute bottom-full mb-1 hidden rounded border border-border bg-popover px-2 py-1 text-[10px] shadow-lg group-hover:block">
          {{ d.date.slice(5) }} · {{ d.sessions }} sessions
        </div>
      </div>
    </div>
    <div v-else class="flex h-32 items-center justify-center text-xs text-muted-foreground">No data</div>
  </div>

  <div v-if="breakdown" class="mt-5 grid grid-cols-3 gap-3">
    <div class="rounded-lg border border-border bg-card p-4">
      <h4 class="mb-3 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">By Source</h4>
      <div class="space-y-2">
        <div v-for="[name, count] in Object.entries(breakdown.by_adapter).sort((a, b) => b[1] - a[1])" :key="name" class="flex items-center gap-2 text-xs">
          <span class="h-2 w-2 shrink-0 rounded-full" :style="{ background: adapterColors[name] ?? '#71717a' }" />
          <span class="min-w-0 truncate">{{ adapterLabel(name) }}</span>
          <div class="flex-1">
            <div class="h-1 overflow-hidden rounded-full bg-muted">
              <div class="h-full rounded-full" :style="{ width: (count / (stats?.sessions ?? 1) * 100) + '%', background: adapterColors[name] ?? '#71717a' }" />
            </div>
          </div>
          <span class="shrink-0 text-muted-foreground">{{ count }}</span>
        </div>
      </div>
    </div>
    <div class="rounded-lg border border-border bg-card p-4">
      <h4 class="mb-3 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Top Projects</h4>
      <div class="space-y-2">
        <div v-for="[name, count] in Object.entries(breakdown.by_project).sort((a, b) => b[1] - a[1]).slice(0, 8)" :key="name" class="flex items-center gap-2 text-xs">
          <span class="h-2 w-2 shrink-0 rounded-full bg-primary" />
          <span class="min-w-0 truncate" :title="name">{{ name.split('/').pop() }}</span>
          <span class="ml-auto shrink-0 text-muted-foreground">{{ count }}</span>
        </div>
      </div>
    </div>
    <div class="rounded-lg border border-border bg-card p-4">
      <h4 class="mb-3 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Recent Activity</h4>
      <div class="space-y-3">
        <div>
          <div class="text-[11px] text-muted-foreground">Last 7 Days</div>
          <div class="text-xl font-bold">{{ formatNumber(breakdown.recent_7d_sessions) }} <span class="text-xs font-normal text-muted-foreground">sessions</span></div>
          <div class="text-[11px] text-muted-foreground">{{ formatNumber(breakdown.recent_7d_messages) }} messages</div>
        </div>
        <div>
          <div class="text-[11px] text-muted-foreground">Last 30 Days</div>
          <div class="text-xl font-bold">{{ formatNumber(breakdown.recent_30d_sessions) }} <span class="text-xs font-normal text-muted-foreground">sessions</span></div>
          <div class="text-[11px] text-muted-foreground">{{ formatNumber(breakdown.recent_30d_messages) }} messages</div>
        </div>
      </div>
    </div>
  </div>
</template>
