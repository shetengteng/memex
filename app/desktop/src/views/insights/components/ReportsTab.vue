<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  Calendar,
  CalendarDays,
  CalendarRange,
  Download,
  Sparkles,
} from 'lucide-vue-next'
import { toast } from 'vue-sonner'
import type { AggregateSummary } from '@/types'
import { useMemex } from '@/composables/useMemex'
import { humanizeBackendError } from '@/lib/utils'

// 后端支持 'daily' / 'weekly' / 'monthly'
type BackendScope = 'daily' | 'weekly' | 'monthly'

const router = useRouter()
const memex = useMemex()
const scope = ref<BackendScope>('daily')
const reports = ref<AggregateSummary[]>([])
const loading = ref(false)
const regenerating = ref(false)
const selectedId = ref<number | null>(null)

async function loadReports() {
  loading.value = true
  try {
    reports.value = await memex.listReports(scope.value, 60)
    selectedId.value = reports.value[0]?.id ?? null
  } catch (e) {
    console.warn('[ReportsTab] listReports failed', e)
    reports.value = []
  } finally {
    loading.value = false
  }
}

onMounted(loadReports)
watch(scope, loadReports)

const selectedReport = computed(
  () => reports.value.find((r) => r.id === selectedId.value) ?? reports.value[0] ?? null,
)

function onScopeChange(s: string | number) {
  const sv = String(s)
  if (sv === 'daily' || sv === 'weekly' || sv === 'monthly') scope.value = sv
}

async function regenerate() {
  if (!selectedReport.value || regenerating.value) return
  regenerating.value = true
  try {
    const r = await memex.regenerateReport(scope.value, selectedReport.value.scope_key)
    if (r) {
      toast.success('已重新生成')
      await loadReports()
      // 保持选中同一个 scope_key
      const same = reports.value.find((x) => x.scope_key === r.scope_key)
      if (same) selectedId.value = same.id
    } else {
      toast.info('未生成新报告，可能是数据不足')
    }
  } catch (e) {
    const fe = humanizeBackendError(e)
    toast.error('重新生成失败', {
      description: fe.friendly,
      action: fe.action
        ? { label: fe.action.label, onClick: () => router.push(fe.action!.route) }
        : undefined,
      duration: 8000,
    })
  } finally {
    regenerating.value = false
  }
}
</script>

<template>
  <div class="mx-auto w-full max-w-6xl px-4 py-4 lg:px-6 lg:py-6">
    <div class="mb-4 flex items-center justify-between">
      <Tabs :model-value="scope" @update:model-value="onScopeChange">
        <TabsList class="h-9">
          <TabsTrigger value="daily" class="gap-1.5">
            <Calendar class="size-3.5" />
            日报
          </TabsTrigger>
          <TabsTrigger value="weekly" class="gap-1.5">
            <CalendarDays class="size-3.5" />
            周报
          </TabsTrigger>
          <TabsTrigger value="monthly" class="gap-1.5">
            <CalendarRange class="size-3.5" />
            月报
          </TabsTrigger>
        </TabsList>
      </Tabs>
      <div class="flex items-center gap-2">
        <Button
          variant="ghost"
          size="sm"
          class="h-8 gap-1.5"
          :disabled="!selectedReport || regenerating"
          @click="regenerate"
        >
          <Sparkles class="size-3.5" />
          {{ regenerating ? '生成中…' : '重新生成' }}
        </Button>
        <Button variant="outline" size="sm" class="h-8 gap-1.5" disabled>
          <Download class="size-3.5" />
          导出
        </Button>
      </div>
    </div>

    <div class="grid grid-cols-1 gap-6 lg:grid-cols-[240px_1fr]">
      <Card class="overflow-hidden">
        <div
          class="border-b px-3 py-2.5 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground"
        >
          {{ loading ? '加载中…' : `最近 ${reports.length} 份` }}
        </div>
        <ScrollArea class="max-h-[480px]">
          <ul>
            <li v-for="r in reports" :key="r.id">
              <button
                :class="[
                  'flex w-full items-baseline justify-between px-3 py-2 text-left transition-colors hover:bg-accent',
                  selectedId === r.id ? 'bg-accent font-semibold' : '',
                ]"
                @click="selectedId = r.id"
              >
                <span class="text-[13px] tabular-nums">{{ r.scope_key }}</span>
                <span class="text-[11px] text-muted-foreground tabular-nums">
                  {{ r.session_count }} 个
                </span>
              </button>
            </li>
            <li v-if="!loading && !reports.length" class="px-3 py-6 text-center text-[12px] italic text-muted-foreground">
              暂无报告，点击右上角"重新生成"
            </li>
          </ul>
        </ScrollArea>
      </Card>

      <Card v-if="selectedReport" class="p-6">
        <header class="mb-4">
          <div class="mb-1 flex items-baseline gap-3">
            <h2 class="text-lg font-semibold">{{ selectedReport.title || selectedReport.scope_key }}</h2>
            <Badge variant="secondary">{{ selectedReport.session_count }} 个会话</Badge>
          </div>
          <p class="text-[12px] text-muted-foreground">
            生成于 {{ new Date(selectedReport.created_at).toLocaleString() }}
          </p>
        </header>

        <pre
          class="whitespace-pre-wrap font-sans text-[14px] leading-relaxed"
        >{{ selectedReport.summary }}</pre>

        <template v-if="selectedReport.decisions.length">
          <Separator class="my-5" />
          <div class="mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
            关键决策
          </div>
          <ul class="space-y-1.5 text-[13px]">
            <li v-for="d in selectedReport.decisions" :key="d" class="flex gap-2">
              <span class="mt-2 size-1 shrink-0 rounded-full bg-primary" />
              <span>{{ d }}</span>
            </li>
          </ul>
        </template>

        <template v-if="selectedReport.topics.length">
          <Separator class="my-5" />
          <div class="mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
            主题
          </div>
          <div class="flex flex-wrap gap-1.5">
            <Badge v-for="t in selectedReport.topics" :key="t" variant="secondary">{{ t }}</Badge>
          </div>
        </template>
      </Card>

      <Card v-else class="flex items-center justify-center p-6 text-[12px] italic text-muted-foreground">
        {{ loading ? '加载中…' : '请选择一个报告' }}
      </Card>
    </div>
  </div>
</template>
