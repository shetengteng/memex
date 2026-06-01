<script setup lang="ts">
import { ref, computed } from 'vue'
import { Loader2 } from 'lucide-vue-next'
import type { SessionRow } from '@/types'
import { timeAgo, adapterColor, adapterBg, adapterLabel } from '@/lib/utils'

const props = defineProps<{
  sessions: SessionRow[]
  loading: boolean
}>()

const emit = defineEmits<{
  openSession: [sessionId: string]
}>()

const filterAdapter = ref<string>('all')

const adapterOptions = computed(() => {
  const set = new Set(props.sessions.map(s => s.source))
  return Array.from(set).sort()
})

const filteredSessions = computed(() => {
  if (filterAdapter.value === 'all') return props.sessions
  return props.sessions.filter(s => s.source === filterAdapter.value)
})

function formatDate(dateStr: string): string {
  try {
    const d = new Date(dateStr)
    return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })
  } catch { return dateStr }
}
</script>

<template>
  <div class="mb-5 flex items-center justify-between">
    <h2 class="text-xl font-bold tracking-tight">Sessions</h2>
    <div class="flex items-center gap-2">
      <span class="text-xs text-muted-foreground">Filter:</span>
      <select
        v-model="filterAdapter"
        class="rounded-md border border-border bg-card px-2.5 py-1.5 text-xs outline-none focus:ring-1 focus:ring-primary"
      >
        <option value="all">All Sources</option>
        <option v-for="a in adapterOptions" :key="a" :value="a">{{ adapterLabel(a) }}</option>
      </select>
    </div>
  </div>

  <div v-if="loading" class="flex items-center justify-center py-10">
    <Loader2 class="h-5 w-5 animate-spin text-muted-foreground" />
  </div>
  <div v-else class="overflow-hidden rounded-lg border border-border">
    <table class="w-full text-sm">
      <thead>
        <tr class="border-b border-border bg-muted/50">
          <th class="px-4 py-2.5 text-left text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Project</th>
          <th class="px-4 py-2.5 text-left text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Tool</th>
          <th class="px-4 py-2.5 text-left text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Summary</th>
          <th class="px-4 py-2.5 text-left text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Messages</th>
          <th class="px-4 py-2.5 text-left text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Created</th>
          <th class="px-4 py-2.5 text-left text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Updated</th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="s in filteredSessions"
          :key="s.id"
          class="cursor-pointer border-b border-border transition-colors hover:bg-accent"
          @click="emit('openSession', s.id)"
        >
          <td class="px-4 py-2.5 text-xs font-semibold">{{ s.project_path?.split('/').pop() ?? '-' }}</td>
          <td class="px-4 py-2.5">
            <span class="inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-semibold" :class="[adapterBg(s.source), adapterColor(s.source)]">
              {{ adapterLabel(s.source) }}
            </span>
          </td>
          <td class="max-w-[280px] truncate px-4 py-2.5 text-xs text-muted-foreground">{{ s.title ?? '-' }}</td>
          <td class="px-4 py-2.5 text-xs">{{ s.message_count }}</td>
          <td class="px-4 py-2.5 text-xs text-muted-foreground">{{ formatDate(s.created_at) }}</td>
          <td class="px-4 py-2.5 text-xs text-muted-foreground">{{ timeAgo(s.updated_at) }}</td>
        </tr>
      </tbody>
    </table>
  </div>
  <div class="mt-3 text-center text-xs text-muted-foreground">
    Showing {{ filteredSessions.length }}{{ filterAdapter !== 'all' ? ` (filtered from ${sessions.length})` : '' }} sessions
  </div>
</template>
