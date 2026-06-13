<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import {
  ActivitySquare,
  AlertTriangle,
  CheckCircle2,
  Clock,
  Radio,
  TrendingUp,
  Wrench,
  XCircle,
} from 'lucide-vue-next'
import { useMemex } from '@/composables/useMemex'
import { humanizeBackendError } from '@/lib/utils'
import { useI18n } from '@/i18n'
import type { McpCallEntry, McpCallStats24h } from '@/types'
import McpCallDetailDialog from './McpCallDetailDialog.vue'
import { asMetric, formatFullTime, formatLatency, formatRelative, type RelativeLabels } from './mcp-format'

const { t } = useI18n()
const i18nRelative = computed<RelativeLabels>(() => ({
  justNow: t('connect.relative.just_now'),
  secondsAgo: (n) => t('connect.relative.seconds_ago', { n }),
  minutesAgo: (n) => t('connect.relative.minutes_ago', { n }),
  hoursAgo: (n) => t('connect.relative.hours_ago', { n }),
  daysAgo: (n) => t('connect.relative.days_ago', { n }),
}))

// 轮询间隔。3s 足以营造"实时"观感，又不会把 SQLite I/O 拉得很满
// （一次 stats + 一次 recent，本地 db 综合 < 10ms）。
const POLL_MS = 3_000
// 事件流上限。视觉密度考虑：100 条 × ~36px ≈ 3.6k px，scroll 内 240px 容器
// 够看了，再多不增加价值反而拖慢 diff。
const RECENT_LIMIT = 100
// 卡片视觉空间放得下的工具拆分行数。多出来折叠成 "+ N 个工具更多"。
const TOOL_BREAKDOWN_VISIBLE = 6

const memex = useMemex()
const stats = ref<McpCallStats24h>(emptyStats())
const recent = ref<McpCallEntry[]>([])
const loading = ref(true)
const errorMsg = ref<string | null>(null)
const tick = ref(0)
const selectedEntry = ref<McpCallEntry | null>(null)

let timer: ReturnType<typeof setInterval> | null = null
let tickTimer: ReturnType<typeof setInterval> | null = null

function emptyStats(): McpCallStats24h {
  return {
    total: 0,
    success: 0,
    failed: 0,
    avg_latency_ms: 0,
    by_tool: [],
    last_call_at: null,
  }
}

async function refresh() {
  try {
    const [s, r] = await Promise.all([
      memex.mcpCallStats24h(),
      memex.mcpRecentCalls(RECENT_LIMIT),
    ])
    stats.value = s
    recent.value = r
    errorMsg.value = null
  } catch (e) {
    // 留住上次成功的数据，只在 footer 显示错误，避免一次抖动就把整卡清空。
    errorMsg.value = humanizeBackendError(e).friendly
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  refresh()
  timer = setInterval(refresh, POLL_MS)
  // 1s 节拍只用来让 "X 分钟前" 这种相对时间跟着走，不触发 IPC。
  tickTimer = setInterval(() => {
    tick.value++
  }, 1_000)
})

onBeforeUnmount(() => {
  if (timer != null) clearInterval(timer)
  if (tickTimer != null) clearInterval(tickTimer)
})

const successRate = computed(() => {
  if (stats.value.total === 0) return null
  return (stats.value.success / stats.value.total) * 100
})

const successRateLabel = computed(() => {
  const v = successRate.value
  if (v == null) return '—'
  return `${v.toFixed(v >= 99.95 ? 0 : 1)}%`
})

// "实时" 状态色：全成功 → 绿；有失败 → 黄；近 60s 完全没动 → 灰。
const liveStatus = computed<'idle' | 'healthy' | 'degraded'>(() => {
  if (stats.value.last_call_at == null) return 'idle'
  const ms = Date.now() - new Date(stats.value.last_call_at).getTime()
  if (ms > 60_000 && stats.value.failed === 0) return 'idle'
  return stats.value.failed > 0 ? 'degraded' : 'healthy'
})

const liveStatusLabel = computed(() => {
  switch (liveStatus.value) {
    case 'healthy':
      return t('connect.mcp.live.healthy')
    case 'degraded':
      return t('connect.mcp.live.degraded')
    case 'idle':
      return t('connect.mcp.live.idle')
  }
  return t('connect.mcp.live.idle')
})

