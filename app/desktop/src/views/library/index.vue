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
  FolderGit2,
  GitBranch,
  MessagesSquare,
  RefreshCw,
  Search,
} from 'lucide-vue-next'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import {
  librarySessions,
  libraryHasMore,
  totals,
  reloadLibrarySessions,
  loadMoreLibrarySessions,
  refreshProjects,
  type Session,
} from '@/stores/memex'
import type { SessionListFilter } from '@/types'
import { useMemex } from '@/composables/useMemex'
import { startScanning, stopScanning } from '@/composables/useScanState'
import { toast } from 'vue-sonner'
import { toastBackendError } from '@/lib/toast-error'
import LibraryFacets from './components/LibraryFacets.vue'
import LibrarySessionListItem from './components/LibrarySessionListItem.vue'
import LibrarySessionDrawer from './components/LibrarySessionDrawer.vue'
import LibraryThreadsTab from './components/LibraryThreadsTab.vue'
import LibraryProjectsGrid from './components/LibraryProjectsGrid.vue'
import {
  groupSessionsByDate,
  type GroupLabels,
  type SortKey,
  type SummaryFilter,
  type TimeFilter,
} from './composables/sessionFilters'
import { useI18n } from '@/i18n'

const memex = useMemex()
const { t } = useI18n()
const ingesting = ref(false)
const loadingMore = ref(false)
const LOAD_MORE_PAGE_SIZE = 100

async function runIngest() {
  if (ingesting.value) return
  ingesting.value = true
  startScanning()
  try {
    const r = await memex.triggerIngest()
    toast.success(t('library.sessions.toast.ingest_done', { messages: r.messages_ingested, chunks: r.chunks_created }))
    await Promise.all([reloadLibrarySessions(currentFilter.value), refreshProjects()])
  } catch (e) {
    toastBackendError(t('library.sessions.toast.ingest_failed'), e)
  } finally {
    stopScanning()
    ingesting.value = false
  }
}

async function onLoadMore() {
  if (loadingMore.value || !libraryHasMore.value) return
  loadingMore.value = true
  try {
    const r = await loadMoreLibrarySessions(currentFilter.value, LOAD_MORE_PAGE_SIZE)
    if (r.loaded === 0) {
      toast.info(t('library.sessions.toast.all_loaded'))
    }
  } finally {
    loadingMore.value = false
  }
}

// 无限滚动：当 sentinel 进入视口时触发 onLoadMore。仍保留底部按钮作为兜底入口。
const loadMoreSentinel = ref<HTMLElement | null>(null)
let loadMoreObserver: IntersectionObserver | null = null

function setupInfiniteScroll() {
  if (loadMoreObserver) return
  if (!loadMoreSentinel.value) return
  loadMoreObserver = new IntersectionObserver(
    (entries) => {
      const hit = entries.some((e) => e.isIntersecting)
      if (hit && libraryHasMore.value && !loadingMore.value) {
        onLoadMore()
      }
    },
    { rootMargin: '200px 0px' },
  )
  loadMoreObserver.observe(loadMoreSentinel.value)
}

function teardownInfiniteScroll() {
  if (loadMoreObserver) {
    loadMoreObserver.disconnect()
    loadMoreObserver = null
  }
}

watch(loadMoreSentinel, (el) => {
  if (el) {
    setupInfiniteScroll()
  } else {
    teardownInfiniteScroll()
  }
})

onBeforeUnmount(teardownInfiniteScroll)

