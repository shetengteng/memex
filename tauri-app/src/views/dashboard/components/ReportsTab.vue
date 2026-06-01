<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useMemex } from '@/composables/useMemex'
import { Separator } from '@/components/ui/separator'
import { Button } from '@/components/ui/button'
import { RefreshCw } from 'lucide-vue-next'
import type { AggregateSummary } from '@/types'

const { listReports } = useMemex()

const scope = ref<'daily' | 'weekly'>('daily')
const items = ref<AggregateSummary[]>([])
const selectedKey = ref<string | null>(null)
const loading = ref(false)

async function load() {
  loading.value = true
  try {
    items.value = await listReports(scope.value, 60)
    if (items.value.length && !items.value.find((i) => i.scope_key === selectedKey.value)) {
      selectedKey.value = items.value[0].scope_key
    } else if (!items.value.length) {
      selectedKey.value = null
    }
  } catch {
    items.value = []
    selectedKey.value = null
  } finally {
    loading.value = false
  }
}

onMounted(load)
watch(scope, () => {
  selectedKey.value = null
  load()
})

const current = computed(() => items.value.find((i) => i.scope_key === selectedKey.value) || null)

function formatLabel(r: AggregateSummary): string {
  if (r.scope_type === 'daily') return r.scope_key.replace(/^daily:/, '')
  if (r.scope_type === 'weekly') return r.scope_key.replace(/^weekly:/, '')
  return r.scope_key
}

function formatCreatedAt(iso: string): string {
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return iso
  return d.toLocaleString()
}
</script>

<template>
  <div>
    <header class="mb-6 flex items-baseline justify-between">
      <div>
        <h2 class="text-xl font-semibold">Reports</h2>
        <p class="mt-1 text-xs text-muted-foreground">
          Automatic daily and weekly digests built from L2 session summaries.
        </p>
      </div>
      <Button variant="ghost" size="sm" :disabled="loading" @click="load">
        <RefreshCw class="mr-1.5 h-3.5 w-3.5" :class="{ 'animate-spin': loading }" />
        Refresh
      </Button>
    </header>

    <div class="mb-5 inline-flex rounded-md border border-border p-0.5">
      <button
        v-for="s in (['daily', 'weekly'] as const)"
        :key="s"
        class="px-3 py-1 text-xs font-medium capitalize transition-colors"
        :class="scope === s ? 'rounded bg-muted text-foreground' : 'text-muted-foreground hover:text-foreground'"
        @click="scope = s"
      >{{ s }}</button>
    </div>

    <div v-if="loading && !items.length" class="text-sm text-muted-foreground">Loading…</div>

    <div v-else-if="!items.length" class="rounded-md border border-dashed border-border px-4 py-8 text-center">
      <p class="text-sm font-medium">No {{ scope }} reports yet</p>
      <p class="mx-auto mt-2 max-w-md text-xs text-muted-foreground">
        Reports are generated when an LLM provider is active and there are at least
        {{ scope === 'daily' ? 2 : 3 }} sessions in the {{ scope === 'daily' ? 'current day' : 'current ISO week' }}.
        Enable Ollama in <em>Settings</em> or set a Claude API key, then run <code>memex ingest</code>.
      </p>
    </div>

    <div v-else class="grid grid-cols-[220px_1fr] gap-6">
      <aside>
        <ul class="space-y-0.5">
          <li v-for="r in items" :key="r.scope_key">
            <button
              class="flex w-full items-baseline justify-between rounded-md px-3 py-2 text-left transition-colors"
              :class="r.scope_key === selectedKey ? 'bg-primary/10 text-primary' : 'text-muted-foreground hover:bg-accent hover:text-foreground'"
              @click="selectedKey = r.scope_key"
            >
              <span class="text-sm font-medium tabular-nums">{{ formatLabel(r) }}</span>
              <span class="text-xs text-muted-foreground">{{ r.session_count }}</span>
            </button>
          </li>
        </ul>
      </aside>

      <article v-if="current">
        <header class="mb-3">
          <h3 class="text-lg font-semibold">{{ current.title || formatLabel(current) }}</h3>
          <p class="mt-1 text-xs text-muted-foreground">
            {{ current.session_count }} session{{ current.session_count === 1 ? '' : 's' }} ·
            generated {{ formatCreatedAt(current.created_at) }}
          </p>
        </header>

        <p class="text-sm leading-relaxed whitespace-pre-line">{{ current.summary }}</p>

        <template v-if="current.topics.length">
          <Separator class="my-5" />
          <div class="mb-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">Topics</div>
          <div class="flex flex-wrap gap-1.5">
            <span
              v-for="t in current.topics"
              :key="t"
              class="rounded-md bg-muted px-2 py-0.5 text-xs text-foreground"
            >{{ t }}</span>
          </div>
        </template>

        <template v-if="current.decisions.length">
          <Separator class="my-5" />
          <div class="mb-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">Decisions</div>
          <ul class="space-y-1.5">
            <li
              v-for="d in current.decisions"
              :key="d"
              class="flex gap-2 text-sm"
            >
              <span class="mt-1 h-1.5 w-1.5 shrink-0 rounded-full bg-primary" />
              <span class="leading-relaxed">{{ d }}</span>
            </li>
          </ul>
        </template>
      </article>
    </div>
  </div>
</template>
