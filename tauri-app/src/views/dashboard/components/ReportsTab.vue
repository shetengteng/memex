<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useMemex } from '@/composables/useMemex'
import { useI18n } from '@/i18n'
import { Separator } from '@/components/ui/separator'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { Card, CardContent } from '@/components/ui/card'
import { RefreshCw, Sparkles } from 'lucide-vue-next'
import type { AggregateSummary } from '@/types'

const { t } = useI18n()
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

async function handleRegenerate(scopeKey?: string) {
  regenerating.value = true
  regenError.value = ''
  try {
    const updated = await regenerateReport(scope.value, scopeKey)
    if (updated) {
      await load()
      selectedKey.value = updated.scope_key
    } else {
      regenError.value =
        scope.value === 'daily'
          ? t('reports.empty.daily')
          : t('reports.empty.weekly')
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
        <h2 class="text-xl font-semibold">{{ t('reports.title') }}</h2>
        <p class="mt-1 text-xs text-muted-foreground">{{ t('reports.subtitle') }}</p>
      </div>
      <div class="flex items-center gap-2">
        <Button
          variant="ghost"
          size="sm"
          class="h-8 gap-1.5 text-xs"
          :disabled="regenerating"
          @click="handleRegenerate"
        >
          <Sparkles class="h-3.5 w-3.5" :class="{ 'animate-pulse': regenerating }" />
          {{ regenerating ? t('reports.regenerate.in_progress') : scope === 'daily' ? t('reports.regenerate.daily') : t('reports.regenerate.weekly') }}
        </Button>
        <Button variant="ghost" size="sm" :disabled="loading" @click="load" class="h-8 gap-1.5">
          <RefreshCw class="h-3.5 w-3.5" :class="{ 'animate-spin': loading }" />
          {{ t('common.refresh') }}
        </Button>
      </div>
    </header>

    <ToggleGroup
      :model-value="scope"
      type="single"
      variant="outline"
      size="sm"
      class="mb-5"
      @update:model-value="(v) => { if (v === 'daily' || v === 'weekly') scope = v }"
    >
      <ToggleGroupItem value="daily" class="text-xs">{{ t('reports.tab.daily') }}</ToggleGroupItem>
      <ToggleGroupItem value="weekly" class="text-xs">{{ t('reports.tab.weekly') }}</ToggleGroupItem>
    </ToggleGroup>

    <Card v-if="regenError" class="mb-3 border-destructive/40 bg-destructive/5">
      <CardContent class="px-3 py-2 text-xs text-destructive">
        {{ regenError }}
      </CardContent>
    </Card>

    <div v-if="loading && !items.length" class="text-sm text-muted-foreground">{{ t('common.loading') }}</div>

    <Card v-else-if="!items.length" class="border-dashed">
      <CardContent class="px-4 py-8 text-center">
        <p class="text-sm font-medium">{{ scope === 'daily' ? t('reports.empty.daily') : t('reports.empty.weekly') }}</p>
        <p
          class="mx-auto mt-2 max-w-md text-xs text-muted-foreground"
          v-html="t('reports.empty.hint', { scope: scope === 'daily' ? t('reports.tab.daily') : t('reports.tab.weekly'), min: scope === 'daily' ? 2 : 3 })"
        />
      </CardContent>
    </Card>

    <div v-else class="grid grid-cols-[220px_1fr] gap-6">
      <aside>
        <ul class="space-y-0.5">
          <li v-for="r in items" :key="r.scope_key">
            <Button
              variant="ghost"
              class="flex h-auto w-full items-baseline justify-between rounded-md px-3 py-2 text-left transition-colors"
              :class="r.scope_key === selectedKey ? 'bg-primary/10 text-primary hover:bg-primary/10 hover:text-primary' : 'text-muted-foreground hover:text-foreground'"
              @click="selectedKey = r.scope_key"
            >
              <span class="text-sm font-medium tabular-nums">{{ formatLabel(r) }}</span>
              <span class="text-xs font-normal text-muted-foreground">{{ r.session_count }}</span>
            </Button>
          </li>
        </ul>
      </aside>

      <article v-if="current">
        <header class="mb-3 flex items-start justify-between">
          <div>
            <h3 class="text-lg font-semibold">{{ current.title || formatLabel(current) }}</h3>
            <p class="mt-1 text-xs text-muted-foreground">
              {{ t('reports.session_count', { count: current.session_count }) }} ·
              {{ t('reports.generated_at', { time: formatCreatedAt(current.created_at) }) }}
            </p>
          </div>
          <Button
            variant="ghost"
            size="sm"
            class="h-7 gap-1 text-xs text-muted-foreground"
            :disabled="regenerating"
            @click="handleRegenerate(current.scope_key)"
          >
            <RefreshCw class="h-3 w-3" :class="{ 'animate-spin': regenerating }" />
            {{ t('reports.regenerate.this') }}
          </Button>
        </header>

        <p class="text-sm leading-relaxed whitespace-pre-line">{{ current.summary }}</p>

        <template v-if="current.topics.length">
          <Separator class="my-5" />
          <div class="mb-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">{{ t('reports.section.topics') }}</div>
          <div class="flex flex-wrap gap-1.5">
            <Badge v-for="topic in current.topics" :key="topic" variant="secondary">{{ topic }}</Badge>
          </div>
        </template>

        <template v-if="current.decisions.length">
          <Separator class="my-5" />
          <div class="mb-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">{{ t('reports.section.decisions') }}</div>
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
