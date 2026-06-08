/**
 * `LibraryThreadsTab` 的状态机 + actions。
 *
 * 拆出来的目的是让 .vue 文件回到 < 200 行（用户规则要求 < 300）。
 * 这里只持有**视图态**（当前过滤、当前选中、当前打开的删除目标、当前的
 * 搜索 query 等），不直接走 IPC——IPC 仍在 stores/memex.ts 内，本 composable
 * 只是 thin orchestration。
 */
import { computed, onMounted, ref } from 'vue'
import { toast } from 'vue-sonner'
import {
  deleteThread,
  fetchThreadDetail,
  refreshThreads,
  regenerateThreads,
  searchThreadByQuery,
  threads,
} from '@/stores/memex'
import type { SessionRow, ThreadRow } from '@/types'
import { humanizeBackendError } from '@/lib/utils'

export type FilterKey = 'all' | 'multi_project' | 'recent_7d'

const AUTO_CLUSTER_KEY = 'memex.threads.autoCluster'

export function useThreadsView() {
  // ── 主搜索 ───────────────────────────────────────────────
  const llmQuery = ref('')
  const llmSearching = ref(false)

  // ── 自动 / 手动聚类 ──────────────────────────────────────
  const autoCluster = ref(readAutoClusterPref())
  const regenerating = ref(false)

  // ── 筛选 ─────────────────────────────────────────────────
  const filter = ref<FilterKey>('all')

  // ── 详情面板 ────────────────────────────────────────────
  const selectedThread = ref<ThreadRow | null>(null)
  const detailSessions = ref<SessionRow[]>([])
  const detailLoading = ref(false)

  // ── 删除二次确认 ────────────────────────────────────────
  const deleteTarget = ref<ThreadRow | null>(null)
  const deleting = ref(false)

  // ── derived ─────────────────────────────────────────────
  const filterCounts = computed(() => ({
    all: threads.length,
    multi_project: threads.filter((t) => (t.projects ?? []).length >= 2).length,
    recent_7d: threads.filter((t) =>
      isWithinDays(t.last_session_at ?? t.updated_at, 7),
    ).length,
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

  const sheetOpen = computed({
    get: () => selectedThread.value !== null,
    set: (v: boolean) => {
      if (!v) selectedThread.value = null
    },
  })

  // ── actions ─────────────────────────────────────────────
  function setAutoCluster(v: boolean) {
    autoCluster.value = v
    try {
      localStorage.setItem(AUTO_CLUSTER_KEY, String(v))
    } catch {
      /* ignore */
    }
  }

  async function openThread(t: ThreadRow) {
    selectedThread.value = t
    detailLoading.value = true
    detailSessions.value = []
    try {
      const detail = await fetchThreadDetail(t.id)
      detailSessions.value = detail?.sessions ?? []
      if (detail?.thread) selectedThread.value = detail.thread
    } finally {
      detailLoading.value = false
    }
  }

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
        const still = threads.find((t) => t.id === selectedThread.value?.id)
        if (still) {
          await openThread(still)
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

  function requestDelete(t: ThreadRow, e?: Event) {
    if (e) e.stopPropagation()
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

  function focusSearch() {
    const el = document.getElementById('threads-search-input') as HTMLInputElement | null
    el?.focus()
  }

  function cancelDelete() {
    deleteTarget.value = null
  }

  // ── lifecycle ───────────────────────────────────────────
  onMounted(async () => {
    await refreshThreads()
  })

  return {
    // 状态
    llmQuery,
    llmSearching,
    autoCluster,
    regenerating,
    filter,
    selectedThread,
    detailSessions,
    detailLoading,
    deleteTarget,
    deleting,
    sheetOpen,
    // derived
    filterCounts,
    filteredThreads,
    threadsRef: threads,
    // actions
    setAutoCluster,
    openThread,
    onSearch,
    applySuggestion,
    onRegenerate,
    requestDelete,
    confirmDelete,
    cancelDelete,
    focusSearch,
  }
}

function readAutoClusterPref(): boolean {
  try {
    return localStorage.getItem(AUTO_CLUSTER_KEY) !== 'false'
  } catch {
    return true
  }
}

function isWithinDays(iso: string | null | undefined, days: number): boolean {
  if (!iso) return false
  const t = Date.parse(iso)
  if (Number.isNaN(t)) return false
  return Date.now() - t <= days * 24 * 60 * 60 * 1000
}
