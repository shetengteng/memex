<script setup lang="ts">
import { computed } from 'vue'
import { Card } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import {
  Archive,
  ArrowUpRight,
  Send,
  Settings2,
  Zap,
} from '@lucide/vue'
import IdeChip from '@/components/shell/IdeChip.vue'
import { sessions } from '@/mock/data'

const resumeCandidates = computed(() =>
  sessions.filter((s) => !!s.interruptedAt).slice(0, 3),
)

const fromNow = (iso: string) => {
  const diff = (Date.now() - new Date(iso).getTime()) / 1000
  if (diff < 60) return `${Math.floor(diff)} 秒前`
  if (diff < 3600) return `${Math.floor(diff / 60)} 分钟前`
  if (diff < 86400) return `${Math.floor(diff / 3600)} 小时前`
  return `${Math.floor(diff / 86400)} 天前`
}
</script>

<template>
  <Card class="p-5">
    <div class="mb-4 flex items-center justify-between">
      <div class="flex items-center gap-2">
        <Zap class="size-4" :style="{ color: 'var(--adapter-codex)' }" />
        <h3 class="text-[14px] font-semibold">接着想想？</h3>
        <span class="text-[11px] text-muted-foreground">智能续接你的近期会话</span>
      </div>
      <Button variant="ghost" size="sm" class="h-7 gap-1 text-xs">
        <Settings2 class="size-3" />
        规则
      </Button>
    </div>

    <div class="space-y-2">
      <article
        v-for="s in resumeCandidates"
        :key="s.id"
        class="rounded-lg border p-3"
      >
        <div class="mb-1 flex items-baseline justify-between gap-3">
          <div class="flex min-w-0 items-baseline gap-2">
            <span class="truncate text-[13px] font-semibold">{{ s.title }}</span>
            <span class="shrink-0 text-[11px] text-muted-foreground">
              {{ s.project }} · {{ fromNow(s.startedAt) }}
            </span>
          </div>
          <IdeChip :adapter="s.adapter" class="shrink-0" />
        </div>
        <p class="mb-2 text-[12px] text-muted-foreground">
          <span class="font-medium" :style="{ color: 'var(--warning)' }">中断点</span>：{{ s.interruptedAt }}
        </p>
        <div class="flex items-center gap-1.5">
          <Button size="sm" variant="outline" class="h-7 gap-1 text-xs">
            <ArrowUpRight class="size-3" />
            继续会话
          </Button>
          <Button size="sm" variant="ghost" class="h-7 gap-1 text-xs">
            <Send class="size-3" />
            发送到 IDE
          </Button>
          <Button size="sm" variant="ghost" class="h-7 gap-1 text-xs">
            <Archive class="size-3" />
            归档
          </Button>
        </div>
      </article>
    </div>
  </Card>
</template>
