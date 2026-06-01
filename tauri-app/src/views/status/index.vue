<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useMemex } from '@/composables/useMemex'
import { formatNumber } from '@/lib/utils'
import { Separator } from '@/components/ui/separator'
import { Button } from '@/components/ui/button'
import { RefreshCw } from 'lucide-vue-next'
import type { Stats } from '@/types'

const { getStats, getConfig } = useMemex()

type Tone = 'success' | 'warning' | 'error' | 'muted'
interface Signal { label: string; value: string; hint?: string; tone: Tone }

const stats = ref<Stats>({
  sessions: 0,
  messages: 0,
  chunks: 0,
  db_exists: false,
  summaries: 0,
  chunks_summarized: 0,
  llm_provider: null,
})
const loading = ref(true)
const daemonOk = ref<boolean | null>(null)
const daemonLatency = ref<number | null>(null)

const adapterDefs: { key: string; label: string }[] = [
  { key: 'claude_code', label: 'Claude Code' },
  { key: 'cursor', label: 'Cursor' },
  { key: 'codex', label: 'Codex' },
  { key: 'opencode', label: 'OpenCode' },
]

const adapterEnabled = ref<Record<string, boolean>>({})
const llmOllama = ref<boolean>(false)
const llmCloud = ref<boolean>(false)

async function probeDaemon() {
  const t0 = performance.now()
  try {
    const resp = await fetch('http://127.0.0.1:9999/health', {
      signal: AbortSignal.timeout(1500),
    })
    daemonOk.value = resp.ok
    daemonLatency.value = Math.round(performance.now() - t0)
  } catch {
    daemonOk.value = false
    daemonLatency.value = null
  }
}

onMounted(async () => {
  try { stats.value = await getStats() } catch { /* ignore */ }

  for (const a of adapterDefs) {
    try {
      const v = await getConfig(`adapter.${a.key}.enabled`)
      adapterEnabled.value[a.key] = v === null ? true : v === 'true'
    } catch {
      adapterEnabled.value[a.key] = true
    }
  }
  try {
    const v = await getConfig('llm.ollama_enabled')
    llmOllama.value = v === 'true'
  } catch { /* default */ }
  try {
    const v = await getConfig('llm.cloud_fallback')
    llmCloud.value = v === 'true'
  } catch { /* default */ }

  await probeDaemon()
  loading.value = false
})

function refresh() {
  loading.value = true
  Promise.allSettled([getStats().then((s) => (stats.value = s)), probeDaemon()]).finally(() => {
    loading.value = false
  })
}

const systemSignals = (): Signal[] => {
  const out: Signal[] = []
  out.push(
    daemonOk.value === null
      ? { label: 'Daemon', value: 'checking…', tone: 'muted' }
      : daemonOk.value
        ? { label: 'Daemon', value: 'running', hint: daemonLatency.value !== null ? `127.0.0.1:9999 · ${daemonLatency.value}ms` : '127.0.0.1:9999', tone: 'success' }
        : { label: 'Daemon', value: 'offline', hint: 'memex daemon start', tone: 'error' },
  )
  out.push(
    stats.value.db_exists
      ? { label: 'Database', value: 'ready', hint: `${formatNumber(stats.value.sessions)} sessions · ${formatNumber(stats.value.messages)} messages`, tone: 'success' }
      : { label: 'Database', value: 'not initialized', hint: 'memex ingest', tone: 'error' },
  )
  out.push({
    label: 'Index',
    value: formatNumber(stats.value.chunks) + ' chunks',
    hint: stats.value.chunks > 0 ? 'FTS5 ready' : 'no chunks yet',
    tone: stats.value.chunks > 0 ? 'success' : 'muted',
  })
  return out
}

const adapterSignals = (): Signal[] =>
  adapterDefs.map((a) => ({
    label: a.label,
    value: adapterEnabled.value[a.key] ? 'enabled' : 'disabled',
    tone: adapterEnabled.value[a.key] ? 'success' : 'muted',
  }))

