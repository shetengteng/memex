<script setup lang="ts">
/**
 * L5「主题线索」master-detail 视图。
 *
 * 左侧：线索列表（按 updated_at DESC）+ 顶部「重新聚类」按钮 + 搜索。
 * 右侧：选中线索的 sessions 列表（复用 LibrarySessionListItem）。
 *
 * 设计：本组件不直接打开 session 详情 Dialog——它把点击事件 emit 上去，
 * 由 library/index.vue 复用已有的 LibrarySessionDrawer 弹框，避免双份维护。
 */
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import LibrarySessionListItem from './LibrarySessionListItem.vue'
import { GitBranch, Loader2, RefreshCw, Search, Sparkles } from 'lucide-vue-next'
import {
  fetchThreadDetail,
  refreshThreads,
  regenerateThreads,
  rowToSession,
  threads,
  type Session,
} from '@/stores/memex'
import type { SessionRow, ThreadRow } from '@/types'

const emit = defineEmits<{ open: [Session] }>()

const threadQuery = ref('')
const selectedThreadId = ref<number | null>(null)
const detailSessions = ref<SessionRow[]>([])
const detailLoading = ref(false)
const regenerating = ref(false)
const regenerateError = ref<string | null>(null)

// 左侧线索列表宽度（像素）。用户可以拖动中间分隔条改变。
// 上下界限：240..560，避免拖到太窄或挤掉右侧。
const LEFT_MIN = 240
const LEFT_MAX = 560
const LEFT_STORAGE_KEY = 'memex.library.threads.leftWidth'
const leftWidth = ref(
  (() => {
    try {
      const v = Number(localStorage.getItem(LEFT_STORAGE_KEY))
      if (Number.isFinite(v) && v >= LEFT_MIN && v <= LEFT_MAX) return v
    } catch {
      /* ignore */
    }
    return 340
  })(),
)
const isDragging = ref(false)
let dragStartX = 0
let dragStartWidth = 0

function onSeparatorPointerDown(e: PointerEvent) {
  isDragging.value = true
  dragStartX = e.clientX
  dragStartWidth = leftWidth.value
  document.body.style.cursor = 'col-resize'
  document.body.style.userSelect = 'none'
  window.addEventListener('pointermove', onSeparatorPointerMove)
  window.addEventListener('pointerup', onSeparatorPointerUp, { once: true })
}

function onSeparatorPointerMove(e: PointerEvent) {
  if (!isDragging.value) return
  const delta = e.clientX - dragStartX
  const next = Math.min(LEFT_MAX, Math.max(LEFT_MIN, dragStartWidth + delta))
  leftWidth.value = next
}

function onSeparatorPointerUp() {
  isDragging.value = false
  document.body.style.cursor = ''
  document.body.style.userSelect = ''
  window.removeEventListener('pointermove', onSeparatorPointerMove)
  try {
    localStorage.setItem(LEFT_STORAGE_KEY, String(leftWidth.value))
  } catch {
    /* ignore */
  }
}

onUnmounted(() => {
  window.removeEventListener('pointermove', onSeparatorPointerMove)
})

const filteredThreads = computed(() => {
  const q = threadQuery.value.trim().toLowerCase()
  let xs = threads.slice()
  if (q) {
    xs = xs.filter(
      (t: ThreadRow) =>
        t.name.toLowerCase().includes(q) || t.summary.toLowerCase().includes(q),
    )
  }
  return xs
})

const selectedThread = computed(() =>
  threads.find((t) => t.id === selectedThreadId.value) ?? null,
)

const tFmt = (iso: string) =>
  new Date(iso).toLocaleString('zh-CN', { dateStyle: 'short', timeStyle: 'short' })

async function selectThread(id: number) {
  selectedThreadId.value = id
  detailLoading.value = true
  detailSessions.value = []
  try {
    const detail = await fetchThreadDetail(id)
    detailSessions.value = detail?.sessions ?? []
  } finally {
    detailLoading.value = false
  }
}

async function onRegenerate() {
  regenerating.value = true
  regenerateError.value = null
  try {
    await regenerateThreads()
    // 拉完新数据后，如果之前选中的线索还在，重新拉详情
    if (selectedThreadId.value != null) {
      const stillExists = threads.some((t) => t.id === selectedThreadId.value)
      if (stillExists) {
        await selectThread(selectedThreadId.value)
      } else {
        selectedThreadId.value = null
        detailSessions.value = []
      }
    }
  } catch (e) {
    regenerateError.value = e instanceof Error ? e.message : String(e)
  } finally {
    regenerating.value = false
  }
}

const openSession = (s: Session) => emit('open', s)

onMounted(async () => {
  await refreshThreads()
  // 首次默认选中第一条，让右侧不空
  if (threads.length && selectedThreadId.value == null) {
    await selectThread(threads[0].id)
  }
})

// 用户在外面再次切换到 threads tab 时，threads 数组可能已经更新（被外面 refresh），
// 这里跟一下默认选中状态，避免空白。
watch(
  () => threads.length,
  (n) => {
    if (n && selectedThreadId.value == null) {
      void selectThread(threads[0].id)
    }
  },
)
</script>

