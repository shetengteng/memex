<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { Lightbulb, RefreshCw, AlertCircle, ChevronLeft, Sparkles } from 'lucide-vue-next'
import type { ReflectEntry, ReflectDetail } from '@/types'
import { useMemex } from '@/composables/useMemex'
import { useI18n } from '@/i18n'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'

const { t } = useI18n()
const { reflectList, reflectGet, reflectRun } = useMemex()

const entries = ref<ReflectEntry[]>([])
const detail = ref<ReflectDetail | null>(null)
const loadingList = ref(false)
const loadingDetail = ref(false)
const running = ref(false)
const runError = ref('')
const listError = ref('')
const period = ref<'week' | 'month' | '7d' | '14d' | '30d'>('week')
const lastRunResult = ref<{ scope_key: string; period_label: string } | null>(null)

const periodOptions = computed(() => [
  { value: 'week', label: t('reflect.period.week') },
  { value: '7d', label: t('reflect.period.last_7d') },
  { value: '14d', label: t('reflect.period.last_14d') },
  { value: '30d', label: t('reflect.period.last_30d') },
  { value: 'month', label: t('reflect.period.month') },
])

async function loadList() {
  loadingList.value = true
  listError.value = ''
  try {
    entries.value = await reflectList()
  } catch (e) {
    listError.value = e instanceof Error ? e.message : String(e)
  } finally {
    loadingList.value = false
  }
}

async function openDetail(entry: ReflectEntry) {
  loadingDetail.value = true
  detail.value = null
  try {
    detail.value = await reflectGet(entry.scope_key)
  } catch (e) {
    listError.value = e instanceof Error ? e.message : String(e)
  } finally {
    loadingDetail.value = false
  }
}

function backToList() {
  detail.value = null
}

async function runReflect() {
  if (running.value) return
  running.value = true
  runError.value = ''
  lastRunResult.value = null
  try {
    const r = await reflectRun(period.value)
    lastRunResult.value = { scope_key: r.scope_key, period_label: r.period_label }
    await loadList()
    // 跑完直接打开新生成的 detail，方便查看
    const matched = entries.value.find((e) => e.scope_key === r.scope_key)
    if (matched) await openDetail(matched)
  } catch (e) {
    runError.value = e instanceof Error ? e.message : String(e)
  } finally {
    running.value = false
  }
}

function formatScopeKey(key: string): string {
  // week:2026-W23 → "周报 W23"，month:2026-06 → "月报 2026-06"，days7:2026-06-04 → "近 7 天"
  if (key.startsWith('week:')) {
    return key.replace(/^week:/, t('reflect.label.week_prefix') + ' ')
  }
  if (key.startsWith('month:')) {
    return key.replace(/^month:/, t('reflect.label.month_prefix') + ' ')
  }
  const daysMatch = key.match(/^days(\d+):(.+)$/)
  if (daysMatch) {
    return t('reflect.label.days_format', { n: daysMatch[1], date: daysMatch[2] })
  }
  return key
}

function formatDate(s: string): string {
  try {
    return new Date(s).toLocaleString()
  } catch {
    return s
  }
}

// 极简 markdown 渲染：## 标题 / - 列表 / 段落。reflect 输出已经是受控格式，不引入 markdown 库。
function renderMarkdown(md: string): string {
  const lines = md.split('\n')
  const out: string[] = []
  let inList = false
  for (const line of lines) {
    const trimmed = line.trim()
    if (trimmed.startsWith('# ')) {
      if (inList) { out.push('</ul>'); inList = false }
      out.push(`<h2 class="mt-4 mb-2 text-base font-semibold">${escape(trimmed.slice(2))}</h2>`)
    } else if (trimmed.startsWith('## ')) {
      if (inList) { out.push('</ul>'); inList = false }
      out.push(`<h3 class="mt-3 mb-1.5 text-sm font-semibold text-muted-foreground uppercase tracking-wider">${escape(trimmed.slice(3))}</h3>`)
    } else if (trimmed.startsWith('- ')) {
      if (!inList) { out.push('<ul class="space-y-1 pl-4 list-disc text-sm">'); inList = true }
      out.push(`<li>${escape(trimmed.slice(2))}</li>`)
    } else if (trimmed.startsWith('_') && trimmed.endsWith('_')) {
      if (inList) { out.push('</ul>'); inList = false }
      out.push(`<p class="text-sm text-muted-foreground italic">${escape(trimmed.slice(1, -1))}</p>`)
    } else if (trimmed.length > 0) {
      if (inList) { out.push('</ul>'); inList = false }
      out.push(`<p class="text-sm">${escape(trimmed)}</p>`)
    }
  }
  if (inList) out.push('</ul>')
  return out.join('\n')
}

function escape(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
}

onMounted(loadList)
</script>

