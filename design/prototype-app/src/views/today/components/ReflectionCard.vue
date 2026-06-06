<script setup lang="ts">
import { useRouter } from 'vue-router'
import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ArrowRight, Lightbulb, Plus } from '@lucide/vue'

const router = useRouter()

const reflectionItems = [
  {
    title: '上周反思未读',
    chip: '12 条摘要',
    body: '"shadcn 重构 + MCP 接入 + AsyncMQ 集成的协同关系…"',
    dashed: false,
    icon: ArrowRight,
  },
  {
    title: '5 月反思未生成',
    chip: undefined,
    body: '累计 312 个会话 · 让 AI 帮你回顾一下',
    dashed: true,
    icon: Plus,
  },
]
</script>

<template>
  <Card class="flex flex-col p-5">
    <div class="mb-3 flex items-center justify-between">
      <div class="flex items-center gap-2">
        <Lightbulb class="size-4" :style="{ color: 'var(--warning)' }" />
        <h3 class="text-[14px] font-semibold">等待你的反思</h3>
        <Badge variant="secondary">L4</Badge>
      </div>
      <Badge class="border-amber-500/30 bg-amber-500/10 text-amber-700">2 项</Badge>
    </div>

    <div class="mb-4 space-y-2">
      <button
        v-for="(item, idx) in reflectionItems"
        :key="idx"
        :class="[
          'group w-full rounded-lg border p-3 text-left transition-colors hover:bg-accent',
          item.dashed && 'border-dashed',
        ]"
      >
        <div class="mb-1 flex items-center justify-between">
          <span class="text-[13px] font-semibold">{{ item.title }}</span>
          <Badge v-if="item.chip" variant="outline">{{ item.chip }}</Badge>
          <component v-else :is="item.icon" class="size-3.5 text-muted-foreground" />
        </div>
        <p class="truncate text-[12px] text-muted-foreground">{{ item.body }}</p>
      </button>
    </div>

    <div class="mt-auto flex items-center justify-between border-t pt-3">
      <span class="text-[11px] text-muted-foreground">最近反思 2026-06-06 09:12</span>
      <Button variant="ghost" size="sm" class="h-7 gap-1 text-xs" @click="router.push('/insights')">
        全部反思
        <ArrowRight class="size-3" />
      </Button>
    </div>
  </Card>
</template>
