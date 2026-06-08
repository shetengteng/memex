<script setup lang="ts">
import { Badge } from '@/components/ui/badge'
import IdeChip from '@/components/shell/IdeChip.vue'
import { Check, ChevronRight, Clock, MessageCircle } from 'lucide-vue-next'
import type { Session } from '@/stores/memex'

defineProps<{
  session: Session
  groupKey: string
  active: boolean
}>()
defineEmits<{ open: [Session] }>()

const groupFmt = (iso: string, group: string) => {
  const d = new Date(iso)
  const hm = d.toLocaleTimeString('zh-CN', {
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  })
  if (group === 'today' || group === 'yesterday') return hm
  if (group === 'week') {
    const day = ['周日', '周一', '周二', '周三', '周四', '周五', '周六'][d.getDay()]
    return `${day} ${hm}`
  }
  return `${d.getMonth() + 1}/${d.getDate()} ${hm}`
}
</script>

<template>
  <button
    :data-active="active"
    class="group relative flex w-full items-start border-b border-border/60 py-3.5 pl-5 pr-5 text-left transition-colors last:border-b-0 hover:bg-accent/40 data-[active=true]:bg-accent/40"
    :class="!session.l2Done && session.messages === 0 && 'opacity-60'"
    @click="$emit('open', session)"
  >
    <div class="min-w-0 flex-1">
      <div class="mb-1 flex items-baseline justify-between gap-3">
        <span class="truncate text-[14px] font-semibold tracking-tight">{{ session.title }}</span>
        <IdeChip :adapter="session.adapter" class="shrink-0" />
      </div>
      <!-- intent 为空时直接不渲染整行，避免列表里出现一长串占位的 '—' 视觉噪声（用户反馈截图） -->
      <p
        v-if="session.intent && session.intent.trim()"
        class="mb-2 truncate text-[12.5px] text-muted-foreground/90"
      >
        {{ session.intent }}
      </p>
      <div class="flex items-center gap-2">
        <div class="flex min-w-0 flex-1 flex-wrap items-center gap-1.5">
          <Badge
            variant="secondary"
            class="h-5 gap-1 bg-muted/70 px-1.5 font-normal text-muted-foreground"
          >
            <MessageCircle class="size-2.5" />
            <span class="tabular-nums">{{ session.messages }}</span>
          </Badge>
          <Badge
            variant="secondary"
            class="h-5 gap-1 bg-muted/70 px-1.5 font-normal text-muted-foreground"
          >
            <Clock class="size-2.5" />
            <span class="tabular-nums">{{ session.durationMin }}m</span>
          </Badge>
          <Badge
            v-if="session.l2Done"
            variant="outline"
            class="h-5 gap-1 border-emerald-500/30 bg-emerald-500/5 px-1.5 font-normal text-emerald-700 dark:text-emerald-400"
          >
            <Check class="size-2.5" />
            L2 已生成
          </Badge>
          <Badge
            v-else
            variant="outline"
            class="h-5 gap-1 border-amber-500/40 bg-amber-500/5 px-1.5 font-normal text-amber-700 dark:text-amber-500"
          >
            <Clock class="size-2.5" />
            L2 待生成
          </Badge>
          <template v-if="session.topics.length">
            <span class="mx-0.5 size-1 shrink-0 rounded-full bg-border" />
            <Badge
              v-for="t in session.topics.slice(0, 3)"
              :key="t"
              variant="outline"
              class="h-5 px-1.5 font-normal text-muted-foreground"
            >
              {{ t }}
            </Badge>
            <span
              v-if="session.topics.length > 3"
              class="text-[10px] tabular-nums text-muted-foreground"
            >
              +{{ session.topics.length - 3 }}
            </span>
          </template>
        </div>
        <span class="shrink-0 truncate text-[11px] tabular-nums text-muted-foreground/80">
          {{ session.project }} · {{ groupFmt(session.startedAt, groupKey) }}
        </span>
      </div>
    </div>
    <ChevronRight
      class="mt-1.5 ml-2 size-4 shrink-0 text-muted-foreground/50 transition-all group-hover:translate-x-0.5 group-hover:text-muted-foreground group-data-[active=true]:translate-x-0.5 group-data-[active=true]:text-foreground"
    />
  </button>
</template>
