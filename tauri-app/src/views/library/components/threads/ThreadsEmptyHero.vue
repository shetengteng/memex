<script setup lang="ts">
/**
 * 完全没有线索时显示的搜索引擎风格 hero。
 * 5 个建议词点击就触发关键词检索，省去用户构思 query 的成本。
 */
import { Badge } from '@/components/ui/badge'
import { Sparkles } from 'lucide-vue-next'

const SUGGESTIONS = [
  'Tauri 多窗口',
  '会话摘要 prompt',
  'memex 桌面化',
  'cursor 适配器',
  'LLM 节流',
]

defineEmits<{ apply: [string] }>()
</script>

<template>
  <div class="flex h-full min-h-[60vh] items-center justify-center px-6 py-12">
    <div class="w-full max-w-lg text-center">
      <div class="mx-auto flex size-14 items-center justify-center rounded-full bg-primary/10">
        <Sparkles class="size-6 text-primary" />
      </div>
      <h3 class="mt-4 text-[16px] font-semibold">从主题开始检索</h3>
      <p class="mx-auto mt-2 max-w-md text-[12.5px] text-muted-foreground">
        输入一个关键词或问题，让本地 LLM 从你最近 80 个有摘要的会话里挑出相关的，
        组成一条「线索」。每条线索都会保留下来，方便你下次回顾。
      </p>
      <div class="mt-6 flex flex-wrap items-center justify-center gap-2">
        <Badge
          v-for="s in SUGGESTIONS"
          :key="s"
          variant="outline"
          class="cursor-pointer rounded-full px-3 py-1 text-[11.5px] hover:border-primary hover:text-foreground"
          @click="$emit('apply', s)"
        >
          {{ s }}
        </Badge>
      </div>
      <p class="mt-6 text-[11px] text-muted-foreground/70">
        或者点上方<span class="mx-1 font-medium text-foreground">全量聚类</span>让 LLM 自动归纳所有主题。
      </p>
    </div>
  </div>
</template>
