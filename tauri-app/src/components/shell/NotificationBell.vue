<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Bell, Inbox, AlertTriangle, FileCheck, Brain, CalendarClock } from 'lucide-vue-next'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { useNotifications } from '@/composables/useNotifications'
import type { NotificationEntry } from '@/types'

const { items, unreadCount, refreshList, markRead, markAllRead } = useNotifications()

const popoverOpen = ref(false)
const dialogOpen = ref(false)
const activeItem = ref<NotificationEntry | null>(null)

// 用户首次展开 popover 时主动拉一次 list（轮询只取 unreadCount，list 是惰性）
watch(popoverOpen, (open) => {
  if (open) {
    void refreshList()
  }
})

const KIND_LABEL: Record<string, string> = {
  ingest_failed: '采集失败',
  summary_done: '摘要完成',
  reflect_pending: '反思待处理',
  weekly_report: '周报生成',
}

function kindIcon(kind: string) {
  switch (kind) {
    case 'ingest_failed':
      return AlertTriangle
    case 'summary_done':
      return FileCheck
    case 'reflect_pending':
      return Brain
    case 'weekly_report':
      return CalendarClock
    default:
      return Bell
  }
}

function kindIconClass(kind: string): string {
  switch (kind) {
    case 'ingest_failed':
      return 'text-rose-500'
    case 'summary_done':
      return 'text-emerald-500'
    case 'reflect_pending':
      return 'text-amber-500'
    case 'weekly_report':
      return 'text-blue-500'
    default:
      return 'text-muted-foreground'
  }
}

const unreadBadge = computed(() => (unreadCount.value > 99 ? '99+' : String(unreadCount.value)))

function relativeTime(iso: string): string {
  const now = Date.now()
  const t = new Date(iso).getTime()
  const diffSec = Math.max(0, Math.floor((now - t) / 1000))
  if (diffSec < 60) return `${diffSec}s 前`
  if (diffSec < 3600) return `${Math.floor(diffSec / 60)}min 前`
  if (diffSec < 86_400) return `${Math.floor(diffSec / 3600)}h 前`
  const days = Math.floor(diffSec / 86_400)
  if (days < 7) return `${days}d 前`
  return new Date(iso).toLocaleDateString()
}

function formatFullTime(iso: string): string {
  const d = new Date(iso)
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  const hh = String(d.getHours()).padStart(2, '0')
  const mm = String(d.getMinutes()).padStart(2, '0')
  const ss = String(d.getSeconds()).padStart(2, '0')
  return `${y}-${m}-${day} ${hh}:${mm}:${ss}`
}

// Try-parse payload；JSON parse 失败时返回原始字符串，前端按 pre 渲染。
function parsedPayload(it: NotificationEntry | null): { kind: 'json' | 'text' | 'empty'; value: unknown } {
  if (!it || !it.payload_json) return { kind: 'empty', value: null }
  try {
    return { kind: 'json', value: JSON.parse(it.payload_json) }
  } catch {
    return { kind: 'text', value: it.payload_json }
  }
}

const payloadView = computed(() => parsedPayload(activeItem.value))

function openDetail(it: NotificationEntry) {
  activeItem.value = it
  popoverOpen.value = false
  dialogOpen.value = true
  if (it.read_at === null) {
    void markRead(it.id)
  }
}

function onMarkAllRead() {
  void markAllRead()
}
</script>

