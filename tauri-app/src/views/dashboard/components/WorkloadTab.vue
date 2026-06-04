<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { RefreshCw, Activity } from 'lucide-vue-next'
import { useMemex } from '@/composables/useMemex'
import { useI18n } from '@/i18n'
import { adapterLabel, formatNumber } from '@/lib/utils'
import IdeIcon from '@/components/IdeIcon.vue'
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import type { WorkloadReport, WorkloadHeatmapCell, WorkloadBucket, WorkloadProjectBucket, WorkloadDailyEntry } from '@/types'

const { t } = useI18n()
const { getWorkload } = useMemex()

type Range = '7' | '30' | '90'
const days = ref<Range>('30')
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
    report.value = await getWorkload(Number(days.value))
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

// 7 × 24 满阵列；后端给的是 sparse 输出
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

// log1p 拉伸长尾色阶
function heatColor(sessions: number): string {
  if (sessions <= 0) return 'transparent'
  if (heatmapMax.value <= 0) return 'transparent'
  const t = Math.log1p(sessions) / Math.log1p(heatmapMax.value)
  const opacity = Math.max(0.12, Math.min(1, t))
  return `hsl(217 91% 60% / ${opacity})`
}

// ---- 日历视图（GitHub 贡献图风格） ----
// 列 = 周，行 = weekday(Mon..Sun)，每格 = 一天

function localDateKey(d: Date): string {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}

function toIsoWeekday(d: Date): number {
  // JS: 0=Sunday..6=Saturday → ISO 0=Monday..6=Sunday
  return (d.getDay() + 6) % 7
}

interface CalendarCell {
  date: string
  inRange: boolean
  sessions: number
  messages: number
}

// 固定列数：覆盖 90d 所需的最大周数，短时间段在左侧留白，避免不同 range 块数视觉不一致
const CALENDAR_COLUMNS = 14

// 返回 { columns: CalendarCell[7][], monthLabels: { col, label }[] }
const calendarData = computed(() => {
  if (!report.value) return { columns: [] as CalendarCell[][], monthLabels: [] as { col: number; label: string }[] }
  const days = report.value.days
  const map = new Map<string, WorkloadDailyEntry>()
  for (const d of report.value.daily) map.set(d.date, d)

  const today = new Date()
  today.setHours(0, 0, 0, 0)
  const rangeStart = new Date(today)
  rangeStart.setDate(today.getDate() - (days - 1))

  // 以 today 所在周为最后一列，向前固定推 CALENDAR_COLUMNS 周
  const gridEnd = new Date(today)
  gridEnd.setDate(gridEnd.getDate() + (6 - toIsoWeekday(gridEnd)))
  const gridStart = new Date(gridEnd)
  gridStart.setDate(gridStart.getDate() - (CALENDAR_COLUMNS * 7 - 1))

  const columns: CalendarCell[][] = []
  const monthLabels: { col: number; label: string }[] = []
  const monthFmt = new Intl.DateTimeFormat(undefined, { month: 'short' })

  const cursor = new Date(gridStart)
  let lastMonth = -1
  for (let col = 0; col < CALENDAR_COLUMNS; col++) {
    const week: CalendarCell[] = []
    for (let w = 0; w < 7; w++) {
      const key = localDateKey(cursor)
      const entry = map.get(key)
      const inRange = cursor >= rangeStart && cursor <= today
      week.push({
        date: key,
        inRange,
        sessions: inRange ? (entry?.sessions ?? 0) : 0,
        messages: inRange ? (entry?.messages ?? 0) : 0,
      })
      cursor.setDate(cursor.getDate() + 1)
    }
    const colStart = new Date(gridStart)
    colStart.setDate(gridStart.getDate() + col * 7)
    if (colStart.getMonth() !== lastMonth) {
      monthLabels.push({ col, label: monthFmt.format(colStart) })
      lastMonth = colStart.getMonth()
    }
    columns.push(week)
  }
  return { columns, monthLabels }
})

const calendarMax = computed(() => {
  if (!report.value) return 0
  return report.value.daily.reduce((m, d) => Math.max(m, d.sessions), 0)
})

function calendarColor(cell: CalendarCell): string {
  if (!cell.inRange) return 'transparent'
  if (cell.sessions <= 0) return 'hsl(220 9% 90% / 0.4)' // 空格子：极淡的灰
  if (calendarMax.value <= 0) return 'transparent'
  const t = Math.log1p(cell.sessions) / Math.log1p(calendarMax.value)
  const opacity = Math.max(0.18, Math.min(1, t))
  return `hsl(217 91% 60% / ${opacity})`
}

const calendarHasData = computed(() => {
  if (!report.value) return false
  return report.value.daily.length > 0
})

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

