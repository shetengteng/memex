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
import { save as saveDialog } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
import type { AggregateSummary } from '@/types'
import { useMemex } from '@/composables/useMemex'
import { humanizeBackendError } from '@/lib/utils'
import { useI18n } from '@/i18n'

const { t } = useI18n()

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
  if (regenerating.value) return
  regenerating.value = true
  const scopeKey = selectedReport.value?.scope_key
  try {
    const r = await memex.regenerateReport(scope.value, scopeKey)
    if (r) {
      toast.success(scopeKey ? t('insights.reports.toast.regenerated') : t('insights.reports.toast.generated'))
      await loadReports()
      const same = reports.value.find((x) => x.scope_key === r.scope_key)
      if (same) selectedId.value = same.id
    } else {
      toast.info(t('insights.reports.toast.no_new'))
    }
  } catch (e) {
    const fe = humanizeBackendError(e)
    toast.error(t('insights.reports.toast.regenerate_failed'), {
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

const exporting = ref(false)

function buildExportMarkdown(r: AggregateSummary): string {
  const lines: string[] = []
  lines.push(`# ${r.title || r.scope_key}`)
  lines.push('')
  lines.push(`- Scope: \`${scope.value}\``)
  lines.push(`- Scope key: \`${r.scope_key}\``)
  lines.push(`- Sessions: ${r.session_count}`)
  lines.push(`- Generated at: ${new Date(r.created_at).toISOString()}`)
  lines.push('')
  lines.push('## Summary')
  lines.push('')
  lines.push(r.summary || '_(empty)_')
  if (r.decisions.length) {
    lines.push('')
    lines.push('## Key Decisions')
    lines.push('')
    for (const d of r.decisions) lines.push(`- ${d}`)
  }
  if (r.topics.length) {
    lines.push('')
    lines.push('## Topics')
    lines.push('')
    lines.push(r.topics.map((t) => `\`${t}\``).join(' · '))
  }
  lines.push('')
  return lines.join('\n')
}

async function exportReport() {
  if (!selectedReport.value || exporting.value) return
  exporting.value = true
  try {
    const r = selectedReport.value
    const safeKey = r.scope_key.replace(/[^A-Za-z0-9._-]+/g, '_')
    const defaultName = `memex-${scope.value}-${safeKey}.md`
    const target = await saveDialog({
      title: t('insights.reports.export.dialog_title'),
      defaultPath: defaultName,
      filters: [{ name: 'Markdown', extensions: ['md'] }],
    })
    if (!target) return
    await invoke('export_text_file', {
      targetPath: target,
      content: buildExportMarkdown(r),
    })
    toast.success(t('insights.reports.toast.export_done', { target }))
  } catch (e) {
    toast.error(t('insights.reports.toast.export_failed'), { description: String(e) })
  } finally {
    exporting.value = false
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
            {{ t('insights.reports.scope.daily') }}
          </TabsTrigger>
          <TabsTrigger value="weekly" class="gap-1.5">
            <CalendarDays class="size-3.5" />
            {{ t('insights.reports.scope.weekly') }}
          </TabsTrigger>
          <TabsTrigger value="monthly" class="gap-1.5">
            <CalendarRange class="size-3.5" />
            {{ t('insights.reports.scope.monthly') }}
          </TabsTrigger>
        </TabsList>
      </Tabs>
      <div class="flex items-center gap-2">
        <Button
          variant="ghost"
          size="sm"
          class="h-8 gap-1.5"
          :title="selectedReport
            ? t('insights.reports.action.regenerate_tooltip_existing', { key: selectedReport.scope_key })
            : t('insights.reports.action.regenerate_tooltip_new', { kind: t(`insights.reports.scope_kind.${scope}`) })"
          :disabled="regenerating"
          @click="regenerate"
        >
          <Sparkles class="size-3.5" />
          {{ regenerating
            ? t('insights.reports.action.regenerate_busy')
            : (selectedReport ? t('insights.reports.action.regenerate') : t('insights.reports.action.generate')) }}
        </Button>
        <Button
          variant="outline"
          size="sm"
          class="h-8 gap-1.5"
          :disabled="!selectedReport || exporting"
          @click="exportReport"
        >
          <Download class="size-3.5" />
          {{ exporting ? t('insights.reports.action.export_busy') : t('insights.reports.action.export') }}
        </Button>
      </div>
    </div>

    <div class="grid grid-cols-1 gap-6 lg:grid-cols-[240px_1fr]">
      <Card class="overflow-hidden">
        <div
          class="border-b px-3 py-2.5 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground"
        >
          {{ loading
            ? t('insights.reports.list.loading')
            : t('insights.reports.list.recent_n', { n: reports.length }) }}
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
                  {{ t('insights.reports.list.entry_count', { n: r.session_count }) }}
                </span>
              </button>
            </li>
            <li v-if="!loading && !reports.length" class="px-3 py-6 text-center text-[12px] italic text-muted-foreground">
              {{ t('insights.reports.list.empty') }}
            </li>
          </ul>
        </ScrollArea>
      </Card>

      <Card v-if="selectedReport" class="p-6">
        <header class="mb-4">
          <div class="mb-1 flex items-baseline gap-3">
            <h2 class="text-lg font-semibold">{{ selectedReport.title || selectedReport.scope_key }}</h2>
            <Badge variant="secondary">{{ t('insights.reports.detail.session_count', { n: selectedReport.session_count }) }}</Badge>
          </div>
          <p class="text-[12px] text-muted-foreground">
            {{ t('insights.reports.detail.generated_at', { time: new Date(selectedReport.created_at).toLocaleString() }) }}
          </p>
        </header>

        <pre
          class="whitespace-pre-wrap font-sans text-[14px] leading-relaxed"
        >{{ selectedReport.summary }}</pre>

        <template v-if="selectedReport.decisions.length">
          <Separator class="my-5" />
          <div class="mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
            {{ t('insights.reports.detail.section.decisions') }}
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
            {{ t('insights.reports.detail.section.topics') }}
          </div>
          <div class="flex flex-wrap gap-1.5">
            <Badge v-for="topic in selectedReport.topics" :key="topic" variant="secondary">{{ topic }}</Badge>
          </div>
        </template>
      </Card>

      <Card v-else class="flex items-center justify-center p-6 text-[12px] italic text-muted-foreground">
        {{ loading ? t('insights.reports.detail.empty_loading') : t('insights.reports.detail.empty_pick') }}
      </Card>
    </div>
  </div>
</template>
