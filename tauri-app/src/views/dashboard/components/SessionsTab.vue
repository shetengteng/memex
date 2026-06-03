<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { Loader2, ChevronLeft, ChevronRight } from 'lucide-vue-next'
import type { SessionRow } from '@/types'
import { timeAgo, adapterColor, adapterBg, adapterLabel } from '@/lib/utils'
import { useI18n } from '@/i18n'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'

const { t } = useI18n()

const props = defineProps<{
  sessions: SessionRow[]
  loading: boolean
  initialFilter?: string
  initialMessagesFilter?: 'all' | 'invalid' | 'valid'
}>()

const emit = defineEmits<{
  openSession: [sessionId: string]
}>()

const searchQuery = ref(props.initialFilter ?? '')
const filterAdapter = ref<string>('all')
const filterDays = ref<string>('all')
// 这个 filter 主要给「LLM 摘要进度条旁的 X 个无效会话徽章」当跳转目标用：
// invalid = message_count < 2 (拿不到 L2 摘要)，valid = >= 2，all 不过滤
const filterMessages = ref<'all' | 'invalid' | 'valid'>(props.initialMessagesFilter ?? 'all')
const page = ref(1)
const pageSize = 20

watch(() => props.initialFilter, (v) => { if (v) searchQuery.value = v })
watch(() => props.initialMessagesFilter, (v) => { if (v) filterMessages.value = v })

const adapterOptions = computed(() => {
  const set = new Set(props.sessions.map(s => s.source))
  return Array.from(set).sort()
})

const filteredSessions = computed(() => {
  let list = props.sessions

  if (filterAdapter.value !== 'all') {
    list = list.filter(s => s.source === filterAdapter.value)
  }

  if (filterMessages.value === 'invalid') {
    list = list.filter(s => (s.message_count ?? 0) < 2)
  } else if (filterMessages.value === 'valid') {
    list = list.filter(s => (s.message_count ?? 0) >= 2)
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

watch([searchQuery, filterAdapter, filterDays, filterMessages], () => { page.value = 1 })

function formatDate(dateStr: string): string {
  try {
    const d = new Date(dateStr)
    return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })
  } catch { return dateStr }
}

function summaryLine(s: SessionRow): string {
  const c = (s.summary_title ?? s.title ?? s.first_user_message ?? '').trim()
  if (!c) return '—'
  return c.length > 90 ? c.slice(0, 90) + '…' : c
}
</script>

<template>
  <div class="mb-5 flex items-center justify-between">
    <h2 class="text-xl font-bold tracking-tight">{{ t('sessions.title') }}</h2>
  </div>

  <div class="mb-4 flex flex-wrap items-center gap-2">
    <Input
      v-model="searchQuery"
      :placeholder="t('sessions.filter.search_placeholder')"
      class="min-w-[200px] flex-1 text-xs"
    />
    <Select v-model="filterAdapter">
      <SelectTrigger class="w-[140px] text-xs">
        <SelectValue :placeholder="t('sessions.filter.all_tools')" />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="all">{{ t('sessions.filter.all_tools') }}</SelectItem>
        <SelectItem v-for="a in adapterOptions" :key="a" :value="a">{{ adapterLabel(a) }}</SelectItem>
      </SelectContent>
    </Select>
    <Select v-model="filterDays">
      <SelectTrigger class="w-[140px] text-xs">
        <SelectValue :placeholder="t('sessions.filter.all_time')" />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="all">{{ t('sessions.filter.all_time') }}</SelectItem>
        <SelectItem value="1">{{ t('sessions.filter.today') }}</SelectItem>
        <SelectItem value="7">{{ t('sessions.filter.last_7d') }}</SelectItem>
        <SelectItem value="30">{{ t('sessions.filter.last_30d') }}</SelectItem>
        <SelectItem value="90">{{ t('sessions.filter.last_90d') }}</SelectItem>
      </SelectContent>
    </Select>
    <Select v-model="filterMessages">
      <SelectTrigger class="w-[160px] text-xs">
        <SelectValue :placeholder="t('sessions.filter.all_messages')" />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="all">{{ t('sessions.filter.all_messages') }}</SelectItem>
        <SelectItem value="invalid">{{ t('sessions.filter.invalid_only') }}</SelectItem>
        <SelectItem value="valid">{{ t('sessions.filter.valid_only') }}</SelectItem>
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
          <th class="px-4 py-2.5 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{{ t('sessions.table.project') }}</th>
          <th class="px-4 py-2.5 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{{ t('sessions.table.tool') }}</th>
          <th class="px-4 py-2.5 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{{ t('sessions.table.summary') }}</th>
          <th class="px-4 py-2.5 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{{ t('sessions.table.messages') }}</th>
          <th class="px-4 py-2.5 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{{ t('sessions.table.created') }}</th>
          <th class="px-4 py-2.5 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{{ t('sessions.table.updated') }}</th>
        </tr>
      </thead>
      <tbody>
        <tr v-if="pagedSessions.length === 0">
          <td colspan="6" class="px-4 py-10 text-center text-xs text-muted-foreground">{{ t('sessions.empty') }}</td>
        </tr>
        <tr
          v-for="s in pagedSessions"
          :key="s.id"
          class="cursor-pointer border-b border-border transition-colors hover:bg-accent"
          @click="emit('openSession', s.id)"
        >
          <td class="px-4 py-2.5 text-xs font-semibold">{{ s.project_path?.split('/').pop() ?? '-' }}</td>
          <td class="whitespace-nowrap px-4 py-2.5">
            <span class="inline-flex items-center whitespace-nowrap rounded-full px-2 py-0.5 text-xs font-semibold" :class="[adapterBg(s.source), adapterColor(s.source)]">
              {{ adapterLabel(s.source) }}
            </span>
          </td>
          <td class="max-w-[320px] truncate px-4 py-2.5 text-xs text-muted-foreground">{{ summaryLine(s) }}</td>
          <td class="px-4 py-2.5 text-xs">{{ s.message_count }}</td>
          <td class="px-4 py-2.5 text-xs text-muted-foreground">{{ formatDate(s.created_at) }}</td>
          <td class="px-4 py-2.5 text-xs text-muted-foreground">{{ timeAgo(s.updated_at) }}</td>
        </tr>
      </tbody>
    </table>
  </div>

  <div class="mt-3 flex items-center justify-between text-xs text-muted-foreground">
    <span>{{ t('sessions.pagination.range', {
      start: (page - 1) * pageSize + 1,
      end: Math.min(page * pageSize, filteredSessions.length),
      total: filteredSessions.length,
      filtered: (filterAdapter !== 'all' || (filterDays && filterDays !== 'all') || filterMessages !== 'all' || searchQuery.trim()) ? t('sessions.pagination.filtered') : ''
    }) }}</span>
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
