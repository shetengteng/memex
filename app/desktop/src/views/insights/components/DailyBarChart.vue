<script setup lang="ts">
import { computed } from 'vue'
import { Card } from '@/components/ui/card'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import type { WorkloadDailyEntry } from '@/types'

const props = defineProps<{
  daily: WorkloadDailyEntry[]
  days: number
}>()

// 兼容稀疏后端数据：补齐 N 天，没有数据的那天 sessions = 0
const filled = computed<WorkloadDailyEntry[]>(() => {
  if (!props.daily.length) return []
  const map = new Map(props.daily.map((d) => [d.date, d]))
  const result: WorkloadDailyEntry[] = []
  const end = new Date(props.daily[props.daily.length - 1].date)
  for (let i = props.days - 1; i >= 0; i--) {
    const d = new Date(end)
    d.setDate(end.getDate() - i)
    const key = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
    result.push(
      map.get(key) ?? { date: key, sessions: 0, messages: 0 },
    )
  }
  return result
})

const maxSessions = computed(() =>
  filled.value.reduce((m, d) => Math.max(m, d.sessions), 0),
)

const totalSessions = computed(() =>
  filled.value.reduce((s, d) => s + d.sessions, 0),
)

const totalMessages = computed(() =>
  filled.value.reduce((s, d) => s + d.messages, 0),
)

// 选择 ticks：开头、中点、结尾各放一个。其余看密度。
const xTicks = computed<{ index: number; label: string }[]>(() => {
  const xs = filled.value
  if (!xs.length) return []
  const fmt = (iso: string) => {
    const d = new Date(iso)
    return `${d.getMonth() + 1}/${d.getDate()}`
  }
  const len = xs.length
  if (len <= 7) {
    return xs.map((d, i) => ({ index: i, label: fmt(d.date) }))
  }
  if (len <= 31) {
    const step = Math.ceil(len / 6)
    return xs
      .map((d, i) => ({ index: i, label: fmt(d.date) }))
      .filter((t) => t.index % step === 0 || t.index === len - 1)
  }
  const step = Math.ceil(len / 8)
  return xs
    .map((d, i) => ({ index: i, label: fmt(d.date) }))
    .filter((t) => t.index % step === 0 || t.index === len - 1)
})

function barHeightPct(v: number): number {
  if (maxSessions.value <= 0) return 0
  return Math.max(2, Math.round((v / maxSessions.value) * 100))
}

function tooltipDate(iso: string): string {
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return iso
  const wk = ['周日', '周一', '周二', '周三', '周四', '周五', '周六'][d.getDay()]
  return `${wk} · ${d.getMonth() + 1}月${d.getDate()}日`
}
</script>

<template>
  <Card class="p-5">
    <div class="mb-3 flex items-end justify-between">
      <div>
        <h3 class="text-[14px] font-semibold">每日趋势</h3>
        <p class="text-[11px] text-muted-foreground">
          过去 {{ days }} 天 · 共 {{ totalSessions.toLocaleString() }} 个会话 ·
          {{ totalMessages.toLocaleString() }} 条消息
        </p>
      </div>
      <div class="text-[10px] text-muted-foreground">
        峰值 {{ maxSessions }} / 日
      </div>
    </div>

    <div v-if="!filled.length" class="py-8 text-center text-[12px] text-muted-foreground">
      暂无数据
    </div>

    <template v-else>
      <div class="relative flex h-40 items-end gap-[2px] border-b border-border/40">
        <Tooltip
          v-for="(d, i) in filled"
          :key="d.date"
          :delay-duration="80"
        >
          <TooltipTrigger as-child>
            <div
              class="group relative flex-1 cursor-default rounded-t-sm transition-colors"
              :style="{
                height: `${barHeightPct(d.sessions)}%`,
                background: d.sessions > 0 ? 'var(--primary)' : 'color-mix(in oklab, var(--foreground) 4%, transparent)',
                opacity: d.sessions > 0 ? Math.max(0.4, d.sessions / Math.max(1, maxSessions)) : 0.4,
              }"
            />
          </TooltipTrigger>
          <TooltipContent side="top" :side-offset="4" class="px-2.5 py-1.5">
            <div class="text-[11px] leading-tight">
              <div class="font-medium">{{ tooltipDate(d.date) }}</div>
              <div class="mt-0.5 tabular-nums text-muted-foreground">
                {{ d.sessions }} 会话 · {{ d.messages }} 消息
              </div>
            </div>
          </TooltipContent>
        </Tooltip>
      </div>

      <div class="mt-1.5 flex justify-between text-[10px] tabular-nums text-muted-foreground">
        <span v-for="t in xTicks" :key="t.index">{{ t.label }}</span>
      </div>
    </template>
  </Card>
</template>
