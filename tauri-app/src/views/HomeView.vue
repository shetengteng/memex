<script setup lang="ts">
import { ref, inject, onMounted } from 'vue'
import { Search, Settings, Database, MessageSquare, Layers } from 'lucide-vue-next'
import type { Stats, SessionRow, ViewName } from '@/types'
import { useMemex } from '@/composables/useMemex'
import SessionCard from '@/components/SessionCard.vue'

const navigate = inject<(view: ViewName) => void>('navigate')!
const { getStats, listRecent } = useMemex()

const stats = ref<Stats>({ sessions: 0, messages: 0, chunks: 0, db_exists: false })
const sessions = ref<SessionRow[]>([])
const loading = ref(true)

onMounted(async () => {
  try {
    const [s, r] = await Promise.all([getStats(), listRecent(5)])
    stats.value = s
    sessions.value = r
  } catch (e) {
    console.error('Failed to load home data:', e)
  } finally {
    loading.value = false
  }
})
</script>

<template>
  <div class="flex h-full flex-col">
    <!-- Header -->
    <div class="flex h-11 shrink-0 items-center justify-between border-b border-border px-3">
      <div class="flex items-center gap-2">
        <Database class="h-4 w-4 text-primary" />
        <h1 class="text-sm font-semibold">Memex</h1>
      </div>
      <button
        class="inline-flex h-7 w-7 items-center justify-center rounded-md transition-colors hover:bg-accent"
        @click="navigate('settings')"
      >
        <Settings class="h-4 w-4 text-muted-foreground" />
      </button>
    </div>

    <div class="flex-1 overflow-y-auto">
      <!-- Search Entry -->
      <div class="px-3 pt-3">
        <button
          class="flex h-9 w-full items-center gap-2 rounded-lg border border-input bg-background px-3 text-sm text-muted-foreground transition-colors hover:bg-accent"
          @click="navigate('search')"
        >
          <Search class="h-4 w-4" />
          Search Cursor / Claude / Codex history…
        </button>
      </div>

      <!-- Stats -->
      <div class="mt-3 grid grid-cols-3 gap-2 px-3">
        <div class="rounded-lg bg-secondary p-2.5 text-center">
          <div class="flex items-center justify-center gap-1 text-xs text-muted-foreground">
            <Database class="h-3 w-3" />
            Sessions
          </div>
          <p class="mt-1 text-lg font-semibold tabular-nums">{{ stats.sessions }}</p>
        </div>
        <div class="rounded-lg bg-secondary p-2.5 text-center">
          <div class="flex items-center justify-center gap-1 text-xs text-muted-foreground">
            <MessageSquare class="h-3 w-3" />
            Messages
          </div>
          <p class="mt-1 text-lg font-semibold tabular-nums">{{ stats.messages }}</p>
        </div>
        <div class="rounded-lg bg-secondary p-2.5 text-center">
          <div class="flex items-center justify-center gap-1 text-xs text-muted-foreground">
            <Layers class="h-3 w-3" />
            Chunks
          </div>
          <p class="mt-1 text-lg font-semibold tabular-nums">{{ stats.chunks }}</p>
        </div>
      </div>

      <!-- Recent Sessions -->
      <div class="mt-4 px-3">
        <h2 class="mb-1.5 text-xs font-medium uppercase tracking-wider text-muted-foreground">
          Recent Sessions
        </h2>
        <div v-if="loading" class="space-y-2">
          <div v-for="i in 3" :key="i" class="h-14 animate-pulse rounded-lg bg-muted" />
        </div>
        <div v-else-if="sessions.length === 0" class="py-8 text-center text-sm text-muted-foreground">
          No sessions yet. Run <code class="rounded bg-muted px-1 py-0.5 text-xs">memex ingest</code> first.
        </div>
        <div v-else class="space-y-0.5">
          <SessionCard
            v-for="session in sessions"
            :key="session.id"
            :session="session"
          />
        </div>
      </div>
    </div>

    <!-- Footer -->
    <div class="flex shrink-0 items-center justify-between border-t border-border px-3 py-2">
      <span class="text-[10px] text-muted-foreground">
        {{ stats.db_exists ? 'Database ready' : 'No database' }}
      </span>
      <button
        class="text-xs font-medium text-primary transition-colors hover:underline"
        @click="navigate('search')"
      >
        Open Search
      </button>
    </div>
  </div>
</template>