const lastCallRelative = computed(() => {
  // tick 是占位 dep —— 让 relative time 每秒重算。
  void tick.value
  if (stats.value.last_call_at == null) return null
  return formatRelative(new Date(stats.value.last_call_at), i18nRelative.value)
})

const avgLatencyLabel = computed(() => {
  if (stats.value.success === 0) return '—'
  const v = stats.value.avg_latency_ms
  if (v < 10) return `${v.toFixed(1)} ms`
  if (v < 1_000) return `${Math.round(v)} ms`
  return `${(v / 1_000).toFixed(2)} s`
})

const visibleTools = computed(() => stats.value.by_tool.slice(0, TOOL_BREAKDOWN_VISIBLE))
const moreToolCount = computed(() =>
  Math.max(0, stats.value.by_tool.length - TOOL_BREAKDOWN_VISIBLE),
)

const totalCallsForRatio = computed(() => stats.value.total || 1)
function widthRatio(count: number): string {
  const pct = (count / totalCallsForRatio.value) * 100
  return `${Math.max(4, Math.min(100, pct))}%`
}

</script>

<template>
  <section>
    <div class="mb-3 flex items-start justify-between">
      <div>
        <div class="flex items-center gap-2">
          <ActivitySquare class="size-3.5" :style="{ color: 'var(--adapter-codex)' }" />
          <h2 class="text-[15px] font-semibold">{{ t('connect.mcp.title') }}</h2>
          <Badge
            v-if="liveStatus === 'healthy'"
            variant="outline"
            class="gap-1 border-emerald-500/40 text-emerald-600 dark:text-emerald-400"
          >
            <span class="size-1.5 rounded-full bg-emerald-500 animate-pulse" />
            {{ liveStatusLabel }}
          </Badge>
          <Badge
            v-else-if="liveStatus === 'degraded'"
            variant="outline"
            class="gap-1 border-amber-500/40 text-amber-600 dark:text-amber-400"
          >
            <AlertTriangle class="size-2.5" />
            {{ liveStatusLabel }}
          </Badge>
          <Badge v-else variant="outline" class="gap-1">
            <Radio class="size-2.5 text-muted-foreground" />
            {{ liveStatusLabel }}
          </Badge>
        </div>
        <p class="mt-0.5 text-[11px] text-muted-foreground">
          {{ t('connect.mcp.subtitle') }}
        </p>
      </div>
      <span v-if="lastCallRelative" class="shrink-0 text-[11px] text-muted-foreground">
        {{ t('connect.mcp.last_call', { time: lastCallRelative }) }}
      </span>
    </div>

    <!-- 三联指标 -->
    <div class="mb-3 grid grid-cols-3 gap-2">
      <Card class="flex flex-col gap-1 p-3">
        <div class="flex items-center gap-1.5 text-[11px] text-muted-foreground">
          <TrendingUp class="size-3" />
          {{ t('connect.mcp.metric.calls_24h') }}
        </div>
        <div class="text-[20px] font-semibold leading-tight tabular-nums">
          {{ asMetric(stats.total) }}
        </div>
      </Card>
      <Card class="flex flex-col gap-1 p-3">
        <div class="flex items-center gap-1.5 text-[11px] text-muted-foreground">
          <CheckCircle2 class="size-3" />
          {{ t('connect.mcp.metric.success_rate') }}
        </div>
        <div class="text-[20px] font-semibold leading-tight tabular-nums">
          {{ successRateLabel }}
          <span
            v-if="stats.failed > 0"
            class="ml-1 align-middle text-[10px] font-normal text-amber-600 dark:text-amber-400"
          >
            {{ t('connect.mcp.metric.failed_count', { n: stats.failed }) }}
          </span>
        </div>
      </Card>
      <Card class="flex flex-col gap-1 p-3">
        <div class="flex items-center gap-1.5 text-[11px] text-muted-foreground">
          <Clock class="size-3" />
          {{ t('connect.mcp.metric.avg_latency') }}
        </div>
        <div class="text-[20px] font-semibold leading-tight tabular-nums">
          {{ avgLatencyLabel }}
        </div>
      </Card>
    </div>

    <!-- 主体两栏：左工具拆分，右事件流 -->
    <div class="grid grid-cols-1 gap-3 lg:grid-cols-5">
      <!-- 工具拆分 -->
      <Card class="flex flex-col gap-2 p-3 lg:col-span-2">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-1.5 text-[12px] font-medium">
            <Wrench class="size-3" />
            {{ t('connect.mcp.tools.title') }}
          </div>
          <span v-if="stats.by_tool.length > 0" class="text-[10px] text-muted-foreground tabular-nums">
            {{ t('connect.mcp.tools.count', { n: stats.by_tool.length }) }}
          </span>
        </div>
        <div v-if="loading && stats.by_tool.length === 0" class="space-y-1.5">
          <div v-for="i in 4" :key="i" class="h-7 animate-pulse rounded bg-muted/40" />
        </div>
        <div v-else-if="visibleTools.length === 0" class="py-3 text-center text-[11px] text-muted-foreground">
          {{ t('connect.mcp.tools.empty') }}
        </div>
        <ul v-else class="space-y-1.5">
          <li v-for="tool in visibleTools" :key="tool.tool_name" class="flex flex-col gap-0.5">
            <div class="flex items-baseline justify-between gap-2">
              <code class="truncate font-mono text-[11.5px]">{{ tool.tool_name }}</code>
              <span class="shrink-0 text-[10.5px] tabular-nums text-muted-foreground">
                {{ t('connect.mcp.tools.row_summary', { count: tool.count, latency: formatLatency(tool.avg_latency_ms) }) }}
              </span>
            </div>
            <div class="h-1 w-full overflow-hidden rounded-full bg-muted/40">
              <div
                class="h-full rounded-full"
                :style="{
                  width: widthRatio(tool.count),
                  backgroundColor: 'var(--adapter-codex)',
                  opacity: 0.55,
                }"
              />
            </div>
          </li>
          <li v-if="moreToolCount > 0" class="pt-1 text-center text-[10.5px] text-muted-foreground">
            {{ t('connect.mcp.tools.more', { n: moreToolCount }) }}
          </li>
        </ul>
      </Card>

      <!-- 事件流 -->
      <Card class="flex flex-col gap-2 p-3 lg:col-span-3">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-1.5 text-[12px] font-medium">
            <Radio class="size-3" />
            {{ t('connect.mcp.events.title') }}
          </div>
          <span class="text-[10px] text-muted-foreground tabular-nums">{{ t('connect.mcp.events.recent_n', { n: recent.length }) }}</span>
        </div>
        <div v-if="loading && recent.length === 0" class="space-y-1">
          <div v-for="i in 5" :key="i" class="h-6 animate-pulse rounded bg-muted/40" />
        </div>
        <div v-else-if="recent.length === 0" class="py-3 text-center text-[11px] text-muted-foreground">
          {{ t('connect.mcp.events.empty') }}
        </div>
        <ul v-else class="max-h-[260px] space-y-px overflow-y-auto pr-1 font-mono text-[11px]">
          <li
            v-for="ev in recent"
            :key="ev.id"
            class="flex cursor-pointer items-center gap-2 rounded px-1.5 py-1 transition-colors hover:bg-muted/60"
            :title="ev.error_message ?? t('connect.mcp.events.click_to_detail')"
            @click="selectedEntry = ev"
          >
            <span class="w-[132px] shrink-0 tabular-nums text-muted-foreground">
              {{ formatFullTime(ev.occurred_at) }}
            </span>
            <CheckCircle2
              v-if="ev.success"
              class="size-3 shrink-0 text-emerald-500"
            />
            <XCircle v-else class="size-3 shrink-0 text-rose-500" />
            <span class="flex-1 truncate">{{ ev.tool_name || t('connect.mcp.events.unknown_tool') }}</span>
            <span class="shrink-0 tabular-nums text-muted-foreground">
              {{ formatLatency(ev.latency_ms) }}
            </span>
          </li>
        </ul>
      </Card>
    </div>

    <p v-if="errorMsg" class="mt-2 text-[10.5px] text-rose-500">
      {{ t('connect.mcp.error_prefix', { err: errorMsg }) }}
    </p>

    <McpCallDetailDialog
      :entry="selectedEntry"
      @close="selectedEntry = null"
    />
  </section>
</template>
