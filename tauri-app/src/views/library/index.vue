<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import {
  ArrowUpDown,
  ChevronDown,
  Download,
  FolderGit2,
  GitBranch,
  MessagesSquare,
  RefreshCw,
  Search,
  Sparkles,
} from 'lucide-vue-next'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import {
  sessions,
  totals,
  refreshSessions,
  refreshProjects,
  loadMoreSessions,
  type Session,
} from '@/stores/memex'
import { useMemex } from '@/composables/useMemex'
import { startScanning, stopScanning } from '@/composables/useScanState'
import { toast } from 'vue-sonner'
import LibraryFacets from './components/LibraryFacets.vue'
import LibrarySessionListItem from './components/LibrarySessionListItem.vue'
import LibrarySessionDrawer from './components/LibrarySessionDrawer.vue'
import LibraryProjectsGrid from './components/LibraryProjectsGrid.vue'

const memex = useMemex()
const ingesting = ref(false)
const loadingMore = ref(false)
// 后端返回小于 pageSize 时即认为已加载完；首屏 200 条若返回 < 200 则一开始就 false
const hasMoreSessions = ref(true)
const LOAD_MORE_PAGE_SIZE = 100

async function runIngest() {
  if (ingesting.value) return
  ingesting.value = true
  startScanning()
  try {
    const r = await memex.triggerIngest()
    toast.success(`采集完成：新消息 ${r.messages_ingested} 条，新片段 ${r.chunks_created} 块`)
    await Promise.all([refreshSessions(), refreshProjects()])
    // 重新拉了 200 条，hasMore 取决于后端总数；保守置 true，让用户能继续点
    hasMoreSessions.value = true
  } catch (e) {
    toast.error(`采集失败：${String(e)}`)
  } finally {
    stopScanning()
    ingesting.value = false
  }
}

async function onLoadMore() {
  if (loadingMore.value || !hasMoreSessions.value) return
  loadingMore.value = true
  try {
    const r = await loadMoreSessions(LOAD_MORE_PAGE_SIZE)
    hasMoreSessions.value = r.hasMore
    if (r.loaded === 0) {
      toast.info('已加载全部会话')
    }
  } finally {
    loadingMore.value = false
  }
}

// 已加载全部时主动关闭 hasMore（首屏 200 条；后端总数 ≤ 已加载 → 没有更多）
watch(
  [() => sessions.length, () => totals.sessions],
  ([loaded, total]) => {
    if (total > 0 && loaded >= total) hasMoreSessions.value = false
  },
  { immediate: true },
)