function onRangeUpdate(val: unknown) {
  // ToggleGroup type="single" 反取消时会传 '' / null / undefined；忽略空值，保持当前选中。
  if (val === undefined || val === null || val === '') return
  if (Array.isArray(val)) return
  days.value = String(val) as Range
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
        <ToggleGroup
          type="single"
          variant="outline"
          size="sm"
          :model-value="days"
          @update:model-value="onRangeUpdate"
        >
          <ToggleGroupItem value="7">{{ t('workload.range.7d') }}</ToggleGroupItem>
          <ToggleGroupItem value="30">{{ t('workload.range.30d') }}</ToggleGroupItem>
          <ToggleGroupItem value="90">{{ t('workload.range.90d') }}</ToggleGroupItem>
        </ToggleGroup>
        <Button variant="outline" size="sm" :disabled="loading" @click="load">
          <RefreshCw class="h-3.5 w-3.5" :class="loading ? 'animate-spin' : ''" />
          {{ t('workload.refresh') }}
        </Button>
      </div>
    </div>

    <!-- Error -->
    <Card v-if="error" class="border-destructive/40 bg-destructive/10">
      <CardContent class="py-3 text-sm text-destructive">
        {{ error }}
      </CardContent>
    </Card>

    <!-- First-load skeleton -->
    <Card v-if="loading && !report">
      <CardContent class="py-8 text-center text-sm text-muted-foreground">
        {{ t('workload.loading') }}
      </CardContent>
    </Card>

    <!-- Empty -->
    <Card v-else-if="isEmpty">
      <CardContent class="space-y-1 py-10 text-center">
        <p class="text-base font-medium">{{ t('workload.empty.title') }}</p>
        <p class="text-sm text-muted-foreground">{{ t('workload.empty.hint') }}</p>
      </CardContent>
    </Card>

    <template v-else-if="report">
      <!-- KPI row -->
      <section class="grid grid-cols-2 gap-4 md:grid-cols-4">
        <Card>
          <CardHeader>
            <CardTitle>{{ t('workload.overall.sessions') }}</CardTitle>
          </CardHeader>
          <CardContent>
            <div class="text-3xl font-bold tabular-nums">{{ formatNumber(overall!.sessions) }}</div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>{{ t('workload.overall.messages') }}</CardTitle>
          </CardHeader>
          <CardContent>
            <div class="text-3xl font-bold tabular-nums">{{ formatNumber(overall!.messages) }}</div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>{{ t('workload.overall.active_days') }}</CardTitle>
          </CardHeader>
          <CardContent>
            <div class="flex items-baseline gap-2">
              <span class="text-3xl font-bold tabular-nums">{{ overall!.active_days }}</span>
              <span class="text-xs text-muted-foreground">/ {{ report.days }}</span>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>{{ t('workload.overall.peak_day') }}</CardTitle>
          </CardHeader>
          <CardContent>
            <div v-if="overall!.peak_day" class="text-sm font-semibold tabular-nums">
              {{ t('workload.overall.peak_day_value', { date: overall!.peak_day, n: overall!.peak_day_sessions }) }}
            </div>
            <div v-else class="text-sm text-muted-foreground">—</div>
          </CardContent>
        </Card>
      </section>

      <!-- Calendar (GitHub-style) -->
      <Card>
        <CardHeader>
          <CardTitle>{{ t('workload.calendar.title') }}</CardTitle>
          <p class="text-xs text-muted-foreground">{{ t('workload.calendar.subtitle', { n: report.days }) }}</p>
        </CardHeader>
        <CardContent>
          <div v-if="!calendarHasData" class="py-6 text-center text-sm text-muted-foreground">
            {{ t('workload.calendar.empty') }}
          </div>
          <div v-else class="space-y-1.5">
            <table class="w-full text-xs" style="border-collapse: separate; border-spacing: 3px; table-layout: fixed">
              <colgroup>
                <col style="width: 2rem" />
                <col v-for="(_, ci) in calendarData.columns" :key="ci" />
              </colgroup>
              <thead>
                <tr>
                  <th></th>
                  <th
                    v-for="(_, ci) in calendarData.columns"
                    :key="ci"
                    class="text-left font-normal text-muted-foreground"
                  >
                    <span v-if="calendarData.monthLabels.find((m) => m.col === ci)">
                      {{ calendarData.monthLabels.find((m) => m.col === ci)!.label }}
                    </span>
                  </th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="wi in 7" :key="wi - 1">
                  <td class="pr-1.5 text-right text-muted-foreground">
                    <span v-if="(wi - 1) % 2 === 0">{{ weekdayLabel(wi - 1) }}</span>
                  </td>
                  <td
                    v-for="(col, ci) in calendarData.columns"
                    :key="ci"
                    class="p-0"
                    style="height: 16px"
                  >
                    <Tooltip v-if="col[wi - 1].inRange">
                      <TooltipTrigger as-child>
                        <div
                          class="h-full w-full rounded-sm border border-border/30 transition-colors hover:ring-2 hover:ring-primary"
                          :style="{ backgroundColor: calendarColor(col[wi - 1]) }"
                        />
                      </TooltipTrigger>
                      <TooltipContent>
                        <div class="text-xs">
                          <div class="font-semibold">{{ col[wi - 1].date }}</div>
                          <div>{{ formatNumber(col[wi - 1].sessions) }} sessions · {{ formatNumber(col[wi - 1].messages) }} msgs</div>
                        </div>
                      </TooltipContent>
                    </Tooltip>
                    <div v-else class="h-full w-full" />
                  </td>
                </tr>
              </tbody>
            </table>
            <!-- legend -->
            <div class="flex items-center justify-end gap-1.5 pt-1 text-[10px] text-muted-foreground">
              <span>{{ t('workload.calendar.less') }}</span>
              <span class="inline-block h-2.5 w-2.5 rounded-sm border border-border/30" style="background: hsl(220 9% 90% / 0.4)"></span>
              <span class="inline-block h-2.5 w-2.5 rounded-sm border border-border/30" style="background: hsl(217 91% 60% / 0.3)"></span>
              <span class="inline-block h-2.5 w-2.5 rounded-sm border border-border/30" style="background: hsl(217 91% 60% / 0.55)"></span>
              <span class="inline-block h-2.5 w-2.5 rounded-sm border border-border/30" style="background: hsl(217 91% 60% / 0.8)"></span>
              <span class="inline-block h-2.5 w-2.5 rounded-sm border border-border/30" style="background: hsl(217 91% 60% / 1)"></span>
              <span>{{ t('workload.calendar.more') }}</span>
            </div>
          </div>
        </CardContent>
      </Card>

      <!-- Heatmap (weekday × hour habits) -->
      <Card>
        <CardHeader>
          <CardTitle>{{ t('workload.heatmap.title') }}</CardTitle>
          <p class="text-xs text-muted-foreground">{{ t('workload.heatmap.subtitle', { n: report.days }) }}</p>
        </CardHeader>
        <CardContent>
          <div v-if="heatmapMax === 0" class="py-6 text-center text-sm text-muted-foreground">
            {{ t('workload.heatmap.empty') }}
          </div>
          <table v-else class="w-full text-xs" style="border-collapse: separate; border-spacing: 2px; table-layout: fixed">
            <colgroup>
              <col style="width: 2.5rem" />
              <col v-for="h in 24" :key="h - 1" />
            </colgroup>
            <thead>
              <tr>
                <th></th>
                <th
                  v-for="h in 24"
                  :key="h - 1"
                  class="text-center font-normal text-muted-foreground"
                >
                  <span v-if="(h - 1) % 3 === 0">{{ hourLabel(h - 1) }}</span>
                </th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(row, wi) in heatmapGrid" :key="wi">
                <td class="pr-2 text-right text-muted-foreground">{{ weekdayLabel(wi) }}</td>
                <td v-for="cell in row" :key="cell.hour" class="p-0" style="height: 18px">
                  <Tooltip>
                    <TooltipTrigger as-child>
                      <div
                        class="h-full w-full rounded-sm border border-border/30 transition-colors hover:ring-2 hover:ring-primary"
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
        </CardContent>
      </Card>

      <!-- 2-col: Adapter + Project -->
      <section class="grid gap-4 md:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>{{ t('workload.adapter.title') }}</CardTitle>
            <p class="text-xs text-muted-foreground">{{ t('workload.adapter.subtitle') }}</p>
          </CardHeader>
          <CardContent>
            <div v-if="report.by_adapter.length === 0" class="py-6 text-center text-sm text-muted-foreground">
              {{ t('workload.adapter.empty') }}
            </div>
            <ul v-else class="space-y-2.5">
              <li v-for="b in report.by_adapter" :key="b.key" class="space-y-1">
                <div class="flex items-baseline justify-between gap-2 text-xs">
                  <span class="flex items-center gap-1.5 font-medium">
                    <IdeIcon :source="b.key" class="h-3.5 w-3.5 shrink-0" />
                    {{ adapterLabel(b.key) }}
                  </span>
                  <div class="flex items-center gap-1.5">
                    <Badge variant="outline" class="tabular-nums">{{ formatNumber(b.sessions) }}</Badge>
                    <span class="text-xs tabular-nums text-muted-foreground">{{ adapterPercent(b).toFixed(1) }}%</span>
                  </div>
                </div>
                <div class="h-1.5 w-full overflow-hidden rounded-full bg-muted">
                  <div
                    class="h-full rounded-full transition-all"
                    :style="{ width: adapterPercent(b) + '%', backgroundColor: adapterColor(b.key) }"
                  ></div>
                </div>
              </li>
            </ul>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>{{ t('workload.project.title') }}</CardTitle>
            <p class="text-xs text-muted-foreground">{{ t('workload.project.subtitle') }}</p>
          </CardHeader>
          <CardContent>
            <div v-if="report.by_project.length === 0" class="py-6 text-center text-sm text-muted-foreground">
              {{ t('workload.project.empty') }}
            </div>
            <ul v-else class="space-y-2.5">
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
                  <Badge variant="outline" class="shrink-0 tabular-nums">{{ formatNumber(p.sessions) }}</Badge>
                </div>
                <div class="h-1.5 w-full overflow-hidden rounded-full bg-muted">
                  <div
                    class="h-full rounded-full bg-primary transition-all"
                    :style="{ width: projectBarPercent(p) + '%' }"
                  ></div>
                </div>
              </li>
            </ul>
          </CardContent>
        </Card>
      </section>
    </template>
  </div>
</template>
