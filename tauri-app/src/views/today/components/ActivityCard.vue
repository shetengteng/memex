<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Separator } from '@/components/ui/separator'
import { Activity } from 'lucide-vue-next'
import IdeChip from '@/components/shell/IdeChip.vue'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { useMemex } from '@/composables/useMemex'
import type { WorkloadReport } from '@/types'

interface BarStyle {
  height: string
  background: string
  count: number
  hour: string
  isPeak: boolean
}

const memex = useMemex()
const workload = ref<WorkloadReport | null>(null)

// 今天的 weekday（0=Mon ... 6=Sun，匹配后端 WorkloadHeatmapCell.weekday）
function todayWeekday(): number {
  // JS getDay(): 0=Sun..6=Sat; 后端: 0=Mon..6=Sun
  const js = new Date().getDay()
  return js === 0 ? 6 : js - 1
}

// 把后端 heatmap 中今天这一行 24 列抽成 hourlyBars
const hourlyBars = computed<number[]>(() => {
  if (!workload.value) return Array(24).fill(0)
  const wd = todayWeekday()
  const out = Array(24).fill(0)
  for (const cell of workload.value.heatmap) {
    if (cell.weekday === wd) out[cell.hour] = cell.sessions
  }
  return out
})

const maxBar = computed(() => Math.max(...hourlyBars.value, 1))
const peakHour = computed(() => hourlyBars.value.indexOf(maxBar.value))

const bars = computed<BarStyle[]>(() =>
  hourlyBars.value.map((v, idx) => {
    const ratio = v / maxBar.value
    const hour = `${String(idx).padStart(2, '0')}:00`
    const isPeak = idx === peakHour.value && v > 0
    if (v === 0)
      return { height: '8%', background: 'var(--muted)', count: 0, hour, isPeak: false }
    if (isPeak)
      return { height: '100%', background: 'var(--primary)', count: v, hour, isPeak: true }
    const opacity = 0.25 + ratio * 0.6
    return {
      height: `${Math.max(12, ratio * 95)}%`,
      background: `color-mix(in oklab, var(--foreground) ${opacity * 100}%, transparent)`,
      count: v,
      hour,
      isPeak: false,
    }
  }),
)

// 今日合计
const todayDaily = computed(() =>
  workload.value?.daily.find((d) => d.date === today10()) ?? { date: '', sessions: 0, messages: 0 },
)
function today10(): string {
  const d = new Date()
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}

// 今日活跃项目数 = by_project 中 sessions>0 的项目数（getWorkload(1) 只统计今天）
const todayProjects = computed(() => workload.value?.by_project.filter((p) => p.sessions > 0).length ?? 0)
const todayAdapters = computed(() => workload.value?.by_adapter.filter((a) => a.sessions > 0).length ?? 0)

const stats = computed(() => [
  { value: todayDaily.value.sessions, label: '会话' },
  { value: todayDaily.value.messages, label: '消息' },
  { value: todayProjects.value, label: '项目' },
  { value: todayAdapters.value, label: '工具' },
])

// peak window：[peakHour, peakHour+2]
const peakWindow = computed(() => {
  if (maxBar.value <= 1) return '—'
  const p = peakHour.value
  const a = String(p).padStart(2, '0')
  const b = String((p + 2) % 24).padStart(2, '0')
  return `${a}:00 – ${b}:00`
})

// 项目 Top3 用于尾部"主要在 ... ("
const topProjects = computed(() =>
  (workload.value?.by_project ?? [])
    .filter((p) => p.sessions > 0)
    .sort((a, b) => b.sessions - a.sessions)
    .slice(0, 2)
    .map((p) => ({ name: p.name || p.project_path, sessions: p.sessions })),
)

// adapter top 3 chip
const topAdapters = computed(() =>
  (workload.value?.by_adapter ?? [])
    .filter((a) => a.sessions > 0)
    .sort((a, b) => b.sessions - a.sessions)
    .slice(0, 3)
    .map((a) => ({ key: a.key, sessions: a.sessions })),
)

const todayBadge = computed(() => today10().slice(5)) // MM-DD

onMounted(async () => {
  try {
    workload.value = await memex.getWorkload(1)
  } catch (e) {
    console.warn('[ActivityCard] getWorkload(1) failed', e)
  }
})
</script>

<template>
  <Card class="p-5">
    <div class="mb-3 flex items-center justify-between">
      <div class="flex items-center gap-2">
        <Activity class="size-4 text-primary" />
        <h2 class="text-[15px] font-semibold">你今天的活动</h2>
      </div>
      <Badge variant="outline">{{ todayBadge }}</Badge>
    </div>

    <div class="flex items-end justify-between px-6">
      <div class="flex h-12 shrink-0 items-end gap-[2px]">
        <Tooltip v-for="(b, i) in bars" :key="i" :delay-duration="80">
          <TooltipTrigger as-child>
            <div
              class="h-full w-1.5 cursor-default rounded-sm transition-opacity hover:opacity-70"
              :style="{ height: b.height, background: b.background }"
            />
          </TooltipTrigger>
          <TooltipContent side="top" :side-offset="6" class="px-2.5 py-1.5">
            <div class="text-[11px] leading-tight">
              <div class="flex items-center gap-1.5">
                <span class="font-medium tabular-nums">{{ b.hour }}</span>
                <span
                  v-if="b.isPeak"
                  class="rounded-sm bg-primary/15 px-1 text-[9px] font-semibold tracking-wide text-primary"
                >峰值</span>
              </div>
              <div class="mt-0.5 tabular-nums text-muted-foreground">
                {{ b.count }} 个会话
              </div>
            </div>
          </TooltipContent>
        </Tooltip>
      </div>

      <div v-for="s in stats" :key="s.label" class="shrink-0">
        <div class="text-2xl font-bold tabular-nums">{{ s.value }}</div>
        <div class="text-[11px] text-muted-foreground">{{ s.label }}</div>
      </div>
    </div>

    <Separator />

    <div class="flex items-center justify-between text-[12px] text-muted-foreground">
      <div v-if="topProjects.length">
        最活跃 <span class="font-medium text-foreground">{{ peakWindow }}</span> · 主要在
        <span v-for="(p, i) in topProjects" :key="p.name">
          <span class="font-medium text-foreground">{{ p.name }}</span> ({{ p.sessions }})<span
            v-if="i < topProjects.length - 1"
            >&nbsp;/&nbsp;</span
          >
        </span>
      </div>
      <div v-else class="italic">今天还没有会话</div>
      <div class="flex items-center gap-2">
        <IdeChip v-for="a in topAdapters" :key="a.key" :adapter="a.key" :label="`${a.key} ${a.sessions}`" />
      </div>
    </div>
  </Card>
</template>
