<script setup lang="ts">
import { ref, watch } from 'vue'
import { Search, X, Loader2 } from 'lucide-vue-next'
import type { SearchResult } from '@/types'
import { useMemex } from '@/composables/useMemex'
import SearchResultItem from '@/components/SearchResultItem.vue'
import ViewHeader from '@/components/ViewHeader.vue'

const { searchMemex } = useMemex()

const query = ref('')
const results = ref<SearchResult[]>([])
const searching = ref(false)
const searched = ref(false)

const activeFilter = ref<string>('all')
const filters = ['all', 'claude_code', 'cursor', 'codex', 'opencode'] as const

let debounceTimer: ReturnType<typeof setTimeout> | null = null

watch(query, (val) => {
  if (debounceTimer) clearTimeout(debounceTimer)
  if (!val.trim()) {
    results.value = []
    searched.value = false
    return
  }
  debounceTimer = setTimeout(() => doSearch(), 300)
})

async function doSearch() {
  const q = query.value.trim()
  if (!q) return
  searching.value = true
  searched.value = true
  try {
    const raw = await searchMemex(q, 20)
    results.value = activeFilter.value === 'all'
      ? raw
      : raw.filter((r) => r.session_id.startsWith(activeFilter.value))
  } catch (e) {
    console.error('Search failed:', e)
    results.value = []
  } finally {
    searching.value = false
  }
}

function clearQuery() {
  query.value = ''
  results.value = []
  searched.value = false
}

function setFilter(f: string) {
  activeFilter.value = f
  if (query.value.trim()) doSearch()
}

const filterLabel: Record<string, string> = {
  all: 'All',
  claude_code: 'Claude',
  cursor: 'Cursor',
  codex: 'Codex',
  opencode: 'OpenCode',
}
</script>

<template>
  <div class="flex h-full flex-col">
    <ViewHeader title="Search Memories" show-back />

    <!-- Search Input -->
    <div class="border-b border-border px-3 py-2">
      <div class="relative">
        <Search class="absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
        <input
          ref="searchInput"
          v-model="query"
          type="text"
          placeholder="Search across all sessions…"
          class="h-9 w-full rounded-lg border border-input bg-background pl-9 pr-8 text-sm outline-none transition-colors focus:ring-1 focus:ring-ring"
          autofocus
        />
        <button
          v-if="query"
          class="absolute right-2 top-1/2 -translate-y-1/2 rounded p-0.5 hover:bg-accent"
          @click="clearQuery"
        >
          <X class="h-3.5 w-3.5 text-muted-foreground" />
        </button>
      </div>

      <!-- Filters -->
      <div class="mt-2 flex gap-1.5">
        <button
          v-for="f in filters"
          :key="f"
          :class="[
            'rounded-md px-2 py-1 text-xs font-medium transition-colors',
            activeFilter === f
              ? 'bg-primary text-primary-foreground'
              : 'bg-secondary text-secondary-foreground hover:bg-accent',
          ]"
          @click="setFilter(f)"
        >
          {{ filterLabel[f] }}
        </button>
      </div>
    </div>

    <!-- Results -->
    <div class="flex-1 overflow-y-auto px-1">
      <div v-if="searching" class="flex items-center justify-center py-12">
        <Loader2 class="h-5 w-5 animate-spin text-muted-foreground" />
      </div>

      <div v-else-if="searched && results.length === 0" class="px-3 py-12 text-center">
        <p class="text-sm text-muted-foreground">No results for "{{ query }}"</p>
        <p class="mt-1 text-xs text-muted-foreground">Try different keywords or broaden filters.</p>
      </div>

      <div v-else class="space-y-0.5 py-1">
        <SearchResultItem
          v-for="(r, i) in results"
          :key="r.chunk_id"
          :result="r"
          :index="i"
        />
      </div>
    </div>

    <!-- Status bar -->
    <div class="flex shrink-0 items-center justify-between border-t border-border px-3 py-1.5">
      <span class="text-[10px] text-muted-foreground">
        {{ searched ? `${results.length} result(s)` : 'Type to search' }}
      </span>
      <span class="text-[10px] text-muted-foreground">
        ↵ Open · Esc Back
      </span>
    </div>
  </div>
</template>
