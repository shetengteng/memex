<script setup lang="ts">
/**
 * L5「线索（Threads）」视图 v2 —— 卡片网格 + 详情侧栏。
 *
 * 设计依据：原型 design/20260608-01-Memex-线索Threads重新设计-原型.html
 *   - 80% 场景：浏览发现「我都在做什么主题」 → 卡片网格擅长
 *   - 20% 场景：深挖单个主题 → 右侧 Sheet 详情擅长
 *   - 顶部搜索栏（关键词 → LLM 检索）是主入口 (S1)
 *   - 自动聚类开关（UI 占位，定时调度由 daemon 后续接入） + 手动「全量聚类」按钮
 *
 * 严格使用 shadcn-vue 组件：Card / Badge / Button / Input / Switch /
 * Sheet / Separator / Dialog（删除二次确认）。外层滚动条参考 Today 页面，
 * 用 `flex-1 min-h-0 overflow-y-auto` 而不是 ScrollArea，避免 flex 高度链断裂。
 * Sheet 内部保留 ScrollArea（SheetContent 自身高度受控）。
 */
import { computed, onMounted, ref } from 'vue'
import { toast } from 'vue-sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardFooter, CardHeader } from '@/components/ui/card'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Separator } from '@/components/ui/separator'
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet'
import { Switch } from '@/components/ui/switch'
import {
  Clock,
  FolderGit2,
  Loader2,
  Plus,
  RefreshCw,
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

// ─── 主搜索（关键词 LLM 检索） ─────────────────────────────────
const llmQuery = ref('')
const llmSearching = ref(false)

const SUGGESTIONS = [
  'Tauri 多窗口',
  'L2 摘要 prompt',
  'memex 桌面化',
  'cursor 适配器',
  'LLM 节流',
]

// ─── 自动聚类开关 + 手动全量聚类 ──────────────────────────────
const AUTO_CLUSTER_KEY = 'memex.threads.autoCluster'
const autoCluster = ref(
  (() => {
    try {
      return localStorage.getItem(AUTO_CLUSTER_KEY) !== 'false'
    } catch {
      return true
    }
  })(),
)
const regenerating = ref(false)

function onAutoClusterChange(v: boolean) {
  autoCluster.value = v
  try {
    localStorage.setItem(AUTO_CLUSTER_KEY, String(v))
  } catch {
    /* ignore */
  }
}

// ─── 筛选 chips ───────────────────────────────────────────────
type FilterKey = 'all' | 'multi_project' | 'recent_7d'
const filter = ref<FilterKey>('all')

const filterCounts = computed(() => ({
  all: threads.length,
  multi_project: threads.filter((t) => (t.projects ?? []).length >= 2).length,
  recent_7d: threads.filter((t) => {
    const last = t.last_session_at ?? t.updated_at
    return isWithinDays(last, 7)
  }).length,
}))

const filteredThreads = computed(() => {
  switch (filter.value) {
    case 'multi_project':
      return threads.filter((t) => (t.projects ?? []).length >= 2)
    case 'recent_7d':
      return threads.filter((t) =>
        isWithinDays(t.last_session_at ?? t.updated_at, 7),
      )
    default:
      return threads.slice()
  }
})

function isWithinDays(iso: string | null | undefined, days: number): boolean {
  if (!iso) return false
  const t = Date.parse(iso)
  if (Number.isNaN(t)) return false
  return Date.now() - t <= days * 24 * 60 * 60 * 1000
}

// ─── 详情 Sheet ───────────────────────────────────────────────
const selectedThread = ref<ThreadRow | null>(null)
const detailSessions = ref<SessionRow[]>([])
const detailLoading = ref(false)

const sheetOpen = computed({
  get: () => selectedThread.value !== null,
  set: (v: boolean) => {
    if (!v) selectedThread.value = null
  },
})

async function openThread(t: ThreadRow) {
  selectedThread.value = t
  detailLoading.value = true
  detailSessions.value = []
  try {
    const detail = await fetchThreadDetail(t.id)
    detailSessions.value = detail?.sessions ?? []
    // 如果聚合字段在 detail 上有更新（如重新聚类后），同步
    if (detail?.thread) selectedThread.value = detail.thread
  } finally {
    detailLoading.value = false
  }
}

// ─── 删除确认 ─────────────────────────────────────────────────
const deleteTarget = ref<ThreadRow | null>(null)
const deleting = ref(false)

function requestDelete(t: ThreadRow, e?: Event) {
  if (e) {
    e.stopPropagation()
  }
  deleteTarget.value = t
}

async function confirmDelete() {
  const t = deleteTarget.value
  if (!t) return
  deleting.value = true
  try {
    await deleteThread(t.id)
    if (selectedThread.value?.id === t.id) {
      selectedThread.value = null
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

// ─── 操作 ─────────────────────────────────────────────────────
async function onSearch() {
  const q = llmQuery.value.trim()
  if (!q || llmSearching.value) return
  llmSearching.value = true
  try {
    const id = await searchThreadByQuery(q)
    llmQuery.value = ''
    const t = threads.find((x) => x.id === id)
    if (t) await openThread(t)
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
    if (selectedThread.value) {
      const stillExists = threads.find((t) => t.id === selectedThread.value?.id)
      if (stillExists) {
        await openThread(stillExists)
      } else {
        selectedThread.value = null
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

function focusSearch() {
  const el = document.getElementById('threads-search-input') as HTMLInputElement | null
  el?.focus()
}

// ─── 卡片派生数据 ─────────────────────────────────────────────
const dateRangeFmt = (start?: string | null, end?: string | null) => {
  if (!start || !end) return ''
  const s = new Date(start)
  const e = new Date(end)
  const fmt = (d: Date) =>
    d.toLocaleDateString('zh-CN', { month: 'numeric', day: 'numeric' })
  const days = Math.max(1, Math.round((e.getTime() - s.getTime()) / 86_400_000))
  return `${fmt(s)} → ${fmt(e)} · ${days} 天`
}

const timeFmt = (iso: string) =>
  new Date(iso).toLocaleString('zh-CN', {
    month: 'numeric',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  })

const lastProjectName = (path: string) => {
  const parts = path.split('/').filter(Boolean)
  return parts[parts.length - 1] ?? path
}

const adapterLabel = (a: string) => {
  if (a === 'claude_code') return 'Claude Code'
  if (a === 'cursor') return 'Cursor'
  if (a === 'codex') return 'Codex'
  if (a === 'opencode') return 'OpenCode'
  return a
}

// ─── 生命周期 ─────────────────────────────────────────────────
onMounted(async () => {
  await refreshThreads()
})
</script>

<template>
  <div class="flex flex-1 min-h-0 flex-col overflow-hidden">
    <!-- 顶部主搜索栏 + 自动开关 + 手动聚类 -->
    <section class="border-b border-border/60 px-6 py-4">
      <form class="flex items-center gap-2" @submit.prevent="onSearch">
        <div class="relative flex-1">
          <Wand2
            class="pointer-events-none absolute left-3.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
          />
          <Input
            id="threads-search-input"
            v-model="llmQuery"
            class="h-11 pl-10 text-[14px]"
            placeholder="输入主题、关键词或问题，让 LLM 从你的历史会话里挑出相关线索…"
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
      </form>

      <!-- 第二行：筛选 + 自动开关 + 手动聚类 -->
      <div class="mt-3 flex flex-wrap items-center gap-2 text-[12px]">
        <Badge
          :variant="filter === 'all' ? 'default' : 'outline'"
          class="cursor-pointer rounded-full px-3 py-1 text-[12px]"
          @click="filter = 'all'"
        >
          全部
          <span class="ml-1 tabular-nums opacity-70">{{ filterCounts.all }}</span>
        </Badge>
        <Badge
          :variant="filter === 'multi_project' ? 'default' : 'outline'"
          class="cursor-pointer rounded-full px-3 py-1 text-[12px]"
          @click="filter = 'multi_project'"
        >
          跨多项目
          <span class="ml-1 tabular-nums opacity-70">{{ filterCounts.multi_project }}</span>
        </Badge>
        <Badge
          :variant="filter === 'recent_7d' ? 'default' : 'outline'"
          class="cursor-pointer rounded-full px-3 py-1 text-[12px]"
          @click="filter = 'recent_7d'"
        >
          近 7 天
          <span class="ml-1 tabular-nums opacity-70">{{ filterCounts.recent_7d }}</span>
        </Badge>

        <div class="ml-auto flex items-center gap-3">
          <label class="flex cursor-pointer items-center gap-2 text-muted-foreground">
            <span>自动聚类</span>
            <Switch
              :model-value="autoCluster"
              size="sm"
              @update:model-value="onAutoClusterChange"
            />
          </label>
          <Separator orientation="vertical" class="h-4" />
          <Button
            type="button"
            variant="outline"
            size="sm"
            :disabled="regenerating"
            class="h-8"
            @click="onRegenerate"
          >
            <Loader2 v-if="regenerating" class="size-3.5 animate-spin" />
            <RefreshCw v-else class="size-3.5" />
            <span class="ml-1.5">全量聚类</span>
          </Button>
        </div>
      </div>
    </section>

    <!-- 卡片网格 / 空状态 -->
    <div class="flex-1 min-h-0 overflow-y-auto">
      <!-- 真的没有任何线索 -->
      <div
        v-if="threads.length === 0"
        class="flex h-full min-h-[60vh] items-center justify-center px-6 py-12"
      >
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
              @click="applySuggestion(s)"
            >
              {{ s }}
            </Badge>
          </div>
          <p class="mt-6 text-[11px] text-muted-foreground/70">
            或者点上方<span class="mx-1 font-medium text-foreground">全量聚类</span>让 LLM 自动归纳所有主题。
          </p>
        </div>
      </div>

      <!-- 筛选后为空 -->
      <div
        v-else-if="filteredThreads.length === 0"
        class="flex h-60 items-center justify-center px-6 text-center"
      >
        <p class="text-[12.5px] text-muted-foreground">
          当前筛选下没有线索。试试 <button class="text-foreground underline-offset-2 hover:underline" @click="filter = 'all'">查看全部</button>。
        </p>
      </div>

      <!-- 卡片网格 -->
      <ul
        v-else
        class="grid grid-cols-1 gap-3 px-6 py-5 md:grid-cols-2 xl:grid-cols-3"
      >
        <Card
          v-for="t in filteredThreads"
          :key="t.id"
          size="sm"
          class="group/card relative h-full cursor-pointer transition-all hover:border-foreground/40 hover:shadow-sm"
          @click="openThread(t)"
        >
          <CardHeader class="px-4">
            <div class="flex items-start justify-between gap-2">
              <h3 class="line-clamp-2 text-[14px] font-semibold leading-snug">
                {{ t.name }}
              </h3>
              <button
                type="button"
                aria-label="删除这条线索"
                class="flex size-6 shrink-0 items-center justify-center rounded text-muted-foreground opacity-0 transition-opacity hover:bg-destructive/10 hover:text-destructive group-hover/card:opacity-100 focus:opacity-100 focus:outline-none focus:ring-1 focus:ring-ring"
                @click="(e: MouseEvent) => requestDelete(t, e)"
              >
                <Trash2 class="size-3.5" />
              </button>
            </div>
          </CardHeader>

          <CardContent class="flex-1 space-y-3 px-4">
            <p
              v-if="t.summary"
              class="line-clamp-2 text-[12px] leading-relaxed text-muted-foreground"
            >
              {{ t.summary }}
            </p>
            <p v-else class="text-[12px] italic text-muted-foreground/70">
              （暂无摘要）
            </p>

            <!-- 时间跨度 -->
            <div
              v-if="t.first_session_at && t.last_session_at"
              class="flex items-center gap-2 text-[10.5px] text-muted-foreground"
            >
              <Clock class="size-3 shrink-0" />
              <span class="tabular-nums">{{ dateRangeFmt(t.first_session_at, t.last_session_at) }}</span>
            </div>

            <!-- 项目 + 适配器 chips -->
            <div v-if="(t.projects?.length ?? 0) || (t.adapters?.length ?? 0)" class="flex flex-wrap items-center gap-1.5">
              <Badge
                v-for="p in (t.projects ?? []).slice(0, 2)"
                :key="`p-${p}`"
                variant="secondary"
                class="gap-1 px-2 text-[10.5px]"
              >
                <FolderGit2 class="size-2.5" />
                {{ lastProjectName(p) }}
              </Badge>
              <Badge
                v-if="(t.projects?.length ?? 0) > 2"
                variant="secondary"
                class="px-2 text-[10.5px]"
              >
                +{{ (t.projects?.length ?? 0) - 2 }}
              </Badge>
              <Badge
                v-for="a in (t.adapters ?? [])"
                :key="`a-${a}`"
                variant="outline"
                class="px-2 text-[10.5px]"
              >
                {{ adapterLabel(a) }}
              </Badge>
            </div>
          </CardContent>

          <CardFooter class="mt-auto flex items-center justify-between px-4 text-[10.5px] text-muted-foreground/80">
            <span class="tabular-nums">{{ t.session_count }} 个会话</span>
            <span class="tabular-nums">{{ timeFmt(t.updated_at) }}</span>
          </CardFooter>
        </Card>

        <!-- 新建卡 -->
        <button
          type="button"
          class="flex min-h-[160px] flex-col items-center justify-center rounded-xl border border-dashed border-border bg-card/40 text-muted-foreground transition-all hover:border-foreground/40 hover:text-foreground"
          @click="focusSearch"
        >
          <Plus class="size-6" />
          <span class="mt-1.5 text-[12px]">检索一个新主题</span>
        </button>
      </ul>
    </div>

    <!-- 详情侧栏 -->
    <Sheet :open="sheetOpen" @update:open="(v: boolean) => (sheetOpen = v)">
      <SheetContent class="flex w-full flex-col p-0 sm:!max-w-md md:!max-w-lg">
        <SheetHeader class="border-b border-border/60 px-5 py-4">
          <div class="flex items-center gap-2 pr-8">
            <SheetTitle class="line-clamp-2 text-[15px] font-semibold">
              {{ selectedThread?.name }}
            </SheetTitle>
          </div>
          <SheetDescription class="sr-only">线索详情</SheetDescription>
          <div class="flex flex-wrap items-center gap-2 pr-8 pt-1">
            <Badge variant="secondary" class="tabular-nums text-[10.5px]">
              {{ selectedThread?.session_count ?? 0 }} 个会话
            </Badge>
            <Badge
              v-if="(selectedThread?.projects?.length ?? 0) > 0"
              variant="outline"
              class="tabular-nums text-[10.5px]"
            >
              {{ selectedThread?.projects?.length }} 个项目
            </Badge>
          </div>
        </SheetHeader>

        <ScrollArea class="flex-1">
          <div v-if="detailLoading" class="flex h-40 items-center justify-center">
            <Loader2 class="size-4 animate-spin text-muted-foreground" />
          </div>

          <div v-else class="space-y-6 px-5 py-5">
            <!-- 摘要 -->
            <section v-if="selectedThread?.summary">
              <h4 class="text-[10.5px] font-medium uppercase tracking-wider text-muted-foreground">
                主题摘要
              </h4>
              <p class="mt-2 text-[12.5px] leading-relaxed">
                {{ selectedThread.summary }}
              </p>
            </section>

            <!-- 活跃时间 -->
            <section
              v-if="selectedThread?.first_session_at && selectedThread?.last_session_at"
            >
              <h4 class="text-[10.5px] font-medium uppercase tracking-wider text-muted-foreground">
                活跃时间
              </h4>
              <div class="mt-2 flex items-center gap-2 text-[11.5px]">
                <span class="tabular-nums">{{ timeFmt(selectedThread.first_session_at) }}</span>
                <span class="flex-1 border-t border-dashed border-border" />
                <span class="tabular-nums">{{ timeFmt(selectedThread.last_session_at) }}</span>
              </div>
            </section>

            <!-- 涉及项目 -->
            <section v-if="(selectedThread?.projects?.length ?? 0) > 0">
              <h4 class="text-[10.5px] font-medium uppercase tracking-wider text-muted-foreground">
                涉及项目
              </h4>
              <ul class="mt-2 space-y-1">
                <li
                  v-for="p in selectedThread!.projects"
                  :key="p"
                  class="flex items-center gap-2 text-[12px]"
                >
                  <FolderGit2 class="size-3 text-muted-foreground" />
                  <span class="truncate">{{ lastProjectName(p) }}</span>
                  <span class="ml-auto truncate text-[10.5px] text-muted-foreground">{{ p }}</span>
                </li>
              </ul>
            </section>

            <!-- 适配器 -->
            <section v-if="(selectedThread?.adapters?.length ?? 0) > 0">
              <h4 class="text-[10.5px] font-medium uppercase tracking-wider text-muted-foreground">
                适配器
              </h4>
              <div class="mt-2 flex flex-wrap gap-1.5">
                <Badge
                  v-for="a in selectedThread!.adapters"
                  :key="a"
                  variant="secondary"
                  class="px-2 text-[10.5px]"
                >
                  {{ adapterLabel(a) }}
                </Badge>
              </div>
            </section>

            <!-- 会话列表 -->
            <section v-if="detailSessions.length">
              <h4 class="flex items-center justify-between text-[10.5px] font-medium uppercase tracking-wider text-muted-foreground">
                <span>会话 · 按时间倒序</span>
                <span class="tabular-nums">{{ detailSessions.length }}</span>
              </h4>
              <ul class="mt-2 divide-y divide-border rounded-lg border border-border">
                <li
                  v-for="row in detailSessions"
                  :key="row.id"
                  class="cursor-pointer px-3 py-2.5 transition-colors hover:bg-accent/40"
                  @click="emit('open', rowToSession(row))"
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

            <!-- footer 操作 -->
            <Separator />
            <section class="flex items-center gap-2">
              <Button
                v-if="selectedThread"
                type="button"
                variant="outline"
                class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                @click="(e: MouseEvent) => requestDelete(selectedThread!, e)"
              >
                <Trash2 class="size-3.5" />
                <span class="ml-1.5">删除此主题</span>
              </Button>
            </section>
          </div>
        </ScrollArea>
      </SheetContent>
    </Sheet>

    <!-- 删除确认 -->
    <Dialog
      :open="deleteTarget !== null"
      @update:open="(v: boolean) => { if (!v) deleteTarget = null }"
    >
      <DialogContent class="w-[92vw] !max-w-md">
        <DialogHeader>
          <DialogTitle>删除线索</DialogTitle>
          <DialogDescription>
            将删除「{{ deleteTarget?.name }}」（{{ deleteTarget?.session_count }} 个会话的关联）。
            <br />
            <span class="text-muted-foreground">
              只是删除主题分组，不会删除会话本身。下次"全量聚类"可能会再生成同名线索。
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
