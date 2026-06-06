<script setup lang="ts">
import { computed, ref } from 'vue'
import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { RefreshCw } from '@lucide/vue'
import IdeDot from '@/components/shell/IdeDot.vue'
import { workload, habitHeatmap, type Adapter } from '@/mock/data'

const range = ref<'7d' | '30d' | '90d'>('30d')

const trendKpi = computed(() => {
  const sliceCount = range.value === '7d' ? 7 : range.value === '30d' ? 30 : 90
  const slice = workload.slice(-Math.min(sliceCount, workload.length))
  const sessions = slice.reduce((a, w) => a + w.count, 0)
  const messages = sessions * 23
  const active = slice.filter((w) => w.count > 0).length
  const peakIdx = slice.reduce((acc, w, i) => (w.count > slice[acc].count ? i : acc), 0)
  const peak = slice[peakIdx]
  return {
    sessions,
    messages,
    active,
    total: sliceCount,
    peakDate: peak?.date.slice(5),
    peakCount: peak?.count,
  }
})

const calendarColumns = computed(() => {
  if (!workload.length) return [] as (typeof workload)[]
  const cols: (typeof workload)[] = []
  const first = new Date(workload[0].date)
  const startDow = first.getDay()
  let cur: typeof workload = new Array(7).fill(null) as never
  for (let i = 0; i < startDow; i++) cur[i] = null as never
  for (const cell of workload) {
    const d = new Date(cell.date).getDay()
    if (d === 0 && cur.some((x) => x)) {
      cols.push(cur)
      cur = new Array(7).fill(null) as never
    }
    cur[d] = cell
  }
  if (cur.some((x) => x)) cols.push(cur)
  return cols
})

const calIntensity = (count: number) => {
  if (count === 0) return 'color-mix(in oklab, var(--foreground) 4%, transparent)'
  const opacity = Math.min(1, 0.2 + count * 0.13)
  return `color-mix(in oklab, var(--foreground) ${opacity * 100}%, transparent)`
}
const habitIntensity = (v: number) => {
  if (v === 0) return 'color-mix(in oklab, var(--foreground) 4%, transparent)'
  const opacity = Math.min(1, 0.2 + v * 0.15)
  return `color-mix(in oklab, var(--foreground) ${opacity * 100}%, transparent)`
}

interface AdapterUsage {
  id: Adapter
  label: string
  count: number
  pct: number
}
const adapterUsage: AdapterUsage[] = [
  { id: 'cursor', label: 'Cursor', count: 524, pct: 42 },
  { id: 'claude_code', label: 'Claude Code', count: 418, pct: 34 },
  { id: 'opencode', label: 'OpenCode', count: 198, pct: 16 },
  { id: 'codex', label: 'Codex', count: 94, pct: 8 },
]

const projectUsage = [
  { name: 'memex', count: 428, pct: 100 },
  { name: 'tt-projects', count: 312, pct: 73 },
  { name: 'async-pilot', count: 186, pct: 43 },
  { name: 'mms-cli', count: 142, pct: 33 },
  { name: 'ai-hub-cli', count: 98, pct: 23 },
]
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
      <Button variant="outline" size="sm" class="h-8 gap-1.5">
        <RefreshCw class="size-3.5" />
        刷新
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

    <Card class="mb-5 p-5">
      <div class="mb-3 flex items-end justify-between">
        <div>
          <h3 class="text-[14px] font-semibold">活动日历</h3>
          <p class="text-[11px] text-muted-foreground">
            过去 {{ Math.ceil(workload.length / 7) }} 周每日会话数
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
        <tbody>
          <tr v-for="wi in 7" :key="wi - 1">
            <td class="pr-1.5 text-right text-[10px] text-muted-foreground">
              <span v-if="(wi - 1) % 2 === 0">
                {{ ['周日', '周一', '周二', '周三', '周四', '周五', '周六'][wi - 1] }}
              </span>
            </td>
            <td
              v-for="(col, ci) in calendarColumns"
              :key="ci"
              class="p-0"
              style="height: 16px"
            >
              <div
                v-if="col[wi - 1]"
                class="h-full w-full rounded-sm transition-colors hover:ring-2 hover:ring-primary/40"
                :style="{ background: calIntensity(col[wi - 1].count) }"
                :title="`${col[wi - 1].date}: ${col[wi - 1].count}`"
              />
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
            <div
              v-for="(v, hi) in row"
              :key="hi"
              class="h-3 flex-1 rounded-sm"
              :style="{ background: habitIntensity(v) }"
              :title="`${hi} 时: ${v} 个会话`"
            />
          </div>
        </div>
        <div class="flex items-center gap-1 pt-1">
          <span class="w-8" />
          <div class="flex flex-1 gap-[2px]">
            <span
              v-for="h in 24"
              :key="h"
              class="flex-1 text-center text-[9px] text-muted-foreground"
            >
              {{ (h - 1) % 3 === 0 ? String(h - 1).padStart(2, '0') : '' }}
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
