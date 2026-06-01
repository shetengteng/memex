<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { Loader2, ChevronLeft, ChevronRight } from 'lucide-vue-next'
import type { SessionRow } from '@/types'
import { timeAgo, adapterColor, adapterBg, adapterLabel } from '@/lib/utils'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'

const props = defineProps<{
  sessions: SessionRow[]
  loading: boolean
  initialFilter?: string
}>()

const emit = defineEmits<{
  openSession: [sessionId: string]
}>()

const searchQuery = ref(props.initialFilter ?? '')
const filterAdapter = ref<string>('all')
const filterDays = ref<string>('all')
const page = ref(1)
const pageSize = 20

watch(() => props.initialFilter, (v) => { if (v) searchQuery.value = v })

const adapterOptions = computed(() => {
  const set = new Set(props.sessions.map(s => s.source))
  return Array.from(set).sort()
})

const filteredSessions = computed(() => {
  let list = props.sessions

  if (filterAdapter.value !== 'all') {
    list = list.filter(s => s.source === filterAdapter.value)
  }

  if (filterDays.value && filterDays.value !== 'all') {
    const days = parseInt(filterDays.value)
    const cutoff = new Date()
    cutoff.setDate(cutoff.getDate() - days)
    list = list.filter(s => new Date(s.updated_at) >= cutoff)
  }

  if (searchQuery.value.trim()) {
    const q = searchQuery.value.trim().toLowerCase()
    list = list.filter(s =>
      (s.project_path?.toLowerCase().includes(q)) ||
      (s.title?.toLowerCase().includes(q)) ||
      s.source.toLowerCase().includes(q) ||
      s.id.toLowerCase().includes(q),
    )
  }

  return list
})

const totalPages = computed(() => Math.max(1, Math.ceil(filteredSessions.value.length / pageSize)))
const pagedSessions = computed(() => {
  const start = (page.value - 1) * pageSize
  return filteredSessions.value.slice(start, start + pageSize)
})

watch([searchQuery, filterAdapter, filterDays], () => { page.value = 1 })

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
  </div>

  <div class="mb-4 flex flex-wrap items-center gap-2">
    <Input
      v-model="searchQuery"
      placeholder="Search sessions..."
      class="min-w-[200px] flex-1 text-xs"
    />
    <Select v-model="filterAdapter">
      <SelectTrigger class="w-[140px] text-xs">
        <SelectValue placeholder="All Tools" />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="all">All Tools</SelectItem>
        <SelectItem v-for="a in adapterOptions" :key="a" :value="a">{{ adapterLabel(a) }}</SelectItem>
      </SelectContent>
    </Select>
    <Select v-model="filterDays">
      <SelectTrigger class="w-[140px] text-xs">
        <SelectValue placeholder="All Time" />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="all">All Time</SelectItem>
        <SelectItem value="1">Today</SelectItem>
        <SelectItem value="7">Last 7 days</SelectItem>
        <SelectItem value="30">Last 30 days</SelectItem>
        <SelectItem value="90">Last 90 days</SelectItem>
      </SelectContent>
    </Select>
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
        <tr v-if="pagedSessions.length === 0">
          <td colspan="6" class="px-4 py-10 text-center text-xs text-muted-foreground">No sessions found</td>
        </tr>
        <tr
          v-for="s in pagedSessions"
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

  <div class="mt-3 flex items-center justify-between text-xs text-muted-foreground">
    <span>
      Showing {{ (page - 1) * pageSize + 1 }}-{{ Math.min(page * pageSize, filteredSessions.length) }}
      of {{ filteredSessions.length }}{{ filterAdapter !== 'all' || (filterDays && filterDays !== 'all') || searchQuery.trim() ? ' (filtered)' : '' }}
    </span>
    <div v-if="totalPages > 1" class="flex items-center gap-1">
      <Button variant="ghost" size="sm" :disabled="page <= 1" @click="page--">
        <ChevronLeft class="h-4 w-4" />
      </Button>
      <span class="px-2">{{ page }} / {{ totalPages }}</span>
      <Button variant="ghost" size="sm" :disabled="page >= totalPages" @click="page++">
        <ChevronRight class="h-4 w-4" />
      </Button>
    </div>
  </div>
</template>
