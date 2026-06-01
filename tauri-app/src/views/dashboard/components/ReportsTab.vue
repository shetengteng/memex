<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useMemex } from '@/composables/useMemex'
import { Separator } from '@/components/ui/separator'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { RefreshCw, Sparkles } from 'lucide-vue-next'
import type { AggregateSummary } from '@/types'

const { listReports, regenerateReport } = useMemex()

const scope = ref<'daily' | 'weekly'>('daily')
const items = ref<AggregateSummary[]>([])
const selectedKey = ref<string | null>(null)
const loading = ref(false)
const regenerating = ref(false)
const regenError = ref('')

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

async function handleRegenerate() {
  regenerating.value = true
  regenError.value = ''
  try {
    const updated = await regenerateReport(scope.value)
    if (updated) {
      await load()
      selectedKey.value = updated.scope_key
    } else {
      regenError.value =
        scope.value === 'daily'
          ? '今天没有足够的会话摘要可供生成日报'
          : '本周没有足够的会话摘要可供生成周报'
    }
  } catch (e: unknown) {
    regenError.value = e instanceof Error ? e.message : String(e)
  } finally {
    regenerating.value = false
  }
}

onMounted(load)
watch(scope, () => {
  selectedKey.value = null
  regenError.value = ''
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
        <h2 class="text-xl font-semibold">报告</h2>
        <p class="mt-1 text-xs text-muted-foreground">
          基于 L2 会话摘要自动生成的日报和周报。
        </p>
      </div>
      <div class="flex items-center gap-2">
        <Button
          variant="outline"
          size="sm"
          class="h-8 gap-1.5 text-xs"
          :disabled="regenerating"
          @click="handleRegenerate"
        >
          <Sparkles class="h-3.5 w-3.5" :class="{ 'animate-pulse': regenerating }" />
          {{ regenerating ? '生成中…' : scope === 'daily' ? '重新生成日报' : '重新生成周报' }}
        </Button>
        <Button variant="ghost" size="sm" :disabled="loading" @click="load" class="h-8 gap-1.5">
          <RefreshCw class="h-3.5 w-3.5" :class="{ 'animate-spin': loading }" />
          刷新
        </Button>
      </div>
    </header>

    <div class="mb-5 inline-flex rounded-md border border-border p-0.5">
      <button
        v-for="s in (['daily', 'weekly'] as const)"
        :key="s"
        class="px-3 py-1 text-xs font-medium transition-colors"
        :class="scope === s ? 'rounded bg-muted text-foreground' : 'text-muted-foreground hover:text-foreground'"
        @click="scope = s"
      >{{ s === 'daily' ? '日报' : '周报' }}</button>
    </div>

    <div v-if="regenError" class="mb-3 rounded-md border border-destructive/40 bg-destructive/5 px-3 py-2 text-xs text-destructive">
      {{ regenError }}
    </div>

    <div v-if="loading && !items.length" class="text-sm text-muted-foreground">加载中…</div>

    <div v-else-if="!items.length" class="rounded-md border border-dashed border-border px-4 py-8 text-center">
      <p class="text-sm font-medium">还没有{{ scope === 'daily' ? '日报' : '周报' }}</p>
      <p class="mx-auto mt-2 max-w-md text-xs text-muted-foreground">
        当 LLM 服务可用且
        {{ scope === 'daily' ? '当天' : '本 ISO 周' }}内至少有
        {{ scope === 'daily' ? 2 : 3 }} 个会话时，会在每次 ingest 时自动生成报告。
        可在<em>设置</em>里启用 Ollama 或配置 Claude API Key，然后运行 <code>memex ingest</code>，
        或点击右上角"重新生成"按钮立即触发。
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
            涵盖 {{ current.session_count }} 个会话 · 生成于 {{ formatCreatedAt(current.created_at) }}
          </p>
        </header>

        <p class="text-sm leading-relaxed whitespace-pre-line">{{ current.summary }}</p>

        <template v-if="current.topics.length">
          <Separator class="my-5" />
          <div class="mb-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">主题</div>
          <div class="flex flex-wrap gap-1.5">
            <Badge v-for="t in current.topics" :key="t" variant="secondary">{{ t }}</Badge>
          </div>
        </template>

        <template v-if="current.decisions.length">
          <Separator class="my-5" />
          <div class="mb-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">关键决策</div>
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
