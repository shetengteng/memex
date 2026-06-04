<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { RefreshCw, Activity } from 'lucide-vue-next'
import { useMemex } from '@/composables/useMemex'
import { useI18n } from '@/i18n'
import { adapterLabel, formatNumber } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import type { WorkloadReport, WorkloadHeatmapCell, WorkloadBucket, WorkloadProjectBucket } from '@/types'

const { t } = useI18n()
const { getWorkload } = useMemex()

type Range = 7 | 30 | 90
const days = ref<Range>(30)
const report = ref<WorkloadReport | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)

const adapterColors: Record<string, string> = {
  claude_code: '#3b82f6',
  cursor: '#a78bfa',
  codex: '#22d3ee',
  opencode: '#22c55e',
  aider: '#f59e0b',
  continue: '#4ade80',
  continue_dev: '#4ade80',
  cline: '#ec4899',
}

async function load() {
  loading.value = true
  error.value = null
  try {
    report.value = await getWorkload(days.value)
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}

onMounted(load)
watch(days, load)

const overall = computed(() => report.value?.overall)
const isEmpty = computed(() => !overall.value || overall.value.sessions === 0)

// 168 个桶（7 weekday × 24 hour），把后端 sparse 输出补满
const heatmapGrid = computed(() => {
  const grid: WorkloadHeatmapCell[][] = []
  for (let w = 0; w < 7; w++) {
    grid.push(Array.from({ length: 24 }, (_, h) => ({
      weekday: w,
      hour: h,
      sessions: 0,
      messages: 0,
    })))
  }
  if (report.value) {
    for (const cell of report.value.heatmap) {
      if (cell.weekday >= 0 && cell.weekday <= 6 && cell.hour >= 0 && cell.hour <= 23) {
        grid[cell.weekday][cell.hour] = cell
      }
    }
  }
  return grid
})

const heatmapMax = computed(() => {
  let m = 0
  for (const row of heatmapGrid.value) {
    for (const c of row) {
      if (c.sessions > m) m = c.sessions
    }
  }
  return m
})

// 用 log1p 拉伸长尾色阶，避免 1 个会话的格子和 50 个的格子看起来一样深
function heatColor(sessions: number): string {
  if (sessions <= 0) return 'transparent'
  if (heatmapMax.value <= 0) return 'transparent'
  const t = Math.log1p(sessions) / Math.log1p(heatmapMax.value)
  const opacity = Math.max(0.12, Math.min(1, t))
  return `hsl(217 91% 60% / ${opacity})`
}

const adapterTotal = computed(() => {
  if (!report.value) return 0
  return report.value.by_adapter.reduce((acc, b) => acc + b.sessions, 0)
})

function adapterPercent(b: WorkloadBucket): number {
  if (adapterTotal.value <= 0) return 0
  return (b.sessions / adapterTotal.value) * 100
}

function adapterColor(key: string): string {
  return adapterColors[key] ?? '#94a3b8'
}

const projectMax = computed(() => {
  if (!report.value) return 0
  return report.value.by_project.reduce((m, p) => Math.max(m, p.sessions), 0)
})

function projectBarPercent(p: WorkloadProjectBucket): number {
  if (projectMax.value <= 0) return 0
  return (p.sessions / projectMax.value) * 100
}

function projectLabel(p: WorkloadProjectBucket): string {
  if (p.project_path === '(no project)') return t('workload.project.no_project')
  return p.name || p.project_path
}

function weekdayLabel(w: number): string {
  return t(`workload.heatmap.weekday.${w}` as 'workload.heatmap.weekday.0')
}

function hourLabel(h: number): string {
  return h.toString().padStart(2, '0')
}
</script>

<template>
  <div class="space-y-6 p-6">
    <!-- Header -->
    <div class="flex items-start justify-between gap-4">
      <div>
        <h2 class="flex items-center gap-2 text-xl font-bold">
          <Activity class="h-5 w-5 text-primary" />
          {{ t('workload.title') }}
        </h2>
        <p class="mt-1 text-sm text-muted-foreground">{{ t('workload.intro') }}</p>
      </div>
      <div class="flex shrink-0 items-center gap-2">
        <span class="text-xs text-muted-foreground">{{ t('workload.range.label') }}</span>
        <div class="inline-flex rounded-md border border-border bg-card p-0.5">
          <button
            v-for="r in ([7, 30, 90] as Range[])"
            :key="r"
            class="rounded px-3 py-1 text-xs font-medium transition-colors"
            :class="days === r ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-muted'"
            @click="days = r"
          >
            {{ t(`workload.range.${r}d` as 'workload.range.7d') }}
          </button>
        </div>
        <Button variant="outline" size="sm" :disabled="loading" @click="load">
          <RefreshCw class="h-3.5 w-3.5" :class="loading ? 'animate-spin' : ''" />
          {{ t('workload.refresh') }}
        </Button>
      </div>
    </div>

    <!-- Error -->
    <div v-if="error" class="rounded-md border border-destructive/40 bg-destructive/10 p-3 text-sm text-destructive">
      {{ error }}
    </div>

    <!-- Loading skeleton on first load -->
    <div v-if="loading && !report" class="rounded-md border border-border bg-card p-8 text-center text-sm text-muted-foreground">
      {{ t('workload.loading') }}
    </div>

    <!-- Empty -->
    <div
      v-else-if="isEmpty"
      class="rounded-md border border-border bg-card p-10 text-center"
    >
      <p class="text-base font-medium">{{ t('workload.empty.title') }}</p>
      <p class="mt-1 text-sm text-muted-foreground">{{ t('workload.empty.hint') }}</p>
    </div>

    <template v-else-if="report">
      <!-- KPI row -->
      <section class="grid grid-cols-2 gap-4 md:grid-cols-4">
        <div class="rounded-md border border-border bg-card p-4">
          <div class="text-xs font-medium text-muted-foreground">{{ t('workload.overall.sessions') }}</div>
          <div class="mt-1 text-3xl font-bold tabular-nums">{{ formatNumber(overall!.sessions) }}</div>
        </div>
        <div class="rounded-md border border-border bg-card p-4">
          <div class="text-xs font-medium text-muted-foreground">{{ t('workload.overall.messages') }}</div>
          <div class="mt-1 text-3xl font-bold tabular-nums">{{ formatNumber(overall!.messages) }}</div>
        </div>
        <div class="rounded-md border border-border bg-card p-4">
          <div class="text-xs font-medium text-muted-foreground">{{ t('workload.overall.active_days') }}</div>
          <div class="mt-1 flex items-baseline gap-2">
            <span class="text-3xl font-bold tabular-nums">{{ overall!.active_days }}</span>
            <span class="text-xs text-muted-foreground">/ {{ report.days }}</span>
          </div>
        </div>
        <div class="rounded-md border border-border bg-card p-4">
          <div class="text-xs font-medium text-muted-foreground">{{ t('workload.overall.peak_day') }}</div>
          <div v-if="overall!.peak_day" class="mt-1 text-sm font-semibold tabular-nums">
            {{ t('workload.overall.peak_day_value', { date: overall!.peak_day, n: overall!.peak_day_sessions }) }}
          </div>
          <div v-else class="mt-1 text-sm text-muted-foreground">—</div>
        </div>
      </section>

      <!-- Heatmap -->
      <section class="rounded-md border border-border bg-card p-5">
        <div class="mb-3 flex items-baseline justify-between">
          <div>
            <h3 class="text-sm font-bold">{{ t('workload.heatmap.title') }}</h3>
            <p class="mt-0.5 text-xs text-muted-foreground">{{ t('workload.heatmap.subtitle', { n: report.days }) }}</p>
          </div>
        </div>
        <div v-if="heatmapMax === 0" class="py-6 text-center text-sm text-muted-foreground">
          {{ t('workload.heatmap.empty') }}
        </div>
        <div v-else class="overflow-x-auto">
          <table class="w-full text-xs">
            <thead>
              <tr>
                <th class="w-10"></th>
                <th v-for="h in 24" :key="h - 1" class="px-0.5 text-center font-normal text-muted-foreground">
                  <span v-if="(h - 1) % 3 === 0">{{ hourLabel(h - 1) }}</span>
                </th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(row, wi) in heatmapGrid" :key="wi">
                <td class="pr-2 text-right text-muted-foreground">{{ weekdayLabel(wi) }}</td>
                <td v-for="cell in row" :key="cell.hour" class="p-0.5">
                  <Tooltip>
                    <TooltipTrigger as-child>
                      <div
                        class="h-5 w-full rounded-sm border border-border/30 transition-colors hover:ring-2 hover:ring-primary"
                        :style="{ backgroundColor: heatColor(cell.sessions) }"
                      />
                    </TooltipTrigger>
                    <TooltipContent>
                      <div class="text-xs">
                        <div class="font-semibold">{{ weekdayLabel(wi) }} · {{ hourLabel(cell.hour) }}:00</div>
                        <div>{{ formatNumber(cell.sessions) }} sessions · {{ formatNumber(cell.messages) }} msgs</div>
                      </div>
                    </TooltipContent>
                  </Tooltip>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      <!-- 2-col: Adapter + Project -->
      <section class="grid gap-4 md:grid-cols-2">
        <!-- Adapter breakdown -->
        <div class="rounded-md border border-border bg-card p-5">
          <div class="mb-3">
            <h3 class="text-sm font-bold">{{ t('workload.adapter.title') }}</h3>
            <p class="mt-0.5 text-xs text-muted-foreground">{{ t('workload.adapter.subtitle') }}</p>
          </div>
          <div v-if="report.by_adapter.length === 0" class="py-6 text-center text-sm text-muted-foreground">
            {{ t('workload.adapter.empty') }}
          </div>
          <ul v-else class="space-y-2">
            <li v-for="b in report.by_adapter" :key="b.key" class="space-y-1">
              <div class="flex items-baseline justify-between text-xs">
                <span class="flex items-center gap-1.5 font-medium">
                  <span class="inline-block h-2.5 w-2.5 rounded-sm" :style="{ backgroundColor: adapterColor(b.key) }"></span>
                  {{ adapterLabel(b.key) }}
                </span>
                <span class="tabular-nums text-muted-foreground">
                  {{ formatNumber(b.sessions) }} · {{ adapterPercent(b).toFixed(1) }}%
                </span>
              </div>
              <div class="h-1.5 w-full overflow-hidden rounded-full bg-muted">
                <div
                  class="h-full rounded-full transition-all"
                  :style="{ width: adapterPercent(b) + '%', backgroundColor: adapterColor(b.key) }"
                ></div>
              </div>
            </li>
          </ul>
        </div>

        <!-- Project breakdown -->
        <div class="rounded-md border border-border bg-card p-5">
          <div class="mb-3">
            <h3 class="text-sm font-bold">{{ t('workload.project.title') }}</h3>
            <p class="mt-0.5 text-xs text-muted-foreground">{{ t('workload.project.subtitle') }}</p>
          </div>
          <div v-if="report.by_project.length === 0" class="py-6 text-center text-sm text-muted-foreground">
            {{ t('workload.project.empty') }}
          </div>
          <ul v-else class="space-y-2">
            <li v-for="p in report.by_project" :key="p.project_path" class="space-y-1">
              <div class="flex items-baseline justify-between gap-2 text-xs">
                <Tooltip>
                  <TooltipTrigger as-child>
                    <span class="truncate font-medium">{{ projectLabel(p) }}</span>
                  </TooltipTrigger>
                  <TooltipContent>
                    <span class="text-xs">{{ p.project_path }}</span>
                  </TooltipContent>
                </Tooltip>
                <span class="shrink-0 tabular-nums text-muted-foreground">{{ formatNumber(p.sessions) }}</span>
              </div>
              <div class="h-1.5 w-full overflow-hidden rounded-full bg-muted">
                <div
                  class="h-full rounded-full bg-primary transition-all"
                  :style="{ width: projectBarPercent(p) + '%' }"
                ></div>
              </div>
            </li>
          </ul>
        </div>
      </section>
    </template>
  </div>
</template>
