<script setup lang="ts">
import { computed } from 'vue'
import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Separator } from '@/components/ui/separator'
import { Activity } from '@lucide/vue'
import IdeChip from '@/components/shell/IdeChip.vue'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { todayActivity } from '@/mock/data'

interface BarStyle {
  height: string
  background: string
  count: number
  hour: string
  isPeak: boolean
}

const maxBar = Math.max(...todayActivity.hourlyBars, 1)
const peakHour = todayActivity.hourlyBars.indexOf(maxBar)

const bars = computed<BarStyle[]>(() =>
  todayActivity.hourlyBars.map((v, idx) => {
    const ratio = v / maxBar
    const hour = `${String(idx).padStart(2, '0')}:00`
    const isPeak = idx === peakHour && v > 0
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

const stats = [
  { value: todayActivity.sessions, label: '会话' },
  { value: todayActivity.messages, label: '消息' },
  { value: todayActivity.projects, label: '项目' },
  { value: todayActivity.toolsUsed, label: '工具' },
]

const today6 = '06-06'
</script>

<template>
  <Card class="p-5">
    <div class="mb-3 flex items-center justify-between">
      <div class="flex items-center gap-2">
        <Activity class="size-4 text-primary" />
        <h2 class="text-[15px] font-semibold">你今天的活动</h2>
      </div>
      <Badge variant="outline">{{ today6 }}</Badge>
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
      <div>
        最活跃 <span class="font-medium text-foreground">{{ todayActivity.peakWindow }}</span> · 主要在
        <span v-for="(p, i) in todayActivity.byProject.slice(0, 2)" :key="p.name">
          <span class="font-medium text-foreground">{{ p.name }}</span> ({{ p.sessions }})<span
            v-if="i === 0"
            >&nbsp;/&nbsp;</span
          >
        </span>
      </div>
      <div class="flex items-center gap-2">
        <IdeChip adapter="cursor" label="Cursor 8" />
        <IdeChip adapter="claude_code" label="Claude 3" />
        <IdeChip adapter="codex" label="Codex 1" />
      </div>
    </div>
  </Card>
</template>
