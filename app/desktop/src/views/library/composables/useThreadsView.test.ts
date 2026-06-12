import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const mockState = {
  threads: [] as any[],
}

const mockedRefresh = vi.fn(async (_limit?: number) => {})
const mockedFetchDetail = vi.fn(async (id: number) => ({
  thread: { id, name: `thread-${id}` },
  sessions: [{ id: 's1', updatedAt: '2026-06-01T10:00:00Z' }],
}))
const mockedRegenerate = vi.fn(async () => 0)
const mockedDelete = vi.fn(async (_id: number) => {})
const mockedSearch = vi.fn(async (_q: string) => 7)

vi.mock('@/stores/memex', () => ({
  get threads() {
    return mockState.threads
  },
  refreshThreads: (limit?: number) => mockedRefresh(limit),
  fetchThreadDetail: (id: number) => mockedFetchDetail(id),
  regenerateThreads: () => mockedRegenerate(),
  deleteThread: (id: number) => mockedDelete(id),
  searchThreadByQuery: (q: string) => mockedSearch(q),
}))

vi.mock('vue-sonner', () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}))

import { useThreadsView } from './useThreadsView'

const now = () => new Date().toISOString()
const daysAgo = (d: number) =>
  new Date(Date.now() - d * 24 * 60 * 60 * 1000).toISOString()

function mkThread(over: Partial<any> = {}) {
  return {
    id: 1,
    name: 'topic-A',
    summary: '',
    sessionCount: 1,
    createdAt: now(),
    updatedAt: now(),
    lastSessionAt: now(),
    firstSessionAt: now(),
    projects: ['/a'],
    adapters: ['claude_code'],
    ...over,
  }
}

beforeEach(() => {
  mockState.threads = []
  vi.clearAllMocks()
  localStorage.clear()
})

afterEach(() => {
  vi.useRealTimers()
})

describe('useThreadsView', () => {
  it('filteredThreads applies multi_project filter', () => {
    mockState.threads = [
      mkThread({ id: 1, projects: ['/a'] }),
      mkThread({ id: 2, projects: ['/a', '/b'] }),
      mkThread({ id: 3, projects: ['/a', '/b', '/c'] }),
    ]
    const view = useThreadsView()
    view.filter.value = 'multi_project'
    const ids = view.filteredThreads.value.map((t: any) => t.id)
    expect(ids).toEqual([2, 3])
    expect(view.filterCounts.value.multi_project).toBe(2)
  })

  it('filteredThreads applies recent_7d filter by lastSessionAt', () => {
    mockState.threads = [
      mkThread({ id: 1, lastSessionAt: daysAgo(2) }),
      mkThread({ id: 2, lastSessionAt: daysAgo(8) }),
      mkThread({ id: 3, lastSessionAt: daysAgo(1) }),
    ]
    const view = useThreadsView()
    view.filter.value = 'recent_7d'
    const ids = view.filteredThreads.value.map((t: any) => t.id).sort()
    expect(ids).toEqual([1, 3])
    expect(view.filterCounts.value.recent_7d).toBe(2)
  })

  it('openThread loads detail and exposes sessions', async () => {
    mockState.threads = [mkThread({ id: 42 })]
    const view = useThreadsView()
    await view.openThread(mockState.threads[0])
    expect(mockedFetchDetail).toHaveBeenCalledWith(42)
    expect(view.detailSessions.value).toHaveLength(1)
    expect(view.selectedThread.value?.id).toBe(42)
  })

  it('sheetOpen reflects selectedThread (true when set, false on clear)', () => {
    mockState.threads = [mkThread({ id: 5 })]
    const view = useThreadsView()
    expect(view.sheetOpen.value).toBe(false)
    view.selectedThread.value = mockState.threads[0]
    expect(view.sheetOpen.value).toBe(true)
    view.sheetOpen.value = false
    expect(view.selectedThread.value).toBeNull()
  })

  it('setAutoCluster persists to localStorage', () => {
    const view = useThreadsView()
    view.setAutoCluster(false)
    expect(localStorage.getItem('memex.threads.autoCluster')).toBe('false')
    expect(view.autoCluster.value).toBe(false)
  })

  it('confirmDelete calls deleteThread and clears selection if same', async () => {
    const target = mkThread({ id: 9 })
    mockState.threads = [target]
    const view = useThreadsView()
    view.selectedThread.value = target
    view.requestDelete(target)
    expect(view.deleteTarget.value?.id).toBe(9)
    await view.confirmDelete()
    expect(mockedDelete).toHaveBeenCalledWith(9)
    expect(view.selectedThread.value).toBeNull()
    expect(view.deleteTarget.value).toBeNull()
  })

  it('onSearch trims query and opens the resulting thread', async () => {
    mockState.threads = [mkThread({ id: 7 })]
    const view = useThreadsView()
    view.llmQuery.value = '  Tauri 多窗口  '
    await view.onSearch()
    expect(mockedSearch).toHaveBeenCalledWith('Tauri 多窗口')
    expect(view.llmQuery.value).toBe('')
    expect(view.selectedThread.value?.id).toBe(7)
  })

  it('hide/restoreSheetForDrawer keeps selection alive while toggling visibility', () => {
    const t = mkThread({ id: 11 })
    mockState.threads = [t]
    const view = useThreadsView()
    view.selectedThread.value = t
    expect(view.sheetOpen.value).toBe(true)

    view.hideSheetForDrawer()
    expect(view.sheetOpen.value).toBe(false)
    // selection 还在 → 关 Drawer 后可以恢复
    expect(view.selectedThread.value?.id).toBe(11)

    view.restoreSheetFromDrawer()
    expect(view.sheetOpen.value).toBe(true)
    expect(view.selectedThread.value?.id).toBe(11)
  })

  it('manually closing the sheet (sheetOpen=false) clears selection and hidden flag', () => {
    const t = mkThread({ id: 12 })
    mockState.threads = [t]
    const view = useThreadsView()
    view.selectedThread.value = t
    view.hideSheetForDrawer()
    view.sheetOpen.value = false // 用户按 ESC / 点 overlay 真关
    expect(view.selectedThread.value).toBeNull()
    expect(view.sheetHiddenForDrawer.value).toBe(false)
    expect(view.detailSessions.value).toEqual([])
  })
})
