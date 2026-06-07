<script setup lang="ts">
import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { ActivitySquare } from 'lucide-vue-next'
</script>

<template>
  <section>
    <div class="mb-3">
      <div class="flex items-center gap-2">
        <ActivitySquare class="size-3.5" :style="{ color: 'var(--adapter-codex)' }" />
        <h2 class="text-[15px] font-semibold">MCP 工具与活动</h2>
        <Badge variant="outline">Coming Soon</Badge>
      </div>
      <p class="mt-0.5 text-[11px] text-muted-foreground">实时观察你的 AI 在用 Memex 做什么</p>
    </div>

    <Card class="flex flex-col items-center gap-1.5 p-8 text-center">
      <ActivitySquare class="size-10 text-muted-foreground/30" />
      <p class="text-[13px] font-medium">MCP 调用日志即将上线</p>
      <p class="max-w-md text-[11px] text-muted-foreground">
        将展示 AI 通过 MCP 调用 Memex 的工具列表、24 小时调用次数、平均延迟和实时事件流（数据来源：daemon 的
        <code class="font-mono">mcp_call_log</code> 表）
      </p>
    </Card>
  </section>
</template>
