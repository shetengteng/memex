<script setup lang="ts">
import { ref, inject, watch, onMounted } from 'vue'
import { Loader2 } from 'lucide-vue-next'
import type { SearchResult, SessionRow, ViewName } from '@/types'
import { useMemex } from '@/composables/useMemex'
import { adapterAbbr, adapterColor, adapterBg, timeAgo } from '@/lib/utils'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'

const PAGE_SIZE = 20

const props = defineProps<{ query: string }>()
const navigate = inject<(view: ViewName, id?: string) => void>('navigate')!
const { searchMemex, listRecent } = useMemex()

const results = ref<SearchResult[]>([])
const recentSessions = ref<SessionRow[]>([])
const searching = ref(false)
const loadingMore = ref(false)
const hasSearched = ref(false)
const noMoreRecent = ref(false)
const noMoreSearch = ref(false)
const scrollEl = ref<HTMLElement | null>(null)

let debounceTimer: ReturnType<typeof setTimeout> | null = null

onMounted(async () => {
  try {
    const batch = await listRecent(PAGE_SIZE, 0)
    recentSessions.value = batch
    noMoreRecent.value = batch.length < PAGE_SIZE
  } catch { /* ignore */ }
})

watch(() => props.query, (val) => {
  if (debounceTimer) clearTimeout(debounceTimer)
  if (!val.trim()) {
    results.value = []
    hasSearched.value = false
    noMoreSearch.value = false
    return
  }
  debounceTimer = setTimeout(() => doSearch(true), 300)
})

async function doSearch(reset = false) {
  const q = props.query.trim()
  if (!q) return
  if (reset) {
    searching.value = true
    hasSearched.value = true
    results.value = []
    noMoreSearch.value = false
  }
  try {
    const offset = reset ? 0 : results.value.length
    const batch = await searchMemex(q, PAGE_SIZE, offset)
    if (reset) {
      results.value = batch
    } else {
      results.value.push(...batch)
    }
    noMoreSearch.value = batch.length < PAGE_SIZE
  } catch {
    if (reset) results.value = []
  } finally {
    searching.value = false
    loadingMore.value = false
  }
}

async function loadMoreRecent() {
  if (loadingMore.value || noMoreRecent.value) return
  loadingMore.value = true
  try {
    const batch = await listRecent(PAGE_SIZE, recentSessions.value.length)
    recentSessions.value.push(...batch)
    noMoreRecent.value = batch.length < PAGE_SIZE
  } catch { /* ignore */ }
  loadingMore.value = false
}

function onScroll() {
  const el = scrollEl.value
  if (!el || loadingMore.value) return
  const nearBottom = el.scrollTop + el.clientHeight >= el.scrollHeight - 40
  if (!nearBottom) return

  if (hasSearched.value && props.query.trim()) {
    if (!noMoreSearch.value) {
      loadingMore.value = true
      doSearch(false)
    }
  } else {
    loadMoreRecent()
  }
}

function openSession(sessionId: string) {
  navigate('session', sessionId)
}

function sessionLine1(s: SessionRow): string {
  const candidates: Array<string | null | undefined> = [
    s.summary_title,
    s.title,
    s.first_user_message,
  ]
  for (const c of candidates) {
    const trimmed = c?.trim()
    if (trimmed) return trimmed.length > 80 ? trimmed.slice(0, 80) + '…' : trimmed
  }
  return s.id.slice(0, 16)
}

function sessionLine2(s: SessionRow): string {
  const project = s.project_path?.split('/').filter(Boolean).pop()
  const msg = `${s.message_count} msg`
  return project ? `${project} · ${msg}` : msg
}
</script>

<template>
  <div ref="scrollEl" class="h-full overflow-y-auto" @scroll="onScroll">
    <!-- Search Results -->
    <template v-if="hasSearched || query.trim()">
      <div v-if="searching && results.length === 0" class="flex items-center justify-center py-10">
        <Loader2 class="h-4 w-4 animate-spin text-muted-foreground" />
      </div>
      <div v-else-if="results.length === 0 && hasSearched" class="py-10 text-center text-xs text-muted-foreground">
        无结果 "{{ query }}"
      </div>
      <div v-else>
        <button
          v-for="r in results"
          :key="r.chunk_id"
          class="grid w-full grid-cols-[32px_1fr_auto] items-center gap-2.5 px-4 py-2.5 text-left transition-colors hover:bg-accent"
          @click="openSession(r.session_id)"
        >
          <Tooltip>
            <TooltipTrigger as-child>
              <span class="mono grid h-6 w-7 place-items-center rounded text-[10px] font-semibold" :class="[adapterBg(r.adapter ?? ''), adapterColor(r.adapter ?? '')]">
                {{ adapterAbbr(r.adapter ?? '') }}
              </span>
            </TooltipTrigger>
            <TooltipContent side="right">{{ r.adapter }}</TooltipContent>
          </Tooltip>
          <span class="min-w-0">
            <strong class="block truncate text-sm font-semibold">{{ r.content.slice(0, 60) }}</strong>
            <span class="block truncate text-xs text-muted-foreground" v-html="r.snippet" />
          </span>
          <span v-if="r.timestamp" class="mono shrink-0 text-xs text-muted-foreground">{{ timeAgo(r.timestamp) }}</span>
        </button>
        <div v-if="loadingMore" class="flex items-center justify-center py-3">
          <Loader2 class="h-3.5 w-3.5 animate-spin text-muted-foreground" />
        </div>
        <div v-if="noMoreSearch && results.length > 0" class="py-3 text-center text-[10px] text-muted-foreground">
          已加载全部 {{ results.length }} 条结果
        </div>
      </div>
    </template>

    <!-- Recent Sessions (default) -->
    <template v-else>
      <button
        v-for="s in recentSessions"
        :key="s.id"
        class="grid w-full grid-cols-[32px_1fr_auto] items-center gap-2.5 px-4 py-2.5 text-left transition-colors hover:bg-accent"
        @click="openSession(s.id)"
      >
        <span class="mono grid h-6 w-7 place-items-center rounded text-[10px] font-semibold" :class="[adapterBg(s.source), adapterColor(s.source)]">
          {{ adapterAbbr(s.source) }}
        </span>
        <span class="min-w-0">
          <strong class="block truncate text-sm font-semibold">{{ sessionLine1(s) }}</strong>
          <span class="block truncate text-xs text-muted-foreground">{{ sessionLine2(s) }}</span>
        </span>
        <span class="mono shrink-0 text-xs text-muted-foreground">{{ timeAgo(s.updated_at) }}</span>
      </button>
      <div v-if="loadingMore" class="flex items-center justify-center py-3">
        <Loader2 class="h-3.5 w-3.5 animate-spin text-muted-foreground" />
      </div>
      <div v-if="noMoreRecent && recentSessions.length > 0" class="py-3 text-center text-[10px] text-muted-foreground">
        已加载全部 {{ recentSessions.length }} 个 session
      </div>
      <div v-if="recentSessions.length === 0 && !loadingMore" class="py-10 text-center text-xs text-muted-foreground">
        暂无 session，运行 <code class="mono rounded bg-muted px-1 py-0.5 text-[11px]">memex ingest</code> 开始采集
      </div>
    </template>
  </div>
</template>
