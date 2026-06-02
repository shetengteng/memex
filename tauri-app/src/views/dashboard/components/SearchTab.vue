<script setup lang="ts">
import { ref } from 'vue'
import { Search, Loader2 } from 'lucide-vue-next'
import type { SearchResult } from '@/types'
import { useMemex } from '@/composables/useMemex'
import { useI18n } from '@/i18n'
import { adapterColor, adapterBg, adapterLabel } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'

const { t } = useI18n()

const emit = defineEmits<{
  openSession: [sessionId: string]
}>()

const { searchMemex } = useMemex()
const searchQuery = ref('')
const searchResults = ref<SearchResult[]>([])
const searching = ref(false)
const searchDone = ref(false)

async function doSearch() {
  const q = searchQuery.value.trim()
  if (!q) return
  searching.value = true
  searchDone.value = false
  try { searchResults.value = await searchMemex(q, 50, 0) } catch { searchResults.value = [] }
  searching.value = false
  searchDone.value = true
}
</script>

<template>
  <h2 class="mb-5 text-xl font-bold tracking-tight">{{ t('dashboard.search.title') }}</h2>
  <div class="flex gap-2">
    <Input
      v-model="searchQuery"
      :placeholder="t('dashboard.search.placeholder')"
      class="flex-1"
      @keydown.enter="doSearch"
    />
    <Button @click="doSearch" :disabled="searching">
      <Search class="mr-1.5 h-3.5 w-3.5" />
      {{ t('dashboard.search.button') }}
    </Button>
  </div>
  <div class="mt-4">
    <div v-if="searching" class="flex items-center justify-center py-10">
      <Loader2 class="h-5 w-5 animate-spin text-muted-foreground" />
    </div>
    <div v-else-if="searchDone && searchResults.length === 0" class="py-10 text-center text-sm text-muted-foreground">
      {{ t('dashboard.search.no_results', { query: searchQuery }) }}
    </div>
    <template v-else-if="searchResults.length > 0">
      <div class="mb-3 text-xs text-muted-foreground">{{ t('dashboard.search.results_count', { count: searchResults.length, query: searchQuery }) }}</div>
      <div class="space-y-2">
        <Button
          v-for="r in searchResults"
          :key="r.chunk_id"
          variant="outline"
          class="block h-auto w-full items-start whitespace-normal rounded-lg border-border bg-card p-4 text-left transition-all hover:border-primary/30 hover:shadow-sm"
          @click="emit('openSession', r.session_id)"
        >
          <div class="mb-2 flex items-center gap-2 text-xs">
            <span class="inline-flex items-center whitespace-nowrap rounded-full px-2 py-0.5 text-xs font-semibold" :class="[adapterBg(r.adapter ?? ''), adapterColor(r.adapter ?? '')]">
              {{ adapterLabel(r.adapter ?? '') }}
            </span>
            <span class="truncate font-medium">{{ r.project?.split('/').pop() ?? '-' }}</span>
            <span class="ml-auto text-muted-foreground">{{ r.chunk_type }}</span>
          </div>
          <div class="line-clamp-2 text-xs font-normal text-muted-foreground" v-html="r.snippet" />
          <div class="mt-2 text-[10px] font-normal text-muted-foreground">{{ t('dashboard.search.session_id', { id: r.session_id.slice(0, 12) }) }}</div>
        </Button>
      </div>
    </template>
    <div v-else class="py-10 text-center text-sm text-muted-foreground">
      {{ t('dashboard.search.empty_hint') }}
    </div>
  </div>
</template>