<template>
  <Popover v-model:open="popoverOpen">
    <!-- 之前 Tooltip + PopoverTrigger 双层 as-child 包 Button，reka-ui 会把
         click 事件吃掉，导致点击图标无反应。改成 PopoverTrigger 直接挂 Button，
         tooltip 用原生 title 兜底（hover 时浏览器自带浮窗）。 -->
    <PopoverTrigger as-child>
      <Button
        variant="ghost"
        size="icon"
        class="relative size-8 text-muted-foreground hover:text-foreground"
        aria-label="通知中心"
        title="通知中心"
      >
        <Bell class="size-4" />
        <span
          v-if="unreadCount > 0"
          class="absolute -right-0.5 -top-0.5 inline-flex h-4 min-w-4 items-center justify-center rounded-full bg-rose-500 px-1 text-[10px] font-medium leading-none text-white"
        >
          {{ unreadBadge }}
        </span>
      </Button>
    </PopoverTrigger>

    <PopoverContent
      side="bottom"
      align="end"
      :side-offset="8"
      class="w-[360px] p-0"
    >
      <div class="flex items-center justify-between border-b px-3 py-2">
        <div class="flex items-center gap-2">
          <span class="text-[13px] font-medium">通知</span>
          <span v-if="unreadCount > 0" class="text-[11px] text-muted-foreground">
            {{ unreadCount }} 条未读
          </span>
        </div>
        <Button
          variant="ghost"
          size="sm"
          class="h-7 px-2 text-[11px] text-muted-foreground hover:text-foreground"
          :disabled="unreadCount === 0"
          @click="onMarkAllRead"
        >
          全部已读
        </Button>
      </div>

      <div v-if="items.length === 0" class="flex flex-col items-center justify-center gap-2 px-3 py-10 text-center">
        <Inbox class="size-6 text-muted-foreground/60" />
        <div class="text-[12px] text-muted-foreground">暂无通知</div>
        <div class="text-[11px] text-muted-foreground/60">摘要、反思、采集异常会在这里出现</div>
      </div>

      <div v-else class="max-h-[420px] overflow-y-auto">
        <button
          v-for="it in items"
          :key="it.id"
          type="button"
          class="flex w-full gap-2.5 border-b px-3 py-2.5 text-left last:border-b-0 hover:bg-muted/60"
          :class="{ 'bg-muted/30': it.read_at === null }"
          @click="openDetail(it)"
        >
          <div class="relative mt-0.5 shrink-0">
            <component :is="kindIcon(it.kind)" class="size-4" :class="kindIconClass(it.kind)" />
            <span
              v-if="it.read_at === null"
              class="absolute -right-1 -top-1 size-1.5 rounded-full bg-rose-500"
              aria-label="未读"
            />
          </div>
          <div class="min-w-0 flex-1">
            <div class="flex items-center justify-between gap-2">
              <span class="truncate text-[12.5px] font-medium">{{ it.title }}</span>
              <span class="shrink-0 text-[10.5px] text-muted-foreground tabular-nums">
                {{ relativeTime(it.created_at) }}
              </span>
            </div>
            <div class="mt-0.5 line-clamp-2 text-[11.5px] text-muted-foreground">
              {{ it.body }}
            </div>
            <div class="mt-1 text-[10.5px] text-muted-foreground/70">
              {{ KIND_LABEL[it.kind] ?? it.kind }}
            </div>
          </div>
        </button>
      </div>
    </PopoverContent>
  </Popover>

  <Dialog v-model:open="dialogOpen">
    <DialogContent v-if="activeItem" class="sm:max-w-[560px]">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <component :is="kindIcon(activeItem.kind)" class="size-4" :class="kindIconClass(activeItem.kind)" />
          <span>{{ activeItem.title }}</span>
        </DialogTitle>
        <DialogDescription>
          {{ KIND_LABEL[activeItem.kind] ?? activeItem.kind }} · {{ formatFullTime(activeItem.created_at) }}
        </DialogDescription>
      </DialogHeader>

      <div class="space-y-3">
        <div class="rounded-md bg-muted/40 px-3 py-2 text-[12.5px] leading-relaxed whitespace-pre-wrap">
          {{ activeItem.body }}
        </div>

        <div v-if="payloadView.kind !== 'empty'">
          <div class="mb-1 text-[11px] uppercase tracking-wide text-muted-foreground">详情</div>
          <pre
            class="max-h-[280px] overflow-y-auto rounded-md border bg-muted/30 px-3 py-2 text-[11.5px] leading-relaxed"
          >{{ payloadView.kind === 'json' ? JSON.stringify(payloadView.value, null, 2) : payloadView.value }}</pre>
        </div>
      </div>
    </DialogContent>
  </Dialog>
</template>
