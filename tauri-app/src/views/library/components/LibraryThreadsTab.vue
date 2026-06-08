<script setup lang="ts">
/**
 * L5「线索（Threads）」视图 —— 搜索引擎心智模型重设计。
 *
 * 信息架构（参考浏览器历史 / Spotlight 历史）：
 *   - 顶部：醒目的关键词检索（主行为）+ 全量聚类（辅助行为）
 *   - 左侧：搜索历史（之前的线索列表，按时间倒序）
 *   - 右侧：当前选中那次搜索的结果（命中的 session 列表）
 *   - 未选中：搜索引擎风格的 hero 空状态（大搜索框 + 几条建议词）
 *
 * 设计依据用户反馈："线索更像是搜索引擎，之前的是线索的搜索历史"。
 *
 * 组件不直接打开 session 详情，把 open 事件 emit 上去，由 library/index.vue
 * 用已有的 LibrarySessionDrawer 弹框，避免双份维护。
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
  Clock,
  GitBranch,
  History,
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

const selectedThreadId = ref<number | null>(null)
const detailSessions = ref<SessionRow[]>([])
const detailLoading = ref(false)

// 主搜索（关键词 LLM 检索）
const llmQuery = ref('')
const llmSearching = ref(false)

// 全量聚类（辅助）
const regenerating = ref(false)

// 历史筛选
const historyFilter = ref('')

// 删除确认
const deleteTarget = ref<ThreadRow | null>(null)
const deleting = ref(false)

const SUGGESTIONS = [
  'Tauri 多窗口',
  'L2 摘要 prompt',
  'memex 桌面化',
  'cursor 适配器',
  'LLM 节流',
]

const filteredHistory = computed(() => {
  const q = historyFilter.value.trim().toLowerCase()
  if (!q) return threads.slice()
  return threads.filter(
    (t: ThreadRow) =>
      t.name.toLowerCase().includes(q) || t.summary.toLowerCase().includes(q),
  )
})

const selectedThread = computed(() =>
  threads.find((t) => t.id === selectedThreadId.value) ?? null,
)

const tFmt = (iso: string) =>
  new Date(iso).toLocaleString('zh-CN', { dateStyle: 'short', timeStyle: 'short' })

// 左侧历史栏宽度（可拖动），延续上一轮的体验。
const LEFT_MIN = 240
const LEFT_MAX = 480
const LEFT_STORAGE_KEY = 'memex.library.threads.leftWidth'
const leftWidth = ref(
  (() => {
    try {
      const v = Number(localStorage.getItem(LEFT_STORAGE_KEY))
      if (Number.isFinite(v) && v >= LEFT_MIN && v <= LEFT_MAX) return v
    } catch {
      /* ignore */
    }
    return 280
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
  const next = Math.min(LEFT_MAX, Math.max(LEFT_MIN, dragStartWidth + (e.clientX - dragStartX)))
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

