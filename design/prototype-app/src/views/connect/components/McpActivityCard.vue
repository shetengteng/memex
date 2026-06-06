<script setup lang="ts">
import { computed } from 'vue'
import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import {
  Activity,
  ActivitySquare,
  ArrowRight,
  FileText,
} from '@lucide/vue'
import IdeChip from '@/components/shell/IdeChip.vue'
import { ADAPTER_MAP, mcpTools, mcpCallEvents } from '@/mock/data'

const totalCalls24h = computed(() => mcpTools.reduce((a, t) => a + t.calls24h, 0))
const maxCalls = computed(() => Math.max(...mcpTools.map((t) => t.calls24h), 1))
</script>

<template>
  <section>
    <div class="mb-3 flex items-end justify-between">
      <div>
        <div class="flex items-center gap-2">
          <ActivitySquare class="size-3.5" :style="{ color: 'var(--adapter-codex)' }" />
          <h2 class="text-[15px] font-semibold">MCP 工具与活动</h2>
          <Badge variant="outline">Beta</Badge>
        </div>
        <p class="mt-0.5 text-[11px] text-muted-foreground">实时观察你的 AI 在用 Memex 做什么</p>
      </div>
      <Button variant="ghost" size="sm" class="h-8 gap-1.5">
        <FileText class="size-3.5" />
        实时日志
      </Button>
    </div>

    <div class="mb-4 grid grid-cols-1 gap-3 sm:grid-cols-2">
      <Card v-for="t in mcpTools" :key="t.name" class="p-4">
        <div class="mb-2 flex items-center justify-between">
          <div>
            <div class="font-mono text-[13px] font-semibold">{{ t.name }}</div>
            <div class="text-[11px] text-muted-foreground">{{ t.description }}</div>
          </div>
          <Badge
            v-if="t.live"
            class="border-emerald-500/30 bg-emerald-500/10 text-emerald-700"
          >
            ● 在线
          </Badge>
        </div>
        <div class="flex items-baseline justify-between text-[12px]">
          <span>近 24 小时调用</span>
          <span class="text-[16px] font-semibold tabular-nums">{{ t.calls24h }}</span>
        </div>
        <div class="mt-2 h-1.5 w-full overflow-hidden rounded-full bg-muted">
          <div
            class="h-full rounded-full bg-primary"
            :style="{ width: (t.calls24h / maxCalls) * 100 + '%' }"
          />
        </div>
        <div class="mt-2 text-[10px] text-muted-foreground">
          平均延迟 {{ t.avgLatencyMs }} ms
          <span v-if="t.pctMaxLatency"> · 99 分位 &lt; {{ t.pctMaxLatency }} ms</span>
        </div>
      </Card>
    </div>

    <Card class="overflow-hidden p-0">
      <div class="flex items-center justify-between border-b px-4 py-2.5">
        <span class="text-[11px] font-semibold tracking-wider text-muted-foreground">
          最近调用
        </span>
        <span class="flex items-center gap-1 text-[10px] text-muted-foreground">
          <Activity class="size-3" />
          实时流 · 共 {{ totalCalls24h }} 次
        </span>
      </div>
      <div class="font-mono text-[11px]">
        <div
          v-for="(e, i) in mcpCallEvents"
          :key="i"
          class="flex items-center gap-3 px-4 py-1.5"
          :class="i < mcpCallEvents.length - 1 && 'border-b'"
        >
          <span class="text-muted-foreground">{{ e.ts }}</span>
          <IdeChip :adapter="e.client" :label="ADAPTER_MAP[e.client].label.toLowerCase()" />
          <span>{{ e.tool }}</span>
          <span class="truncate text-muted-foreground">{{ e.args }}</span>
          <Badge class="ml-auto border-emerald-500/30 bg-emerald-500/10 text-emerald-700">
            → {{ e.resultSummary }} · {{ e.latencyMs }} ms
          </Badge>
        </div>
      </div>
      <Separator />
      <div class="flex items-center justify-between px-4 py-3">
        <span class="text-[11px] text-muted-foreground">数据来自 daemon 的 mcp_call_log 表</span>
        <Button variant="link" size="sm" class="h-auto p-0 text-[12px]">
          查看实时日志
          <ArrowRight class="ml-1 size-3" />
        </Button>
      </div>
    </Card>
  </section>
</template>
