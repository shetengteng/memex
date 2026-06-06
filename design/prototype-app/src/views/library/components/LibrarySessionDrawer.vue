<script setup lang="ts">
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Sheet,
  SheetContent,
  SheetTitle,
  SheetDescription,
} from '@/components/ui/sheet'
import IdeChip from '@/components/shell/IdeChip.vue'
import {
  Archive,
  ExternalLink,
  Send,
  Trash2,
} from '@lucide/vue'
import type { Session } from '@/mock/data'

defineProps<{ session: Session | null; open: boolean }>()
defineEmits<{ 'update:open': [boolean] }>()

const tFmt = (iso: string) => {
  const d = new Date(iso)
  return d.toLocaleString('zh-CN', { dateStyle: 'short', timeStyle: 'short' })
}
</script>

<template>
  <Sheet :open="open" @update:open="(v) => $emit('update:open', v)">
    <SheetContent
      class="flex w-full flex-col overflow-hidden p-0 data-[side=right]:sm:max-w-[640px] data-[side=right]:lg:max-w-[720px]"
    >
      <header v-if="session" class="flex items-start gap-3 border-b px-5 py-4">
        <div class="min-w-0 flex-1">
          <div class="mb-1.5 flex items-center gap-2">
            <IdeChip :adapter="session.adapter" />
            <span class="text-[11px] text-muted-foreground">
              {{ session.project }} · {{ tFmt(session.startedAt) }}
            </span>
          </div>
          <SheetTitle class="text-[16px] font-semibold leading-tight">{{ session.title }}</SheetTitle>
          <SheetDescription class="mt-1 text-[12px]">
            {{ session.messages }} 条消息 · {{ session.durationMin }} 分钟 · L2 摘要
            {{ session.l2Done ? '已生成' : '待生成' }}
          </SheetDescription>
        </div>
      </header>

      <div v-if="session" class="flex-1 space-y-5 overflow-y-auto px-5 py-4">
        <section>
          <div class="mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
            LLM 摘要（L2）
          </div>
          <p class="text-[13px] leading-relaxed">{{ session.intent }}</p>
        </section>

        <section v-if="session.topics.length">
          <div class="mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
            主题
          </div>
          <div class="flex flex-wrap gap-1.5">
            <Badge v-for="t in session.topics" :key="t" variant="secondary">{{ t }}</Badge>
          </div>
        </section>

        <section v-if="session.decisions?.length">
          <div class="mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
            关键决策
          </div>
          <ul class="space-y-1.5 text-[13px]">
            <li v-for="d in session.decisions" :key="d" class="flex gap-2">
              <span class="mt-2 size-1 shrink-0 rounded-full bg-primary" />
              <span>{{ d }}</span>
            </li>
          </ul>
        </section>

        <section v-if="session.next?.length">
          <div class="mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
            下一步
          </div>
          <ul class="space-y-1.5 text-[13px]">
            <li v-for="n in session.next" :key="n" class="flex gap-2">
              <span class="mt-2 size-1 shrink-0 rounded-full bg-primary" />
              <span>{{ n }}</span>
            </li>
          </ul>
        </section>

        <section>
          <div class="mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
            最近消息预览
          </div>
          <div class="space-y-2">
            <div class="rounded-md bg-muted p-3">
              <div class="mb-1 text-[10px] font-semibold uppercase text-muted-foreground">用户</div>
              <p class="text-[12px]">先帮我给一个新的 ASCII 原型和功能设计</p>
            </div>
            <div class="rounded-md border p-3">
              <div
                class="mb-1 text-[10px] font-semibold uppercase"
                :style="{ color: 'var(--adapter-claude)' }"
              >
                助手
              </div>
              <p class="text-[12px]">收到。我先梳理一下当前项目的功能和页面结构…</p>
            </div>
          </div>
        </section>
      </div>

      <footer class="flex shrink-0 items-center gap-2 border-t px-5 py-3.5">
        <Button class="flex-1 gap-1.5">
          <ExternalLink class="size-3.5" />
          打开完整会话
        </Button>
        <Button variant="outline" size="sm" class="gap-1.5">
          <Send class="size-3.5" />
          发送到 IDE
        </Button>
        <Button variant="ghost" size="icon" class="size-9">
          <Archive class="size-4" />
        </Button>
        <Button variant="ghost" size="icon" class="size-9">
          <Trash2 class="size-4" />
        </Button>
      </footer>
    </SheetContent>
  </Sheet>
</template>
