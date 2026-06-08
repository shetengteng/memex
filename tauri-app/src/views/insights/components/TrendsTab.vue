<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { RefreshCw } from 'lucide-vue-next'
import IdeDot from '@/components/shell/IdeDot.vue'
import { ADAPTER_MAP } from '@/stores/memex'
import type { WorkloadReport } from '@/types'
import { useMemex } from '@/composables/useMemex'
import DailyBarChart from './DailyBarChart.vue'

const memex = useMemex()
const range = ref<'7d' | '30d' | '90d'>('30d')
const data = ref<WorkloadReport | null>(null)
const loading = ref(false)

async function load() {
  loading.value = true
  try {
    const days = range.value === '7d' ? 7 : range.value === '30d' ? 30 : 90
    data.value = await memex.getWorkload(days)
  } catch (e) {
    console.warn('[TrendsTab] getWorkload failed', e)
    data.value = null
  } finally {
    loading.value = false
  }
}

onMounted(load)
watch(range, load)

const days = computed(() => (range.value === '7d' ? 7 : range.value === '30d' ? 30 : 90))

const trendKpi = computed(() => {
  if (!data.value) {
    return { sessions: 0, messages: 0, active: 0, total: days.value, peakDate: '—', peakCount: 0 }
  }
  return {
    sessions: data.value.overall.sessions,
    messages: data.value.overall.messages,
    active: data.value.overall.active_days,
    total: days.value,
    peakDate: data.value.overall.peak_day?.slice(5) ?? '—',
    peakCount: data.value.overall.peak_day_sessions,
  }
})

// 把后端 daily 数组（按日期升序）排成日历列：每列 7 行（周日~周六）
interface CalCell { date: string; count: number }
const calendarColumns = computed<(CalCell | null)[][]>(() => {
  const daily = data.value?.daily ?? []
  if (!daily.length) return []
  const cols: (CalCell | null)[][] = []
  const first = new Date(daily[0].date)
  const startDow = first.getDay() // 0=Sun..6=Sat
  let cur: (CalCell | null)[] = new Array(7).fill(null)
  for (let i = 0; i < startDow; i++) cur[i] = null
  for (const d of daily) {
    const dow = new Date(d.date).getDay()
    if (dow === 0 && cur.some((x) => x)) {
      cols.push(cur)
      cur = new Array(7).fill(null)
    }
    cur[dow] = { date: d.date, count: d.sessions }
  }
  if (cur.some((x) => x)) cols.push(cur)
  return cols
})

// GitHub 风格的月份标签：每列代表一周，若该周内出现某月的第 1~7 天则在那一列上方标 "M月"。
// 同一个月只在它首次出现的那一列标，避免重复堆叠。
const calendarMonthLabels = computed<string[]>(() => {
  const cols = calendarColumns.value
  const labels: string[] = new Array(cols.length).fill('')
  let lastMonth = -1
  for (let ci = 0; ci < cols.length; ci++) {
    for (let wi = 0; wi < 7; wi++) {
      const cell = cols[ci][wi]
      if (!cell) continue
      const m = new Date(cell.date).getMonth()
      const day = new Date(cell.date).getDate()
      // 进入新月份 + 该周 day<=7（保证标签贴在该月首列），才标
      if (m !== lastMonth && day <= 7) {
        labels[ci] = `${m + 1}月`
        lastMonth = m
        break
      }
    }
  }
  return labels
})

const calIntensity = (count: number) => {
  if (count === 0) return 'color-mix(in oklab, var(--foreground) 4%, transparent)'
  const opacity = Math.min(1, 0.2 + count * 0.13)
  return `color-mix(in oklab, var(--foreground) ${opacity * 100}%, transparent)`
}

// 把 yyyy-mm-dd 转成 "周X · M月D日" 的中文友好格式
function formatCalendarDate(iso: string): string {
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return iso
  const w = ['周日', '周一', '周二', '周三', '周四', '周五', '周六'][d.getDay()]
  return `${w} · ${d.getMonth() + 1}月${d.getDate()}日`
}
const habitIntensity = (v: number) => {
  if (v === 0) return 'color-mix(in oklab, var(--foreground) 4%, transparent)'
  const opacity = Math.min(1, 0.2 + v * 0.15)
  return `color-mix(in oklab, var(--foreground) ${opacity * 100}%, transparent)`
}

// 后端 heatmap 是 weekday × hour 稀疏 cell。转成 7×24 二维矩阵供模板渲染。
// 后端 weekday 0=Mon..6=Sun；模板想要 Sun..Sat（di 0..6 对应 周日..周六）
const habitHeatmap = computed<number[][]>(() => {
  const matrix: number[][] = Array.from({ length: 7 }, () => Array(24).fill(0))
  for (const c of data.value?.heatmap ?? []) {
    // backend weekday 0=Mon..6=Sun → 0=Sun..6=Sat: Mon(0)→1, Sun(6)→0
    const dow = c.weekday === 6 ? 0 : c.weekday + 1
    matrix[dow][c.hour] = c.sessions
  }
  return matrix
})

