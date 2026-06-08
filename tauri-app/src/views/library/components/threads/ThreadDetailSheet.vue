<script setup lang="ts">
/**
 * 线索详情侧栏 —— 主题摘要 / 活跃时间 / 涉及项目 / 适配器 / 会话列表 / 删除入口。
 *
 * 用 shadcn-vue `Sheet`（reka-ui Dialog 实现）+ 内层 `flex-1 min-h-0 overflow-y-auto`
 * 自己处理滚动，不复用 ScrollArea —— 后者在 DialogContent 的 fixed 上下文里
 * 高度链建立不起来，滚动会失效。
 */
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet'
import { FolderGit2, Loader2, Trash2 } from 'lucide-vue-next'
import type { SessionRow, ThreadRow } from '@/types'
import { adapterLabel, lastProjectName, timeFmt } from '../../composables/threadsFormat'

defineProps<{
  open: boolean
  thread: ThreadRow | null
  sessions: SessionRow[]
  loading: boolean
}>()

const emit = defineEmits<{
  'update:open': [boolean]
  delete: [ThreadRow]
  openSession: [SessionRow]
}>()
</script>

<template>
  <Sheet :open="open" @update:open="(v: boolean) => emit('update:open', v)">
    <SheetContent class="flex w-full flex-col p-0 sm:!max-w-md md:!max-w-lg">
      <SheetHeader class="border-b border-border/60 px-5 py-4">
        <div class="flex items-center gap-2 pr-8">
          <SheetTitle class="line-clamp-2 text-[15px] font-semibold">
            {{ thread?.name }}
          </SheetTitle>
        </div>
        <SheetDescription class="sr-only">线索详情</SheetDescription>
        <div class="flex flex-wrap items-center gap-2 pr-8 pt-1">
          <Badge variant="secondary" class="tabular-nums text-[10.5px]">
            {{ thread?.session_count ?? 0 }} 个会话
          </Badge>
          <Badge
            v-if="(thread?.projects?.length ?? 0) > 0"
            variant="outline"
            class="tabular-nums text-[10.5px]"
          >
            {{ thread?.projects?.length }} 个项目
          </Badge>
        </div>
      </SheetHeader>

      <div class="flex-1 min-h-0 overflow-y-auto">
        <div v-if="loading" class="flex h-40 items-center justify-center">
          <Loader2 class="size-4 animate-spin text-muted-foreground" />
        </div>

        <div v-else class="space-y-6 px-5 py-5">
          <section v-if="thread?.summary">
            <h4 class="text-[10.5px] font-medium uppercase tracking-wider text-muted-foreground">
              主题摘要
            </h4>
            <p class="mt-2 text-[12.5px] leading-relaxed">
              {{ thread.summary }}
            </p>
          </section>

          <section v-if="thread?.first_session_at && thread?.last_session_at">
            <h4 class="text-[10.5px] font-medium uppercase tracking-wider text-muted-foreground">
              活跃时间
            </h4>
            <div class="mt-2 flex items-center gap-2 text-[11.5px]">
              <span class="tabular-nums">{{ timeFmt(thread.first_session_at) }}</span>
              <span class="flex-1 border-t border-dashed border-border" />
              <span class="tabular-nums">{{ timeFmt(thread.last_session_at) }}</span>
            </div>
          </section>

          <section v-if="(thread?.projects?.length ?? 0) > 0">
            <h4 class="text-[10.5px] font-medium uppercase tracking-wider text-muted-foreground">
              涉及项目
            </h4>
            <ul class="mt-2 space-y-1.5">
              <li
                v-for="p in thread!.projects"
                :key="p"
                class="flex items-start gap-2"
              >
                <FolderGit2 class="mt-[3px] size-3.5 shrink-0 text-muted-foreground" />
                <div class="min-w-0 flex-1">
                  <div class="truncate text-[12.5px] font-medium">{{ lastProjectName(p) }}</div>
                  <div class="truncate text-[10.5px] text-muted-foreground">{{ p }}</div>
                </div>
              </li>
            </ul>
          </section>

          <section v-if="(thread?.adapters?.length ?? 0) > 0">
            <h4 class="text-[10.5px] font-medium uppercase tracking-wider text-muted-foreground">
              适配器
            </h4>
            <div class="mt-2 flex flex-wrap gap-1.5">
              <Badge
                v-for="a in thread!.adapters"
                :key="a"
                variant="secondary"
                class="px-2 text-[10.5px]"
              >
                {{ adapterLabel(a) }}
              </Badge>
            </div>
          </section>

          <section v-if="sessions.length">
            <h4 class="flex items-center justify-between text-[10.5px] font-medium uppercase tracking-wider text-muted-foreground">
              <span>会话 · 按时间倒序</span>
              <span class="tabular-nums">{{ sessions.length }}</span>
            </h4>
            <ul class="mt-2 divide-y divide-border rounded-lg border border-border">
              <li
                v-for="row in sessions"
                :key="row.id"
                class="cursor-pointer px-3 py-2.5 transition-colors hover:bg-accent/40"
                @click="emit('openSession', row)"
              >
                <div class="flex items-start justify-between gap-2">
                  <span class="line-clamp-1 text-[12.5px] font-medium">
                    {{ row.summary_title ?? row.title ?? '未命名会话' }}
                  </span>
                  <span class="shrink-0 tabular-nums text-[10.5px] text-muted-foreground">
                    {{ timeFmt(row.updated_at) }}
                  </span>
                </div>
                <div class="mt-1 flex items-center gap-2 text-[10.5px] text-muted-foreground">
                  <span>{{ adapterLabel(row.source) }}</span>
                  <span>·</span>
                  <span class="tabular-nums">{{ row.message_count }} 条</span>
                </div>
              </li>
            </ul>
          </section>

          <Separator />
          <section class="flex items-center gap-2">
            <Button
              v-if="thread"
              type="button"
              variant="outline"
              class="text-destructive hover:bg-destructive/10 hover:text-destructive"
              @click="emit('delete', thread)"
            >
              <Trash2 class="size-3.5" />
              <span class="ml-1.5">删除此主题</span>
            </Button>
          </section>
        </div>
      </div>
    </SheetContent>
  </Sheet>
</template>
