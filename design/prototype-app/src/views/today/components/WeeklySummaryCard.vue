<script setup lang="ts">
import { useRouter } from 'vue-router'
import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ArrowRight, BrainCircuit } from '@lucide/vue'

const router = useRouter()

const weekly = {
  count: '67',
  messages: '1,892',
  projects: 4,
  body: `主线在做 Memex 菜单栏 shadcn 重构：确认 7 tab → 5 tab 收敛，引入 Sidebar block 与 ⌘K 命令面板。同期推进 tt-projects 的 AsyncMQ 集成方案。`,
  decisions: [
    '用 Sidebar 区块替代手写导航，节省约 120 行代码',
    '把"会话"和"项目"合并到"资料库"，支持多视图切换',
    '搜索统一到全局 ⌘K，消除弹窗/仪表盘双实现',
  ],
  topics: ['ui', 'shadcn', 'refactor', '+5'],
}
</script>

<template>
  <Card class="flex flex-col p-5">
    <div class="mb-3 flex items-center justify-between">
      <div class="flex items-center gap-2">
        <BrainCircuit class="size-4" :style="{ color: 'var(--adapter-claude)' }" />
        <h3 class="text-[14px] font-semibold">本周自动摘要</h3>
        <Badge variant="secondary">L3</Badge>
      </div>
      <span class="text-[11px] text-muted-foreground">第 23 周</span>
    </div>

    <p class="mb-3 text-[13px] text-muted-foreground">
      {{ weekly.count }} 个会话 · {{ weekly.messages }} 条消息 · 跨 {{ weekly.projects }} 个项目
    </p>

    <p class="mb-4 text-[13px] leading-relaxed">{{ weekly.body }}</p>

    <div class="mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
      关键决策
    </div>
    <ul class="mb-4 space-y-1.5 text-[13px]">
      <li v-for="d in weekly.decisions" :key="d" class="flex gap-2">
        <span class="mt-2 size-1 shrink-0 rounded-full bg-primary" />
        <span>{{ d }}</span>
      </li>
    </ul>

    <div class="mt-auto flex items-center justify-between border-t pt-3">
      <div class="flex flex-wrap items-center gap-1.5">
        <Badge v-for="t in weekly.topics" :key="t" variant="secondary">{{ t }}</Badge>
      </div>
      <Button variant="ghost" size="sm" class="h-7 gap-1 text-xs" @click="router.push('/insights')">
        完整摘要
        <ArrowRight class="size-3" />
      </Button>
    </div>
  </Card>
</template>