const formatCount = (n: number) => {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1).replace(/\.0$/, '')}M`
  if (n >= 1000) return `${(n / 1000).toFixed(1).replace(/\.0$/, '')}K`
  return String(n)
}

const route = useRoute()
const router = useRouter()

const tab = ref<'sessions' | 'projects' | 'threads'>('sessions')
const sort = ref<'recent' | 'duration' | 'messages'>('recent')

const query = ref('')
const fAdapters = ref<string[]>([])
const fProjects = ref<string[]>([])
const fTime = ref<string>('7d')
const fSummary = ref<string>('all')

const toggleAdapter = (a: string) => {
  const i = fAdapters.value.indexOf(a)
  if (i >= 0) fAdapters.value.splice(i, 1)
  else fAdapters.value.push(a)
}
const toggleProject = (p: string) => {
  const i = fProjects.value.indexOf(p)
  if (i >= 0) fProjects.value.splice(i, 1)
  else fProjects.value.push(p)
}
const clearFilters = () => {
  fAdapters.value = []
  fProjects.value = []
  fTime.value = '7d'
  fSummary.value = 'all'
  query.value = ''
}

const filtered = computed(() => {
  let xs = sessions.slice()
  if (fAdapters.value.length) xs = xs.filter((s) => fAdapters.value.includes(s.adapter))
  if (fProjects.value.length) xs = xs.filter((s) => fProjects.value.includes(s.project))
  if (fSummary.value === 'done') xs = xs.filter((s) => s.l2Done)
  else if (fSummary.value === 'pending') xs = xs.filter((s) => !s.l2Done)
  if (query.value)
    xs = xs.filter((s) =>
      `${s.title} ${s.project} ${s.topics.join(' ')} ${s.intent ?? ''}`
        .toLowerCase()
        .includes(query.value.toLowerCase()),
    )
  if (sort.value === 'duration') xs.sort((a, b) => b.durationMin - a.durationMin)
  else if (sort.value === 'messages') xs.sort((a, b) => b.messages - a.messages)
  else xs.sort((a, b) => new Date(b.startedAt).getTime() - new Date(a.startedAt).getTime())
  return xs
})

const groupedFiltered = computed(() => {
  if (!filtered.value.length) return []
  const maxIso = filtered.value.reduce(
    (acc, s) => (s.startedAt > acc ? s.startedAt : acc),
    filtered.value[0].startedAt,
  )
  const ref0 = new Date(maxIso)
  ref0.setHours(0, 0, 0, 0)
  const yest = new Date(ref0)
  yest.setDate(yest.getDate() - 1)
  const week = new Date(ref0)
  week.setDate(week.getDate() - 6)
  const buckets = {
    today: [] as Session[],
    yesterday: [] as Session[],
    week: [] as Session[],
    earlier: [] as Session[],
  }
  for (const s of filtered.value) {
    const d = new Date(s.startedAt)
    if (d >= ref0) buckets.today.push(s)
    else if (d >= yest) buckets.yesterday.push(s)
    else if (d >= week) buckets.week.push(s)
    else buckets.earlier.push(s)
  }
  const groups: { key: string; label: string; sessions: Session[] }[] = []
  if (buckets.today.length) groups.push({ key: 'today', label: '今天', sessions: buckets.today })
  if (buckets.yesterday.length)
    groups.push({ key: 'yesterday', label: '昨天', sessions: buckets.yesterday })
  if (buckets.week.length) groups.push({ key: 'week', label: '本周', sessions: buckets.week })
  if (buckets.earlier.length)
    groups.push({ key: 'earlier', label: '更早', sessions: buckets.earlier })
  return groups
})

const drawerSession = ref<Session | null>(null)
const drawerOpen = computed({
  get: () => drawerSession.value !== null,
  set: (v: boolean) => {
    if (!v) {
      drawerSession.value = null
      const q = { ...route.query }
      delete q.session
      router.replace({ query: q })
    }
  },
})

const openSession = (s: Session) => {
  drawerSession.value = s
  router.replace({ query: { ...route.query, session: s.id } })
}

const openProject = (name: string) => {
  fProjects.value = [name]
  fAdapters.value = []
  fSummary.value = 'all'
  fTime.value = 'all'
  query.value = ''
  tab.value = 'sessions'
}

watch(
  () => route.query.session,
  (id) => {
    drawerSession.value = sessions.find((x) => x.id === id) ?? null
  },
  { immediate: true },
)

const sortLabel = computed(() => {
  if (sort.value === 'duration') return '时长'
  if (sort.value === 'messages') return '消息数'
  return '最近更新'
})

const activeFilterCount = computed(
  () =>
    fAdapters.value.length +
    fProjects.value.length +
    (fTime.value !== '7d' ? 1 : 0) +
    (fSummary.value !== 'all' ? 1 : 0) +
    (query.value ? 1 : 0),
)
const hasActiveFilters = computed(() => activeFilterCount.value > 0)

const FACETS_WIDTH_KEY = 'memex.library.facetsWidth'
const FACETS_DEFAULT = 256
const FACETS_MIN = 200
const FACETS_MAX = 420
const facetsWidth = ref(FACETS_DEFAULT)
const asideRef = ref<HTMLElement | null>(null)

onMounted(() => {
  const v = localStorage.getItem(FACETS_WIDTH_KEY)
  const n = v ? Number.parseInt(v, 10) : NaN
  if (Number.isFinite(n)) facetsWidth.value = Math.max(FACETS_MIN, Math.min(FACETS_MAX, n))
})

let dragLeft = 0
let dragging = false
function startFacetsDrag(e: MouseEvent) {
  if (!asideRef.value) return
  e.preventDefault()
  dragLeft = asideRef.value.getBoundingClientRect().left
  dragging = true
  document.body.style.cursor = 'col-resize'
  document.body.style.userSelect = 'none'
  window.addEventListener('mousemove', onFacetsDrag)
  window.addEventListener('mouseup', stopFacetsDrag)
}
function onFacetsDrag(e: MouseEvent) {
  if (!dragging) return
  const w = Math.max(FACETS_MIN, Math.min(FACETS_MAX, e.clientX - dragLeft))
  facetsWidth.value = w
}
function stopFacetsDrag() {
  if (!dragging) return
  dragging = false
  document.body.style.cursor = ''
  document.body.style.userSelect = ''
  window.removeEventListener('mousemove', onFacetsDrag)
  window.removeEventListener('mouseup', stopFacetsDrag)
  localStorage.setItem(FACETS_WIDTH_KEY, String(facetsWidth.value))
}
function resetFacets() {
  facetsWidth.value = FACETS_DEFAULT
  localStorage.setItem(FACETS_WIDTH_KEY, String(FACETS_DEFAULT))
}

onBeforeUnmount(() => {
  window.removeEventListener('mousemove', onFacetsDrag)
  window.removeEventListener('mouseup', stopFacetsDrag)
})
</script>

<template>
  <div class="@container/main flex flex-1 flex-col min-h-0 overflow-hidden">
    <Teleport to="#memex-header-center" defer>
      <Tabs v-model="tab">
        <TabsList class="h-8">
          <TabsTrigger value="sessions" class="gap-1.5 text-[12px]">
            <MessagesSquare class="size-3.5" />
            会话
          </TabsTrigger>
          <TabsTrigger value="projects" class="gap-1.5 text-[12px]">
            <FolderGit2 class="size-3.5" />
            项目
          </TabsTrigger>
          <TabsTrigger value="threads" class="gap-1.5 text-[12px]">
            <GitBranch class="size-3.5" />
            线索
            <Badge variant="outline" class="ml-1 h-4 px-1 text-[9px]">测试版</Badge>
          </TabsTrigger>
        </TabsList>
      </Tabs>
    </Teleport>

    <Teleport to="#memex-header-actions" defer>
      <Button variant="outline" size="sm" class="h-8 gap-2" disabled>
        <Download class="size-4" />
        导出
      </Button>
      <Button size="sm" class="h-8 gap-2" :disabled="ingesting" @click="runIngest">
        <RefreshCw :class="['size-4', ingesting && 'animate-spin']" />
        {{ ingesting ? '采集中…' : '立即采集' }}
      </Button>
    </Teleport>

    <div class="flex flex-1 min-h-0 overflow-hidden">
      <aside
        v-if="tab === 'sessions'"
        ref="asideRef"
        class="hidden shrink-0 flex-col overflow-hidden border-r lg:flex"
        :style="{ width: facetsWidth + 'px' }"
      >
        <LibraryFacets
          :f-adapters="fAdapters"
          :f-projects="fProjects"
          :f-time="fTime"
          :f-summary="fSummary"
          :active-filter-count="activeFilterCount"
          :has-active-filters="hasActiveFilters"
          @toggle-adapter="toggleAdapter"
          @toggle-project="toggleProject"
          @update:f-adapters="(v) => (fAdapters = v)"
          @update:f-projects="(v) => (fProjects = v)"
          @update:f-time="(v) => (fTime = v)"
          @update:f-summary="(v) => (fSummary = v)"
          @clear="clearFilters"
        />
      </aside>

      <div
        v-if="tab === 'sessions'"
        class="group/resize hidden w-1 shrink-0 cursor-col-resize select-none transition-colors hover:bg-primary/40 lg:block"
        title="拖拽调整宽度（双击重置）"
        @mousedown="startFacetsDrag"
        @dblclick="resetFacets"
      />

      <section class="flex flex-1 min-w-0 flex-col overflow-hidden">
        <div
          v-if="tab === 'sessions'"
          class="flex shrink-0 items-center gap-2 px-5 pt-3 pb-2"
        >
          <div class="relative flex-1">
            <Search
              class="pointer-events-none absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground"
            />
            <Input v-model="query" class="h-9 pl-9" placeholder="按标题、摘要或片段搜索…" />
          </div>
          <span class="hidden whitespace-nowrap text-[11px] text-muted-foreground md:inline">
            {{ activeFilterCount }} 个筛选 · 显示
            <span class="font-medium text-foreground tabular-nums">{{ filtered.length }}</span>
            /
            <Tooltip :delay-duration="120">
              <TooltipTrigger as-child>
                <span class="cursor-default tabular-nums">{{ formatCount(totals.sessions) }}</span>
              </TooltipTrigger>
              <TooltipContent side="top" :side-offset="6" class="px-2 py-1 text-[11px]">
                <span class="tabular-nums">{{ totals.sessions.toLocaleString() }}</span>
                <span class="ml-1 text-muted-foreground">个会话</span>
              </TooltipContent>
            </Tooltip>
          </span>
          <DropdownMenu>
            <DropdownMenuTrigger as-child>
              <Button variant="outline" size="sm" class="h-9 gap-1.5 text-[12px]">
                <ArrowUpDown class="size-3.5" />
                {{ sortLabel }}
                <ChevronDown class="size-3" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem @click="sort = 'recent'">最近更新</DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem @click="sort = 'duration'">时长</DropdownMenuItem>
              <DropdownMenuItem @click="sort = 'messages'">消息数</DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>

        <div v-if="tab === 'sessions'" class="flex-1 min-h-0 overflow-y-auto">
          <template v-for="g in groupedFiltered" :key="g.key">
            <div
              class="sticky top-0 z-10 flex items-center gap-2 bg-background/90 px-5 py-1.5 backdrop-blur"
            >
              <span class="text-[10px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
                {{ g.label }}
              </span>
              <span
                class="rounded-full bg-muted/80 px-1.5 py-px text-[9px] font-medium tabular-nums text-muted-foreground"
              >
                {{ g.sessions.length }}
              </span>
            </div>
            <LibrarySessionListItem
              v-for="s in g.sessions"
              :key="s.id"
              :session="s"
              :group-key="g.key"
              :active="drawerSession?.id === s.id"
              @open="openSession"
            />
          </template>

          <div
            v-if="!filtered.length"
            class="flex flex-col items-center gap-2 py-12 text-center text-[12px] text-muted-foreground"
          >
            <MessagesSquare class="size-8 text-muted-foreground/40" />
            <p v-if="hasActiveFilters">没有匹配筛选条件的会话</p>
            <p v-else>暂无会话，点击右上角"立即采集"开始扫描</p>
          </div>

          <div
            v-else
            class="flex flex-col items-center gap-1.5 py-6 text-[12px] text-muted-foreground"
          >
            <Button
              v-if="hasMoreSessions"
              variant="ghost"
              size="sm"
              class="h-8 gap-2"
              :disabled="loadingMore"
              @click="onLoadMore"
            >
              <RefreshCw :class="['size-3.5', loadingMore && 'animate-spin']" />
              {{ loadingMore ? '加载中…' : '加载更多' }}
            </Button>
            <span v-else>
              已显示全部
              <span class="font-medium text-foreground tabular-nums">{{ filtered.length }}</span>
              个会话
            </span>
          </div>
        </div>

        <LibraryProjectsGrid v-else-if="tab === 'projects'" @open="openProject" />

        <div v-else class="flex flex-1 items-center justify-center p-10">
          <div class="flex flex-col items-center gap-3 text-center">
            <GitBranch class="size-10 text-muted-foreground/50" />
            <div>
              <div class="text-sm font-medium">线索（Threads） · 即将上线</div>
              <p class="mx-auto mt-1 max-w-md text-xs text-muted-foreground">
                按"主题线索"聚合跨会话的对话。需要后端补一张 thread_links 表，先在界面上留个入口。
              </p>
            </div>
            <Badge variant="outline" class="gap-1.5">
              <Sparkles class="size-3" />
              敬请期待
            </Badge>
          </div>
        </div>
      </section>
    </div>

    <LibrarySessionDrawer
      :session="drawerSession"
      :open="drawerOpen"
      @update:open="(v) => (drawerOpen = v)"
    />
  </div>
</template>
