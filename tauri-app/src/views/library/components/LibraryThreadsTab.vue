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
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { toast } from 'vue-sonner'
import LibrarySessionListItem from './LibrarySessionListItem.vue'
import {
  GitBranch,
  Loader2,
  RefreshCw,
  Search,
  Sparkles,
  Trash2,
  Wand2,
} from 'lucide-vue-next'
import {
  deleteThread,
  fetchThreadDetail,
  refreshThreads,
  regenerateThreads,
  rowToSession,
  searchThreadByQuery,
  threads,
  type Session,
} from '@/stores/memex'
import type { SessionRow, ThreadRow } from '@/types'
import { humanizeBackendError } from '@/lib/utils'

const emit = defineEmits<{ open: [Session] }>()

const threadQuery = ref('')
const selectedThreadId = ref<number | null>(null)
const detailSessions = ref<SessionRow[]>([])
const detailLoading = ref(false)
const regenerating = ref(false)
const regenerateError = ref<string | null>(null)

// 关键词 LLM 检索
const llmQuery = ref('')
const llmSearching = ref(false)

// 删除确认
const deleteTarget = ref<ThreadRow | null>(null)
const deleting = ref(false)

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

async function onLlmSearch() {
  const q = llmQuery.value.trim()
  if (!q) return
  llmSearching.value = true
  try {
    const id = await searchThreadByQuery(q)
    llmQuery.value = ''
    // 新线索往往出现在列表顶部（updated_at 是 now），选中它让用户看到结果
    if (id) {
      await selectThread(id)
    }
    toast.success(`已为「${q}」生成线索`)
  } catch (e) {
    toast.error(humanizeBackendError(e).friendly)
  } finally {
    llmSearching.value = false
  }
}

function requestDelete(t: ThreadRow) {
  deleteTarget.value = t
}

async function confirmDelete() {
  const t = deleteTarget.value
  if (!t) return
  deleting.value = true
  try {
    await deleteThread(t.id)
    if (selectedThreadId.value === t.id) {
      selectedThreadId.value = null
      detailSessions.value = []
    }
    toast.success(`已删除线索「${t.name}」`)
  } catch (e) {
    toast.error(humanizeBackendError(e).friendly)
  } finally {
    deleting.value = false
    deleteTarget.value = null
  }
}

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
      <div class="space-y-2 border-b border-border/60 px-4 py-3">
        <div class="flex items-center gap-2">
          <div class="relative flex-1">
            <Search
              class="pointer-events-none absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground"
            />
            <Input
              v-model="threadQuery"
              class="h-9 pl-9"
              placeholder="搜索已有线索…"
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
        <!-- 按关键词让 LLM 在历史会话里挑相关 session 生成新线索 -->
        <form class="flex items-center gap-2" @submit.prevent="onLlmSearch">
          <div class="relative flex-1">
            <Wand2
              class="pointer-events-none absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground"
            />
            <Input
              v-model="llmQuery"
              class="h-9 pl-9"
              placeholder="按关键词让 LLM 找相关线索…"
              :disabled="llmSearching"
            />
          </div>
          <Button
            type="submit"
            variant="secondary"
            :disabled="llmSearching || !llmQuery.trim()"
            class="h-9 px-3"
          >
            <Loader2 v-if="llmSearching" class="size-3.5 animate-spin" />
            <Wand2 v-else class="size-3.5" />
            <span class="ml-1.5 text-[12px]">检索</span>
          </Button>
        </form>
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
            class="group/thread-row relative cursor-pointer border-b border-border/60 px-4 py-3 transition-colors hover:bg-accent/40 data-[active=true]:bg-accent/40"
            @click="selectThread(t.id)"
          >
            <div class="flex items-center justify-between gap-2">
              <div class="flex min-w-0 items-center gap-2">
                <GitBranch class="size-3.5 shrink-0 text-muted-foreground" />
                <span class="truncate text-[13px] font-semibold">{{ t.name }}</span>
              </div>
              <div class="flex shrink-0 items-center gap-1">
                <Badge variant="secondary" class="tabular-nums text-[10px]">
                  {{ t.session_count }}
                </Badge>
                <!-- hover 时才出现的删除按钮：列表里默认 opacity-0 不抢注意力 -->
                <button
                  type="button"
                  aria-label="删除线索"
                  class="flex size-6 items-center justify-center rounded text-muted-foreground opacity-0 transition-opacity hover:bg-destructive/10 hover:text-destructive group-hover/thread-row:opacity-100 focus:opacity-100 focus:outline-none focus:ring-1 focus:ring-ring"
                  @click.stop="requestDelete(t)"
                >
                  <Trash2 class="size-3" />
                </button>
              </div>
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

    <!-- 删除确认。线索删除不影响下面的 session 本体，只断开 thread_sessions 关联，
         所以风险低。但仍弹个确认避免误删。-->
    <Dialog
      :open="deleteTarget !== null"
      @update:open="(v: boolean) => { if (!v) deleteTarget = null }"
    >
      <DialogContent class="w-[92vw] !max-w-md">
        <DialogHeader>
          <DialogTitle>删除线索</DialogTitle>
          <DialogDescription>
            将删除线索「{{ deleteTarget?.name }}」（包含
            {{ deleteTarget?.session_count }} 个会话的关联）。
            <br />
            <span class="text-muted-foreground">
              这只会断开线索 ↔ 会话的关联，不会删除会话本身。下次"重新聚类"可能会再生成同名线索。
            </span>
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            :disabled="deleting"
            @click="deleteTarget = null"
          >
            取消
          </Button>
          <Button
            type="button"
            class="bg-destructive text-destructive-foreground hover:bg-destructive/90"
            :disabled="deleting"
            @click="confirmDelete"
          >
            <Loader2 v-if="deleting" class="mr-1.5 size-3.5 animate-spin" />
            删除
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