const llmSignals = (): Signal[] => {
  const active = stats.value.llm_provider
  const out: Signal[] = []
  out.push(
    active
      ? { label: 'Active provider', value: active, hint: `${formatNumber(stats.value.summaries)} session · ${formatNumber(stats.value.chunks_summarized)} chunk summaries`, tone: 'success' }
      : { label: 'Active provider', value: 'none', hint: 'summaries paused', tone: 'muted' },
  )
  out.push({
    label: 'Ollama',
    value: llmOllama.value ? 'enabled' : 'disabled',
    tone: llmOllama.value ? 'success' : 'muted',
  })
  out.push({
    label: 'Cloud fallback',
    value: llmCloud.value ? 'opt-in' : 'off',
    hint: llmCloud.value ? 'redacted before send' : undefined,
    tone: llmCloud.value ? 'warning' : 'muted',
  })
  return out
}

const dotClass: Record<Tone, string> = {
  success: 'bg-success',
  warning: 'bg-warning',
  error: 'bg-destructive',
  muted: 'bg-muted-foreground/40',
}

const valueClass: Record<Tone, string> = {
  success: 'text-success',
  warning: 'text-warning',
  error: 'text-destructive',
  muted: 'text-muted-foreground',
}
</script>

<template>
  <div class="h-full overflow-y-auto px-4 py-4">
    <header class="mb-4 flex items-baseline justify-between">
      <h2 class="text-base font-semibold">Health</h2>
      <Button variant="ghost" size="sm" :disabled="loading" class="gap-1.5" @click="refresh">
        <RefreshCw class="h-3.5 w-3.5" :class="{ 'animate-spin': loading }" />
        {{ loading ? 'Refreshing' : 'Refresh' }}
      </Button>
    </header>

    <div v-if="loading && daemonOk === null" class="text-sm text-muted-foreground">Loading…</div>

    <template v-else>
      <!-- System -->
      <section>
        <div class="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">System</div>
        <ul class="space-y-2">
          <li v-for="s in systemSignals()" :key="s.label" class="flex items-center gap-2.5">
            <span class="h-2 w-2 shrink-0 rounded-full" :class="dotClass[s.tone]" />
            <span class="flex-1 text-sm text-foreground">{{ s.label }}</span>
            <span class="text-sm font-medium" :class="valueClass[s.tone]">{{ s.value }}</span>
          </li>
        </ul>
        <ul class="mt-1.5 space-y-1 pl-4">
          <li
            v-for="s in systemSignals().filter((x) => x.hint)"
            :key="s.label + '-hint'"
            class="text-xs text-muted-foreground"
          >{{ s.label }}: {{ s.hint }}</li>
        </ul>
      </section>

      <Separator class="my-4" />

      <!-- Adapters -->
      <section>
        <div class="mb-2 flex items-baseline justify-between">
          <div class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Adapters</div>
          <div class="text-xs text-muted-foreground">{{ adapterSignals().filter((s) => s.tone === 'success').length }}/{{ adapterDefs.length }} on</div>
        </div>
        <ul class="space-y-2">
          <li v-for="s in adapterSignals()" :key="s.label" class="flex items-center gap-2.5">
            <span class="h-2 w-2 shrink-0 rounded-full" :class="dotClass[s.tone]" />
            <span class="flex-1 text-sm">{{ s.label }}</span>
            <span class="text-sm" :class="valueClass[s.tone]">{{ s.value }}</span>
          </li>
        </ul>
      </section>

      <Separator class="my-4" />

      <!-- LLM -->
      <section>
        <div class="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">LLM</div>
        <ul class="space-y-2">
          <li v-for="s in llmSignals()" :key="s.label" class="flex items-center gap-2.5">
            <span class="h-2 w-2 shrink-0 rounded-full" :class="dotClass[s.tone]" />
            <span class="flex-1 text-sm">{{ s.label }}</span>
            <span class="text-sm" :class="valueClass[s.tone]">{{ s.value }}</span>
          </li>
        </ul>
        <ul class="mt-1.5 space-y-1 pl-4">
          <li
            v-for="s in llmSignals().filter((x) => x.hint)"
            :key="s.label + '-hint'"
            class="text-xs text-muted-foreground"
          >{{ s.label }}: {{ s.hint }}</li>
        </ul>
      </section>
    </template>
  </div>
</template>
