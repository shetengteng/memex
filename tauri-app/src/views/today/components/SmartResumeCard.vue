<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { Card } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import {
  Archive,
  ArrowUpRight,
  Send,
  Settings2,
  Zap,
} from 'lucide-vue-next'
import IdeChip from '@/components/shell/IdeChip.vue'
import { sessions } from '@/stores/memex'

const router = useRouter()

// 后端暂未暴露"未完成/被中断"信号，先把最近 3 条 session 当候选
const resumeCandidates = computed(() => sessions.slice(0, 3))

const fromNow = (iso: string) => {
  if (!iso) return '—'
  const diff = (Date.now() - new Date(iso).getTime()) / 1000
  if (diff < 60) return `${Math.floor(diff)} 秒前`
  if (diff < 3600) return `${Math.floor(diff / 60)} 分钟前`
  if (diff < 86400) return `${Math.floor(diff / 3600)} 小时前`
  return `${Math.floor(diff / 86400)} 天前`
}

function openSession(id: string) {
  router.push(`/library?session=${id}`)
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
        <!-- 第 1 行：标题 + IDE 标签。把 project·time 从这里挪走，给标题更多空间。-->
        <div class="mb-1 flex items-baseline justify-between gap-3">
          <span class="truncate text-[13px] font-semibold">{{ s.title }}</span>
          <IdeChip :adapter="s.adapter" class="shrink-0" />
        </div>
        <!-- 第 2 行：会话指标，纯 muted 文本 -->
        <p class="mb-2 text-[12px] text-muted-foreground">
          {{ s.messages }} 条消息 · {{ s.adapter }}
        </p>
        <!-- 第 3 行：按钮（左）+ project·time（右）。左右布局，右侧贴边。-->
        <div class="flex items-center gap-1.5">
          <Button size="sm" variant="outline" class="h-7 gap-1 text-xs" @click="openSession(s.id)">
            <ArrowUpRight class="size-3" />
            打开会话
          </Button>
          <Button size="sm" variant="ghost" class="h-7 gap-1 text-xs" disabled>
            <Send class="size-3" />
            发送到 IDE
          </Button>
          <Button size="sm" variant="ghost" class="h-7 gap-1 text-xs" disabled>
            <Archive class="size-3" />
            归档
          </Button>
          <span class="ml-auto truncate text-[11px] text-muted-foreground">
            {{ s.project }} · {{ fromNow(s.startedAt) }}
          </span>
        </div>
      </article>
      <p v-if="!resumeCandidates.length" class="text-center text-[12px] italic text-muted-foreground">
        暂无可继续的会话
      </p>
    </div>
  </Card>
</template>
