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
import { useI18n } from '@/i18n'

export type FilterKey = 'all' | 'multi_project' | 'recent_7d'

const AUTO_CLUSTER_KEY = 'memex.threads.autoCluster'

export function useThreadsView() {
  const { t } = useI18n()

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
  // 点会话打开 Drawer 时把 Sheet 暂时收起来（不销毁状态），关 Drawer 再恢复，
  // 避免 Sheet + Drawer 两个 Dialog backdrop 叠加成「弹框嵌在抽屉里」的观感。
  const sheetHiddenForDrawer = ref(false)

  // ── 删除二次确认 ────────────────────────────────────────
  const deleteTarget = ref<ThreadRow | null>(null)
  const deleting = ref(false)

  // ── derived ─────────────────────────────────────────────
  const filterCounts = computed(() => ({
    all: threads.length,
    multi_project: threads.filter((th) => (th.projects ?? []).length >= 2).length,
    recent_7d: threads.filter((th) =>
      isWithinDays(th.lastSessionAt ?? th.updatedAt, 7),
    ).length,
  }))

  const filteredThreads = computed(() => {
    switch (filter.value) {
      case 'multi_project':
        return threads.filter((th) => (th.projects ?? []).length >= 2)
      case 'recent_7d':
        return threads.filter((th) =>
          isWithinDays(th.lastSessionAt ?? th.updatedAt, 7),
        )
      default:
        return threads.slice()
    }
  })

  const sheetOpen = computed({
    get: () => selectedThread.value !== null && !sheetHiddenForDrawer.value,
    set: (v: boolean) => {
      // 用户手动关 Sheet（按 ESC / 点 overlay）→ 真正关闭：清状态
      if (!v) {
        selectedThread.value = null
        detailSessions.value = []
        sheetHiddenForDrawer.value = false
      }
    },
  })

  function hideSheetForDrawer() {
    sheetHiddenForDrawer.value = true
  }

  function restoreSheetFromDrawer() {
    sheetHiddenForDrawer.value = false
  }

  // ── actions ─────────────────────────────────────────────
  function setAutoCluster(v: boolean) {
    autoCluster.value = v
    try {
      localStorage.setItem(AUTO_CLUSTER_KEY, String(v))
    } catch {
      /* ignore */
    }
  }

  async function openThread(target: ThreadRow) {
    selectedThread.value = target
    detailLoading.value = true
    detailSessions.value = []
    try {
      const detail = await fetchThreadDetail(target.id)
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
      const found = threads.find((x) => x.id === id)
      if (found) await openThread(found)
      toast.success(t('library.threads.toast.search_done', { query: q }))
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
        const still = threads.find((x) => x.id === selectedThread.value?.id)
        if (still) {
          await openThread(still)
        } else {
          selectedThread.value = null
          detailSessions.value = []
        }
      }
      toast.success(t('library.threads.toast.regenerated'))
    } catch (e) {
      toast.error(humanizeBackendError(e).friendly)
    } finally {
      regenerating.value = false
    }
  }

  function requestDelete(target: ThreadRow, e?: Event) {
    if (e) e.stopPropagation()
    deleteTarget.value = target
  }

  async function confirmDelete() {
    const target = deleteTarget.value
    if (!target) return
    deleting.value = true
    try {
      await deleteThread(target.id)
      if (selectedThread.value?.id === target.id) {
        selectedThread.value = null
        detailSessions.value = []
      }
      toast.success(t('library.threads.toast.deleted', { name: target.name }))
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
    sheetHiddenForDrawer,
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
    hideSheetForDrawer,
    restoreSheetFromDrawer,
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
