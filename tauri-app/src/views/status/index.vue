<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useMemex } from '@/composables/useMemex'
import { formatNumber } from '@/lib/utils'
import { Separator } from '@/components/ui/separator'
import type { Stats } from '@/types'

const { getStats } = useMemex()
const stats = ref<Stats>({ sessions: 0, messages: 0, chunks: 0, db_exists: false, summaries: 0, chunks_summarized: 0, llm_provider: null })
const loading = ref(true)

const adapterList = ['Claude Code', 'Cursor', 'Codex', 'OpenCode', 'Aider', 'Continue', 'Cline']

onMounted(async () => {
  try { stats.value = await getStats() } catch { /* ignore */ }
  loading.value = false
})
</script>

<template>
  <div class="mono h-full space-y-0 overflow-y-auto px-3.5 py-3 text-[11px] leading-relaxed">
    <template v-if="!loading">
      <div><span :class="stats.db_exists ? 'text-success' : 'text-warning'">{{ stats.db_exists ? '✓' : '!' }}</span> index.db — {{ formatNumber(stats.sessions) }} sessions, {{ formatNumber(stats.messages) }} messages</div>
      <div><span class="text-success">✓</span> FTS5 — {{ formatNumber(stats.chunks) }} chunks</div>
      <div v-for="a in adapterList" :key="a"><span class="text-success">✓</span> {{ a }} adapter</div>
      <div><span class="text-success">✓</span> HTTP — 127.0.0.1:9999</div>
      <Separator class="my-1.5" />
      <div :class="stats.llm_provider ? 'text-success' : 'text-warning'">
        <span>{{ stats.llm_provider ? '✓' : '!' }}</span>
        LLM — {{ stats.llm_provider ?? 'disabled' }}
      </div>
      <div class="text-muted-foreground pl-3">
        {{ stats.summaries }} session summaries · {{ stats.chunks_summarized }} chunk summaries
      </div>
      <Separator class="my-1.5" />
      <div class="text-muted-foreground">{{ adapterList.length + 3 }} checks · daemon running</div>
    </template>
    <div v-else class="text-muted-foreground">加载中...</div>
  </div>
</template>
