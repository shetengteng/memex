<script setup lang="ts">
import { ref, inject, watch, onMounted } from 'vue'
import { Loader2 } from 'lucide-vue-next'
import type { SearchResult, SessionRow, ViewName } from '@/types'
import { useMemex } from '@/composables/useMemex'
import { adapterAbbr, adapterColor, adapterBg, timeAgo } from '@/lib/utils'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'

const props = defineProps<{ query: string }>()
const navigate = inject<(view: ViewName, id?: string) => void>('navigate')!
const { searchMemex, listRecent } = useMemex()

const results = ref<SearchResult[]>([])
const recentSessions = ref<SessionRow[]>([])
const searching = ref(false)
const hasSearched = ref(false)

let debounceTimer: ReturnType<typeof setTimeout> | null = null

onMounted(async () => {
  try {
    recentSessions.value = await listRecent(8)
  } catch { /* ignore */ }
})

watch(() => props.query, (val) => {
  if (debounceTimer) clearTimeout(debounceTimer)
  if (!val.trim()) {
    results.value = []
    hasSearched.value = false
    return
  }
  debounceTimer = setTimeout(() => doSearch(), 300)
})

async function doSearch() {
  const q = props.query.trim()
  if (!q) return
  searching.value = true
  hasSearched.value = true
  try {
    results.value = await searchMemex(q, 20)
  } catch {
    results.value = []
  } finally {
    searching.value = false
  }
}

function openSession(sessionId: string) {
  navigate('session', sessionId)
}
</script>

<template>
  <div class="h-full overflow-y-auto">
    <!-- Search Results -->
    <template v-if="hasSearched || query.trim()">
      <div v-if="searching" class="flex items-center justify-center py-10">
        <Loader2 class="h-4 w-4 animate-spin text-muted-foreground" />
      </div>
      <div v-else-if="results.length === 0 && hasSearched" class="py-10 text-center text-xs text-muted-foreground">
        无结果 "{{ query }}"
      </div>
      <div v-else>
        <button
          v-for="r in results"
          :key="r.chunk_id"
          class="grid w-full grid-cols-[26px_1fr_auto] items-center gap-2 px-3.5 py-[7px] text-left transition-colors hover:bg-accent"
          @click="openSession(r.session_id)"
        >
          <Tooltip>
            <TooltipTrigger as-child>
              <span class="mono grid h-5 w-[22px] place-items-center rounded text-[9px] font-semibold" :class="[adapterBg(r.adapter ?? ''), adapterColor(r.adapter ?? '')]">
                {{ adapterAbbr(r.adapter ?? '') }}
              </span>
            </TooltipTrigger>
            <TooltipContent side="right">{{ r.adapter }}</TooltipContent>
          </Tooltip>
          <span class="min-w-0">
            <strong class="block truncate text-xs font-semibold">{{ r.content.slice(0, 60) }}</strong>
            <span class="block truncate text-[11px] text-muted-foreground" v-html="r.snippet" />
          </span>
          <span v-if="r.timestamp" class="mono shrink-0 text-[10px] text-muted-foreground">{{ timeAgo(r.timestamp) }}</span>
        </button>
      </div>
    </template>

    <!-- Recent Sessions (default) -->
    <template v-else>
      <button
        v-for="s in recentSessions"
        :key="s.id"
        class="grid w-full grid-cols-[26px_1fr_auto] items-center gap-2 px-3.5 py-[7px] text-left transition-colors hover:bg-accent"
        @click="openSession(s.id)"
      >
        <span class="mono grid h-5 w-[22px] place-items-center rounded text-[9px] font-semibold" :class="[adapterBg(s.source), adapterColor(s.source)]">
          {{ adapterAbbr(s.source) }}
        </span>
        <span class="min-w-0">
          <strong class="block truncate text-xs font-semibold">{{ s.project_path?.split('/').pop() ?? s.id.slice(0, 16) }}</strong>
          <span class="block truncate text-[11px] text-muted-foreground">{{ s.message_count }} messages · {{ adapterAbbr(s.source) }}</span>
        </span>
        <span class="mono shrink-0 text-[10px] text-muted-foreground">{{ timeAgo(s.updated_at) }}</span>
      </button>
      <div v-if="recentSessions.length === 0" class="py-10 text-center text-xs text-muted-foreground">
        暂无 session，运行 <code class="mono rounded bg-muted px-1 py-0.5 text-[11px]">memex ingest</code> 开始采集
      </div>
    </template>
  </div>
</template>