<template>
  <div class="flex flex-1 min-h-0 overflow-hidden">
    <!-- 左：线索列表 -->
    <aside
      class="flex shrink-0 flex-col border-r border-border/60"
      :style="{ width: `${leftWidth}px` }"
    >
      <div class="flex items-center gap-2 border-b border-border/60 px-4 py-3">
        <div class="relative flex-1">
          <Search
            class="pointer-events-none absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground"
          />
          <Input
            v-model="threadQuery"
            class="h-9 pl-9"
            placeholder="搜索线索…"
          />
        </div>
        <Button
          variant="outline"
          :disabled="regenerating"
          class="h-9 px-3"
          @click="onRegenerate"
        >
          <Loader2 v-if="regenerating" class="size-3.5 animate-spin" />
          <RefreshCw v-else class="size-3.5" />
          <span class="ml-1.5 text-[12px]">重新聚类</span>
        </Button>
      </div>

      <div v-if="regenerateError" class="px-4 py-2 text-[11px] text-destructive">
        {{ regenerateError }}
      </div>

      <div class="flex-1 min-h-0 overflow-y-auto">
        <ul v-if="filteredThreads.length">
          <li
            v-for="t in filteredThreads"
            :key="t.id"
            :data-active="t.id === selectedThreadId"
            class="cursor-pointer border-b border-border/60 px-4 py-3 transition-colors hover:bg-accent/40 data-[active=true]:bg-accent/40"
            @click="selectThread(t.id)"
          >
            <div class="flex items-center justify-between gap-2">
              <div class="flex min-w-0 items-center gap-2">
                <GitBranch class="size-3.5 shrink-0 text-muted-foreground" />
                <span class="truncate text-[13px] font-semibold">{{ t.name }}</span>
              </div>
              <Badge variant="secondary" class="shrink-0 tabular-nums text-[10px]">
                {{ t.session_count }}
              </Badge>
            </div>
            <p
              v-if="t.summary"
              class="mt-1 line-clamp-2 text-[11px] text-muted-foreground"
            >
              {{ t.summary }}
            </p>
            <p class="mt-1 text-[10px] text-muted-foreground/80">
              {{ tFmt(t.updated_at) }}
            </p>
          </li>
        </ul>
        <div
          v-else
          class="flex h-40 items-center justify-center"
        >
          <div class="text-center">
            <GitBranch class="mx-auto size-8 text-muted-foreground/40" />
            <p class="mt-2 text-[12px] text-muted-foreground">
              {{ threadQuery ? '没有匹配的线索' : '暂无线索' }}
            </p>
            <p
              v-if="!threadQuery"
              class="mx-auto mt-1 max-w-[220px] text-[11px] text-muted-foreground/80"
            >
              点击右上角「重新聚类」让 LLM 把跨会话的主题归到一起。
            </p>
          </div>
        </div>
      </div>
    </aside>

    <!-- 中间分隔条：可拖动改变左侧宽度。用户反馈"线索的中间的竖线无法拖动 增加 宽度" -->
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label="拖动改变线索列表宽度"
      class="group relative w-1 shrink-0 cursor-col-resize select-none bg-transparent transition-colors hover:bg-primary/30"
      :data-dragging="isDragging"
      @pointerdown="onSeparatorPointerDown"
    >
      <!-- 视觉上加一条细 hairline 在中间，hover/drag 时显眼一点 -->
      <span
        class="pointer-events-none absolute inset-y-0 left-1/2 w-px -translate-x-1/2 bg-border/60 transition-colors group-hover:bg-primary/60 group-data-[dragging=true]:bg-primary"
      />
    </div>

    <!-- 右：选中线索详情 + sessions -->
    <section class="flex-1 min-w-0 overflow-y-auto">
      <div
        v-if="!selectedThread"
        class="flex h-full items-center justify-center"
      >
        <div class="text-center text-muted-foreground">
          <Sparkles class="mx-auto size-8 text-muted-foreground/40" />
          <p class="mt-2 text-[12px]">从左侧选择一条线索查看命中会话</p>
        </div>
      </div>
      <template v-else>
        <header class="border-b border-border/60 px-5 py-3.5">
          <div class="flex items-center gap-2">
            <GitBranch class="size-4 text-muted-foreground" />
            <h3 class="text-[14px] font-semibold">{{ selectedThread.name }}</h3>
            <Badge variant="outline" class="tabular-nums text-[10px]">
              {{ selectedThread.session_count }} 个会话
            </Badge>
          </div>
          <p
            v-if="selectedThread.summary"
            class="mt-1.5 text-[12px] text-muted-foreground"
          >
            {{ selectedThread.summary }}
          </p>
        </header>

        <div v-if="detailLoading" class="flex h-40 items-center justify-center">
          <Loader2 class="size-4 animate-spin text-muted-foreground" />
        </div>
        <ul v-else-if="detailSessions.length">
          <LibrarySessionListItem
            v-for="row in detailSessions"
            :key="row.id"
            :session="rowToSession(row)"
            group-key="month"
            :active="false"
            @open="openSession"
          />
        </ul>
        <div v-else class="flex h-40 items-center justify-center">
          <p class="text-[12px] text-muted-foreground">这条线索下暂无会话</p>
        </div>
      </template>
    </section>
  </div>
</template>