<template>
  <div>
    <div class="mb-5 flex items-center justify-between">
      <div class="flex items-center gap-2">
        <Lightbulb class="h-5 w-5 text-amber-500" />
        <h2 class="text-xl font-bold tracking-tight">{{ t('reflect.title') }}</h2>
      </div>
    </div>

    <p class="mb-5 text-sm text-muted-foreground leading-snug">{{ t('reflect.intro') }}</p>

    <!-- 触发面板 -->
    <div class="mb-5 rounded-lg border border-border bg-card p-4">
      <div class="mb-3 flex items-center gap-2">
        <Sparkles class="h-4 w-4 text-primary" />
        <h3 class="text-sm font-semibold">{{ t('reflect.run.title') }}</h3>
      </div>

      <div class="flex flex-wrap items-center gap-2">
        <span class="text-xs text-muted-foreground">{{ t('reflect.run.period_label') }}</span>
        <select
          v-model="period"
          class="h-8 rounded-md border border-input bg-background px-2 text-xs"
          :disabled="running"
        >
          <option v-for="opt in periodOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
        </select>
        <Button size="sm" class="h-8 gap-1.5" :disabled="running" @click="runReflect">
          <RefreshCw class="h-3 w-3" :class="running ? 'animate-spin' : ''" />
          {{ running ? t('reflect.run.running') : t('reflect.run.button') }}
        </Button>
        <span v-if="running" class="text-[11px] text-muted-foreground italic">{{ t('reflect.run.hint_long_task') }}</span>
      </div>

      <div v-if="runError" class="mt-2 rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-xs text-destructive">
        <AlertCircle class="mr-1 inline h-3 w-3" />
        {{ runError }}
      </div>
      <div v-else-if="lastRunResult" class="mt-2 text-xs text-success">
        {{ t('reflect.run.done', { label: lastRunResult.period_label }) }}
      </div>
    </div>

    <!-- 详情视图 -->
    <div v-if="detail" class="rounded-lg border border-border bg-card">
      <div class="flex items-center justify-between gap-2 border-b border-border px-4 py-2.5">
        <div class="flex items-center gap-2">
          <Button variant="ghost" size="sm" class="h-7 gap-1 px-2 text-xs" @click="backToList">
            <ChevronLeft class="h-3 w-3" />
            {{ t('reflect.detail.back') }}
          </Button>
          <span class="text-sm font-semibold">{{ detail.title ?? formatScopeKey(detail.scope_key) }}</span>
          <Badge variant="secondary" class="text-[10px]">{{ t('reflect.detail.digest_count', { n: detail.digest_count }) }}</Badge>
        </div>
        <span class="text-[10px] text-muted-foreground">{{ formatDate(detail.created_at) }}</span>
      </div>
      <div class="px-5 py-4" v-html="renderMarkdown(detail.markdown)" />
    </div>

    <!-- 列表视图 -->
    <div v-else>
      <div class="mb-2 flex items-center justify-between">
        <span class="text-xs font-medium uppercase tracking-wider text-muted-foreground">{{ t('reflect.list.title') }}</span>
        <Button variant="ghost" size="sm" class="h-6 gap-1 px-2 text-xs" :disabled="loadingList" @click="loadList">
          <RefreshCw class="h-3 w-3" :class="loadingList ? 'animate-spin' : ''" />
          {{ t('reflect.list.refresh') }}
        </Button>
      </div>

      <div v-if="loadingList && entries.length === 0" class="flex items-center justify-center rounded-lg border border-border bg-card py-12 text-xs text-muted-foreground">
        <RefreshCw class="mr-2 h-3 w-3 animate-spin" />
        {{ t('reflect.list.loading') }}
      </div>

      <div v-else-if="listError" class="rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-xs text-destructive">
        <AlertCircle class="mr-1 inline h-3 w-3" />
        {{ listError }}
      </div>

      <div v-else-if="entries.length === 0" class="rounded-lg border border-dashed border-border px-4 py-12 text-center">
        <Lightbulb class="mx-auto mb-2 h-6 w-6 text-muted-foreground" />
        <p class="text-sm text-muted-foreground">{{ t('reflect.list.empty') }}</p>
        <p class="mt-1 text-[11px] text-muted-foreground italic">{{ t('reflect.list.empty_hint') }}</p>
      </div>

      <div v-else class="overflow-hidden rounded-lg border border-border">
        <button
          v-for="(e, i) in entries"
          :key="e.scope_key"
          class="flex w-full items-center justify-between gap-3 px-4 py-3 text-left transition-colors hover:bg-accent"
          :class="{ 'border-t border-border/40': i > 0 }"
          @click="openDetail(e)"
        >
          <div class="min-w-0 flex-1">
            <div class="flex items-center gap-2">
              <span class="text-sm font-medium">{{ e.title ?? formatScopeKey(e.scope_key) }}</span>
              <Badge variant="secondary" class="text-[10px]">{{ t('reflect.detail.digest_count', { n: e.digest_count }) }}</Badge>
            </div>
            <p class="mt-0.5 text-[11px] text-muted-foreground">
              <code class="font-mono">{{ e.scope_key }}</code>
              · {{ formatDate(e.created_at) }}
            </p>
          </div>
          <ChevronLeft class="h-4 w-4 shrink-0 rotate-180 text-muted-foreground" />
        </button>
      </div>
    </div>

    <div v-if="loadingDetail" class="mt-4 flex items-center justify-center text-xs text-muted-foreground">
      <RefreshCw class="mr-2 h-3 w-3 animate-spin" />
      {{ t('reflect.detail.loading') }}
    </div>
  </div>
</template>