async function onSearch() {
  const q = llmQuery.value.trim()
  if (!q) return
  llmSearching.value = true
  try {
    const id = await searchThreadByQuery(q)
    llmQuery.value = ''
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

function applySuggestion(s: string) {
  llmQuery.value = s
  void onSearch()
}

async function onRegenerate() {
  regenerating.value = true
  try {
    await regenerateThreads()
    if (selectedThreadId.value != null) {
      const stillExists = threads.some((t) => t.id === selectedThreadId.value)
      if (stillExists) {
        await selectThread(selectedThreadId.value)
      } else {
        selectedThreadId.value = null
        detailSessions.value = []
      }
    }
    toast.success('已重新聚类')
  } catch (e) {
    toast.error(humanizeBackendError(e).friendly)
  } finally {
    regenerating.value = false
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
    toast.success(`已删除「${t.name}」`)
  } catch (e) {
    toast.error(humanizeBackendError(e).friendly)
  } finally {
    deleting.value = false
    deleteTarget.value = null
  }
}

const openSession = (s: Session) => emit('open', s)

onMounted(async () => {
  await refreshThreads()
})

// 选择历史变化时让 watcher 默认行为：不要自动跳到第一条，让用户从 hero 出发。
// 但如果用户在另一个 tab/窗口里加了新线索（reactive 数组变了），保持当前选中即可。
watch(
  () => threads.length,
  (n, old) => {
    if (n > old && selectedThreadId.value == null) {
      // 新增了线索（一般是搜索/聚类完成），自动选中最新的那条（updated_at 最大）
      const newest = threads[0]
      if (newest) void selectThread(newest.id)
    }
  },
)
</script>

<template>
  <div class="flex flex-1 min-h-0 overflow-hidden">
    <!-- 左：搜索历史 -->
    <aside
      class="flex shrink-0 flex-col border-r border-border/60"
      :style="{ width: `${leftWidth}px` }"
    >
      <div class="flex items-center gap-2 border-b border-border/60 px-4 py-2.5">
        <History class="size-3.5 text-muted-foreground" />
        <span class="text-[12px] font-medium">搜索历史</span>
        <Badge variant="secondary" class="ml-auto tabular-nums text-[10px]">
          {{ threads.length }}
        </Badge>
      </div>
      <div class="px-4 py-2">
        <div class="relative">
          <Search
            class="pointer-events-none absolute left-3 top-1/2 size-3 -translate-y-1/2 text-muted-foreground"
          />
          <Input
            v-model="historyFilter"
            class="h-8 pl-8 text-[12px]"
            placeholder="筛选历史…"
          />
        </div>
      </div>

      <div class="flex-1 min-h-0 overflow-y-auto">
        <ul v-if="filteredHistory.length" class="px-2 pb-2">
          <li
            v-for="t in filteredHistory"
            :key="t.id"
            :data-active="t.id === selectedThreadId"
            class="group/history-row relative cursor-pointer rounded-md px-2 py-2 transition-colors hover:bg-accent/40 data-[active=true]:bg-accent/60"
            @click="selectThread(t.id)"
          >
            <div class="flex items-start gap-2">
              <Clock class="mt-[3px] size-3 shrink-0 text-muted-foreground/70" />
              <div class="min-w-0 flex-1">
                <div class="flex items-center justify-between gap-2">
                  <span class="truncate text-[12.5px] font-medium">{{ t.name }}</span>
                  <button
                    type="button"
                    aria-label="删除这条搜索历史"
                    class="flex size-5 shrink-0 items-center justify-center rounded text-muted-foreground opacity-0 transition-opacity hover:bg-destructive/10 hover:text-destructive group-hover/history-row:opacity-100 focus:opacity-100 focus:outline-none focus:ring-1 focus:ring-ring"
                    @click.stop="requestDelete(t)"
                  >
                    <Trash2 class="size-3" />
                  </button>
                </div>
                <div class="mt-0.5 flex items-center gap-1.5 text-[10px] text-muted-foreground/80">
                  <span class="tabular-nums">{{ t.session_count }} 个结果</span>
                  <span aria-hidden="true">·</span>
                  <span>{{ tFmt(t.updated_at) }}</span>
                </div>
              </div>
            </div>
          </li>
        </ul>
        <div v-else class="flex h-40 items-center justify-center px-4 text-center">
          <p class="text-[11px] text-muted-foreground">
            {{ historyFilter ? '没有匹配的历史' : '还没有搜索记录' }}
          </p>
        </div>
      </div>
    </aside>

    <!-- 拖动分隔条 -->
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label="拖动改变历史栏宽度"
      class="group relative w-1 shrink-0 cursor-col-resize select-none bg-transparent transition-colors hover:bg-primary/30"
      :data-dragging="isDragging"
      @pointerdown="onSeparatorPointerDown"
    >
      <span
        class="pointer-events-none absolute inset-y-0 left-1/2 w-px -translate-x-1/2 bg-border/60 transition-colors group-hover:bg-primary/60 group-data-[dragging=true]:bg-primary"
      />
    </div>

    <!-- 右：主搜索区 + 结果 -->
    <section class="flex min-w-0 flex-1 flex-col overflow-hidden">
      <!-- 顶部：搜索栏（主操作） -->
      <div class="border-b border-border/60 px-6 py-3">
        <form class="flex items-center gap-2" @submit.prevent="onSearch">
          <div class="relative flex-1">
            <Wand2
              class="pointer-events-none absolute left-3.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
            />
            <Input
              v-model="llmQuery"
              class="h-11 pl-10 text-[14px]"
              placeholder="输入主题、关键词或问题，让 LLM 从历史会话里挑出相关线索…"
              :disabled="llmSearching"
            />
          </div>
          <Button
            type="submit"
            :disabled="llmSearching || !llmQuery.trim()"
            class="h-11 px-5"
          >
            <Loader2 v-if="llmSearching" class="size-4 animate-spin" />
            <Wand2 v-else class="size-4" />
            <span class="ml-1.5">检索</span>
          </Button>
          <!-- 全量聚类降为辅助，外观上去掉强调 -->
          <Button
            type="button"
            variant="ghost"
            :disabled="regenerating"
            class="h-11 text-[12px] text-muted-foreground hover:text-foreground"
            @click="onRegenerate"
          >
            <Loader2 v-if="regenerating" class="size-3.5 animate-spin" />
            <RefreshCw v-else class="size-3.5" />
            <span class="ml-1.5">全量聚类</span>
          </Button>
        </form>
      </div>

      <!-- 中间：结果区 / 空状态 hero -->
      <div class="flex-1 overflow-y-auto">
        <!-- 选中状态：展示当前线索的 sessions -->
        <template v-if="selectedThread">
          <header class="border-b border-border/60 px-6 py-4">
            <div class="flex items-center gap-2">
              <GitBranch class="size-4 text-muted-foreground" />
              <h3 class="text-[15px] font-semibold">{{ selectedThread.name }}</h3>
              <Badge variant="outline" class="tabular-nums text-[10px]">
                {{ selectedThread.session_count }} 个会话
              </Badge>
            </div>
            <p
              v-if="selectedThread.summary"
              class="mt-1.5 text-[12.5px] text-muted-foreground"
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

        <!-- 未选中：搜索引擎风格的 hero -->
        <div v-else class="flex h-full items-center justify-center px-6 py-12">
          <div class="w-full max-w-lg text-center">
            <div class="mx-auto flex size-14 items-center justify-center rounded-full bg-primary/10">
              <Sparkles class="size-6 text-primary" />
            </div>
            <h3 class="mt-4 text-[16px] font-semibold">从主题开始检索</h3>
            <p class="mx-auto mt-2 max-w-md text-[12.5px] text-muted-foreground">
              输入一个关键词或问题，让本地 LLM 从你最近 80 个有摘要的会话里
              挑出相关的，组成一条「线索」。每次检索都会保存到左侧搜索历史。
            </p>
            <div class="mt-6 flex flex-wrap items-center justify-center gap-2">
              <button
                v-for="s in SUGGESTIONS"
                :key="s"
                type="button"
                class="rounded-full border border-border/80 bg-background px-3 py-1 text-[11.5px] text-muted-foreground transition-colors hover:border-primary/60 hover:bg-primary/5 hover:text-foreground"
                @click="applySuggestion(s)"
              >
                {{ s }}
              </button>
            </div>
            <p class="mt-6 text-[11px] text-muted-foreground/70">
              或者点上方<span class="font-medium text-foreground">全量聚类</span>让 LLM 自动归纳所有主题。
            </p>
          </div>
        </div>
      </div>
    </section>

    <!-- 删除确认 -->
    <Dialog
      :open="deleteTarget !== null"
      @update:open="(v: boolean) => { if (!v) deleteTarget = null }"
    >
      <DialogContent class="w-[92vw] !max-w-md">
        <DialogHeader>
          <DialogTitle>删除搜索历史</DialogTitle>
          <DialogDescription>
            将删除「{{ deleteTarget?.name }}」（{{ deleteTarget?.session_count }} 个结果）。
            <br />
            <span class="text-muted-foreground">
              只是删除这次搜索的记录，不会删除会话本身。下次"重新聚类"可能会再生成同名线索。
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