const formatCount = (n: number) => {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1).replace(/\.0$/, '')}M`
  if (n >= 1000) return `${(n / 1000).toFixed(1).replace(/\.0$/, '')}K`
  return String(n)
}

const route = useRoute()
const router = useRouter()

const tab = ref<'sessions' | 'projects' | 'threads'>('sessions')
const sort = ref<SortKey>('recent')

const query = ref('')
const fAdapters = ref<string[]>([])
// fProjects 存的是**完整 project_path** 数组（不是末段名）。后端按
// `project_path IN (?, ?, ...)` 精确匹配，与项目 facet 上的计数一一对齐。
// 之前用末段名 + `LIKE '%/<name>'` 会把同末段的多个不同项目混算，导致
// 顶部"显示 X / 总 Y"跟左侧 facet 单行计数对不上。
const fProjects = ref<string[]>([])
const fTime = ref<TimeFilter>('7d')
const fSummary = ref<SummaryFilter>('all')

const toggleAdapter = (a: string) => {
  const i = fAdapters.value.indexOf(a)
  if (i >= 0) fAdapters.value.splice(i, 1)
  else fAdapters.value.push(a)
}
const toggleProject = (path: string) => {
  const i = fProjects.value.indexOf(path)
  if (i >= 0) fProjects.value.splice(i, 1)
  else fProjects.value.push(path)
}
const clearFilters = () => {
  fAdapters.value = []
  fProjects.value = []
  fTime.value = '7d'
  fSummary.value = 'all'
  query.value = ''
}

// 注意：分组用真实当前时间（today/yesterday/week/earlier），跟后端 SQL
// 时间窗口（'today' / '7d' / '30d' / '90d'）正交——前者是"如何分块展示"，
// 后者是"是否纳入结果集"。所以列表已经被后端按 fTime 过滤一遍后，仍然
// 用本地时间做分组，让"今天 / 昨天 / 本周"在 UI 上始终可读。
const groupLabels = computed<GroupLabels>(() => ({
  today: t('library.group.today'),
  yesterday: t('library.group.yesterday'),
  week: t('library.group.week'),
  earlier: t('library.group.earlier'),
}))
const groupedFiltered = computed(() => groupSessionsByDate(librarySessions, new Date(), groupLabels.value))

// 当前过滤态打包成后端 DTO（None / 不传字段 = 不过滤）。
// 注意 7d/all 等 default 值仍然传给后端，让 SQL WHERE 显式过滤；这跟前端
// `activeFilterCount` 把 '7d' 视为"默认"是两套语义。
const currentFilter = computed<SessionListFilter>(() => {
  const q = query.value.trim()
  return {
    adapters: fAdapters.value.length ? fAdapters.value : undefined,
    projects: fProjects.value.length ? fProjects.value : undefined,
    time: fTime.value,
    summary: fSummary.value,
    query: q || undefined,
    sort: sort.value,
  }
})

// 任何 filter 变化都触发一次后端重拉。{ immediate: true } 让首屏直接生效；
// `flush: 'post'` 等 UI 状态全部 settle 再走 IPC，避免连续点 facet 起多次
// 飞行中的请求叠加返回乱序问题（最后一个 await 决定的最终 sessions[]）。
watch(
  currentFilter,
  async (filter) => {
    await reloadLibrarySessions(filter)
  },
  { immediate: true, flush: 'post' },
)

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

const openProject = (path: string) => {
  fProjects.value = [path]
  fAdapters.value = []
  fSummary.value = 'all'
  fTime.value = 'all'
  query.value = ''
  tab.value = 'sessions'
}

watch(
  () => route.query.session,
  (id) => {
    if (!id) {
      drawerSession.value = null
      return
    }
    // 跨 tab 打开会话时（如线索 tab 点条目），`librarySessions` 数组可能不包含该 id
    //（被当前 filter 筛掉，或还没拉到这一页），此时保留 drawerSession
    //（由调用方提前 set 的实例），避免被覆盖为 null。
    const found = librarySessions.find((x) => x.id === id)
    if (found) {
      drawerSession.value = found
    } else if (drawerSession.value?.id !== id) {
      drawerSession.value = null
    }
  },
  { immediate: true },
)

// `?project=<encoded path>` 入口：CommandPalette / TodayCard 跳到资料库时，
// 用 URL query 携带项目 path，进入 Library 后把它写到 fProjects 触发筛选。
// 不复用 openProject() 是为了让 watch 自己负责清掉 watch trigger 之外的副作用，
// 也确保用户在 Library 内打开浏览器历史返回时筛选状态正确恢复。
watch(
  () => route.query.project,
  (raw) => {
    if (typeof raw !== 'string' || !raw) return
    fProjects.value = [raw]
    fAdapters.value = []
    fSummary.value = 'all'
    fTime.value = 'all'
    query.value = ''
    tab.value = 'sessions'
  },
  { immediate: true },
)

const sortLabel = computed(() => {
  if (sort.value === 'duration') return t('library.sessions.sort.duration')
  if (sort.value === 'messages') return t('library.sessions.sort.messages')
  return t('library.sessions.sort.recent')
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
            {{ t('library.tabs.sessions') }}
          </TabsTrigger>
          <TabsTrigger value="projects" class="gap-1.5 text-[12px]">
            <FolderGit2 class="size-3.5" />
            {{ t('library.tabs.projects') }}
          </TabsTrigger>
          <TabsTrigger value="threads" class="gap-1.5 text-[12px]">
            <GitBranch class="size-3.5" />
            {{ t('library.tabs.threads') }}
          </TabsTrigger>
        </TabsList>
      </Tabs>
    </Teleport>

    <Teleport to="#memex-header-actions" defer>
      <Button size="sm" class="h-8 gap-2" :disabled="ingesting" @click="runIngest">
        <RefreshCw :class="['size-4', ingesting && 'animate-spin']" />
        {{ ingesting ? t('library.action.ingest_busy') : t('library.action.ingest') }}
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
        :title="t('library.sessions.facets_resize_title')"
        @mousedown="startFacetsDrag"
        @dblclick="resetFacets"
      />

      <section class="flex flex-1 min-h-0 min-w-0 flex-col overflow-hidden">
        <div
          v-if="tab === 'sessions'"
          class="flex shrink-0 items-center gap-2 px-5 pt-3 pb-2"
        >
          <div class="relative flex-1">
            <Search
              class="pointer-events-none absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground"
            />
            <Input v-model="query" class="h-9 pl-9" :placeholder="t('library.sessions.search_placeholder')" />
          </div>
          <span class="hidden whitespace-nowrap text-[11px] text-muted-foreground md:inline">
            {{ t('library.sessions.filter_summary_label', { n: activeFilterCount }) }}
            <span class="font-medium text-foreground tabular-nums">{{ librarySessions.length }}</span>
            /
            <Tooltip :delay-duration="120">
              <TooltipTrigger as-child>
                <span class="cursor-default tabular-nums">{{ formatCount(totals.sessions) }}</span>
              </TooltipTrigger>
              <TooltipContent side="top" :side-offset="6" class="px-2 py-1 text-[11px]">
                <span class="tabular-nums">{{ t('library.facets.tooltip.session_count', { n: totals.sessions.toLocaleString() }) }}</span>
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
              <DropdownMenuItem @click="sort = 'recent'">{{ t('library.sessions.sort.recent') }}</DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem @click="sort = 'duration'">{{ t('library.sessions.sort.duration') }}</DropdownMenuItem>
              <DropdownMenuItem @click="sort = 'messages'">{{ t('library.sessions.sort.messages') }}</DropdownMenuItem>
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
            v-if="!librarySessions.length"
            class="flex flex-col items-center gap-2 py-12 text-center text-[12px] text-muted-foreground"
          >
            <MessagesSquare class="size-8 text-muted-foreground/40" />
            <p v-if="hasActiveFilters">{{ t('library.sessions.empty.no_match') }}</p>
            <p v-else>{{ t('library.sessions.empty.no_data') }}</p>
          </div>

          <div
            v-else
            class="flex flex-col items-center gap-1.5 py-6 text-[12px] text-muted-foreground"
          >
            <div
              v-if="libraryHasMore"
              ref="loadMoreSentinel"
              class="flex items-center gap-2 py-1"
            >
              <RefreshCw :class="['size-3.5', loadingMore && 'animate-spin']" />
              <span>{{ loadingMore ? t('library.sessions.load_more.busy') : t('library.sessions.load_more.idle') }}</span>
            </div>
            <span v-else>
              {{ t('library.sessions.load_more.all_shown', { n: librarySessions.length }) }}
            </span>
          </div>
        </div>

        <LibraryProjectsGrid v-else-if="tab === 'projects'" @open="openProject" />

        <LibraryThreadsTab
          v-else
          :drawer-open="drawerOpen"
          @open="openSession"
        />
      </section>
    </div>

    <LibrarySessionDrawer
      :session="drawerSession"
      :open="drawerOpen"
      @update:open="(v) => (drawerOpen = v)"
    />
  </div>
</template>