const adapterUsage = computed(() => {
  const xs = data.value?.by_adapter ?? []
  const max = Math.max(1, ...xs.map((x) => x.sessions))
  return xs
    .filter((x) => x.sessions > 0)
    .map((x) => ({
      id: x.key,
      label: ADAPTER_MAP[x.key]?.label ?? x.key,
      count: x.sessions,
      pct: Math.round((x.sessions / max) * 100),
    }))
    .sort((a, b) => b.count - a.count)
    .slice(0, 8)
})

const projectUsage = computed(() => {
  const xs = data.value?.by_project ?? []
  const max = Math.max(1, ...xs.map((x) => x.sessions))
  return xs
    .filter((x) => x.sessions > 0)
    .map((x) => ({
      name: x.name || x.project_path,
      count: x.sessions,
      pct: Math.round((x.sessions / max) * 100),
    }))
    .sort((a, b) => b.count - a.count)
    .slice(0, 10)
})
</script>

<template>
  <div class="mx-auto w-full max-w-6xl px-4 py-4 lg:px-6 lg:py-6">
    <div class="mb-4 flex items-center justify-between">
      <Tabs v-model="range">
        <TabsList class="h-8">
          <TabsTrigger value="7d" class="gap-1 text-[12px]">近 7 天</TabsTrigger>
          <TabsTrigger value="30d" class="gap-1 text-[12px]">近 30 天</TabsTrigger>
          <TabsTrigger value="90d" class="gap-1 text-[12px]">近 90 天</TabsTrigger>
        </TabsList>
      </Tabs>
      <Button variant="outline" size="sm" class="h-8 gap-1.5" :disabled="loading" @click="load">
        <RefreshCw :class="['size-3.5', loading && 'animate-spin']" />
        {{ loading ? '加载中…' : '刷新' }}
      </Button>
    </div>

    <div class="mb-6 grid grid-cols-2 gap-4 lg:grid-cols-4">
      <Card class="p-4">
        <div class="mb-1 text-[11px] tracking-wider text-muted-foreground">会话总数</div>
        <div class="text-2xl font-bold tabular-nums">{{ trendKpi.sessions.toLocaleString() }}</div>
      </Card>
      <Card class="p-4">
        <div class="mb-1 text-[11px] tracking-wider text-muted-foreground">消息总数</div>
        <div class="text-2xl font-bold tabular-nums">{{ trendKpi.messages.toLocaleString() }}</div>
      </Card>
      <Card class="p-4">
        <div class="mb-1 text-[11px] tracking-wider text-muted-foreground">活跃天数</div>
        <div class="flex items-baseline gap-2">
          <span class="text-2xl font-bold tabular-nums">{{ trendKpi.active }}</span>
          <span class="text-[12px] text-muted-foreground">/ {{ trendKpi.total }} 天</span>
        </div>
      </Card>
      <Card class="p-4">
        <div class="mb-1 text-[11px] tracking-wider text-muted-foreground">峰值日</div>
        <div class="text-[15px] font-semibold tabular-nums">
          {{ trendKpi.peakDate }} · {{ trendKpi.peakCount }}
        </div>
      </Card>
    </div>

    <DailyBarChart
      class="mb-5"
      :daily="data?.daily ?? []"
      :days="days"
    />

    <Card class="mb-5 p-5">
      <div class="mb-3 flex items-end justify-between">
        <div>
          <h3 class="text-[14px] font-semibold">活动日历</h3>
          <p class="text-[11px] text-muted-foreground">
            过去 {{ days }} 天每日会话数
          </p>
        </div>
        <div class="flex items-center gap-1.5 text-[10px] text-muted-foreground">
          <span>少</span>
          <span
            class="size-3 rounded-sm"
            style="background: color-mix(in oklab, var(--foreground) 4%, transparent)"
          />
          <span
            class="size-3 rounded-sm"
            style="background: color-mix(in oklab, var(--foreground) 30%, transparent)"
          />
          <span
            class="size-3 rounded-sm"
            style="background: color-mix(in oklab, var(--foreground) 50%, transparent)"
          />
          <span
            class="size-3 rounded-sm"
            style="background: color-mix(in oklab, var(--foreground) 75%, transparent)"
          />
          <span class="size-3 rounded-sm" style="background: var(--primary)" />
          <span>多</span>
        </div>
      </div>
      <table
        class="w-full"
        style="border-collapse: separate; border-spacing: 3px; table-layout: fixed"
      >
        <colgroup>
          <col style="width: 2rem" />
          <col v-for="(_, ci) in calendarColumns" :key="ci" />
        </colgroup>
        <thead>
          <!-- 月份标签：每列对应一周，遇到新月份的第一周在该列上方标 "M月" -->
          <tr>
            <th class="p-0" style="height: 14px" />
            <th
              v-for="(label, ci) in calendarMonthLabels"
              :key="ci"
              class="p-0 text-left align-bottom text-[10px] text-muted-foreground"
              style="height: 14px"
            >
              {{ label }}
            </th>
          </tr>
        </thead>
        <tbody>
          <!-- 纵坐标显示完整 7 个周几标签（之前只显示周一/三/五，用户反馈"缺少周一周三等" -->
          <tr v-for="wi in 7" :key="wi - 1">
            <td class="pr-1.5 text-right text-[10px] text-muted-foreground">
              {{ ['周日', '周一', '周二', '周三', '周四', '周五', '周六'][wi - 1] }}
            </td>
            <td
              v-for="(col, ci) in calendarColumns"
              :key="ci"
              class="p-0"
              style="height: 16px"
            >
              <Tooltip v-if="col[wi - 1]" :delay-duration="80">
                <TooltipTrigger as-child>
                  <div
                    class="h-full w-full cursor-default rounded-sm transition-colors hover:ring-2 hover:ring-primary/40"
                    :style="{ background: calIntensity(col[wi - 1]!.count) }"
                  />
                </TooltipTrigger>
                <TooltipContent side="top" :side-offset="4" class="px-2.5 py-1.5">
                  <div class="text-[11px] leading-tight">
                    <div class="font-medium">{{ formatCalendarDate(col[wi - 1]!.date) }}</div>
                    <div class="mt-0.5 tabular-nums text-muted-foreground">
                      {{ col[wi - 1]!.count }} 个会话
                    </div>
                  </div>
                </TooltipContent>
              </Tooltip>
            </td>
          </tr>
        </tbody>
      </table>
    </Card>

    <Card class="mb-5 p-5">
      <h3 class="mb-1 text-[14px] font-semibold">小时 × 星期 习惯图</h3>
      <p class="mb-3 text-[11px] text-muted-foreground">看看你最高产的时段</p>

      <div class="space-y-1">
        <div v-for="(row, di) in habitHeatmap" :key="di" class="flex items-center gap-1">
          <span class="w-8 text-right text-[10px] text-muted-foreground">
            {{ ['周日', '周一', '周二', '周三', '周四', '周五', '周六'][di] }}
          </span>
          <div class="flex flex-1 gap-[2px]">
            <Tooltip v-for="(v, hi) in row" :key="hi" :delay-duration="80">
              <TooltipTrigger as-child>
                <div
                  class="h-3 flex-1 cursor-default rounded-sm transition-colors hover:ring-2 hover:ring-primary/40"
                  :style="{ background: habitIntensity(v) }"
                />
              </TooltipTrigger>
              <TooltipContent side="top" :side-offset="4" class="px-2.5 py-1.5">
                <div class="text-[11px] leading-tight">
                  <div class="font-medium">
                    {{ ['周日', '周一', '周二', '周三', '周四', '周五', '周六'][di] }}
                    <span class="tabular-nums">{{ String(hi).padStart(2, '0') }}:00</span>
                  </div>
                  <div class="mt-0.5 tabular-nums text-muted-foreground">
                    {{ v }} 个会话
                  </div>
                </div>
              </TooltipContent>
            </Tooltip>
          </div>
        </div>
        <div class="flex items-center gap-1 pt-1">
          <span class="w-8" />
          <div class="flex flex-1 gap-[2px]">
            <span
              v-for="h in 24"
              :key="h"
              class="flex-1 text-center text-[9px] tabular-nums"
              :class="
                (h - 1) % 6 === 0
                  ? 'font-medium text-muted-foreground'
                  : 'text-muted-foreground/50'
              "
            >
              {{ String(h - 1).padStart(2, '0') }}
            </span>
          </div>
        </div>
      </div>
    </Card>

    <div class="grid grid-cols-1 gap-4 lg:grid-cols-2">
      <Card class="p-5">
        <h3 class="mb-1 text-[14px] font-semibold">工具使用</h3>
        <p class="mb-3 text-[11px] text-muted-foreground">按会话数排序</p>
        <ul class="space-y-3">
          <li v-for="t in adapterUsage" :key="t.id">
            <div class="mb-1 flex items-baseline justify-between text-[12px]">
              <span class="flex items-center gap-1.5 font-medium">
                <IdeDot :adapter="t.id" />
                {{ t.label }}
              </span>
              <span class="text-muted-foreground tabular-nums">
                {{ t.count }} ({{ t.pct }}%)
              </span>
            </div>
            <div class="h-1.5 w-full overflow-hidden rounded-full bg-muted">
              <div
                class="h-full rounded-full"
                :style="{
                  width: t.pct + '%',
                  background: `var(--adapter-${t.id.replace('_code', '')})`,
                }"
              />
            </div>
          </li>
        </ul>
      </Card>

      <Card class="p-5">
        <h3 class="mb-1 text-[14px] font-semibold">项目 Top 10</h3>
        <p class="mb-3 text-[11px] text-muted-foreground">按会话数排序</p>
        <ul class="space-y-3">
          <li v-for="p in projectUsage" :key="p.name">
            <div class="mb-1 flex items-baseline justify-between text-[12px]">
              <span class="truncate font-medium">{{ p.name }}</span>
              <Badge variant="outline" class="tabular-nums">{{ p.count }}</Badge>
            </div>
            <div class="h-1.5 w-full overflow-hidden rounded-full bg-muted">
              <div class="h-full rounded-full bg-primary" :style="{ width: p.pct + '%' }" />
            </div>
          </li>
        </ul>
      </Card>
    </div>
  </div>
</template>
