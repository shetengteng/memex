import { onMounted, onScopeDispose, ref } from 'vue'
import type { NotificationEntry } from '@/types'
import { useMemex } from './useMemex'
import { humanizeBackendError } from '@/lib/utils'

const items = ref<NotificationEntry[]>([])
const unreadCount = ref(0)
const loading = ref(false)
const lastError = ref<string | null>(null)

let timer: ReturnType<typeof setInterval> | null = null
let refCount = 0

const POLL_INTERVAL_MS = 3_000
const LIST_LIMIT = 50

const memex = useMemex()

async function refreshUnread(): Promise<void> {
  try {
    unreadCount.value = await memex.notificationsUnreadCount()
    lastError.value = null
  } catch (e) {
    lastError.value = humanizeBackendError(e).friendly
  }
}

async function refreshList(): Promise<void> {
  loading.value = true
  try {
    items.value = await memex.notificationsList(LIST_LIMIT, false)
    unreadCount.value = items.value.filter((n) => n.read_at === null).length
    lastError.value = null
  } catch (e) {
    lastError.value = humanizeBackendError(e).friendly
  } finally {
    loading.value = false
  }
}

async function markRead(id: number): Promise<void> {
  const changed = await memex.notificationMarkRead(id)
  if (!changed) return
  const idx = items.value.findIndex((n) => n.id === id)
  if (idx >= 0 && items.value[idx].read_at === null) {
    items.value[idx] = { ...items.value[idx], read_at: new Date().toISOString() }
    unreadCount.value = Math.max(0, unreadCount.value - 1)
  }
}

async function markAllRead(): Promise<void> {
  const n = await memex.notificationsMarkAllRead()
  if (n <= 0) return
  const now = new Date().toISOString()
  items.value = items.value.map((it) => (it.read_at === null ? { ...it, read_at: now } : it))
  unreadCount.value = 0
}

async function markUnread(id: number): Promise<void> {
  const changed = await memex.notificationMarkUnread(id)
  if (!changed) return
  const idx = items.value.findIndex((n) => n.id === id)
  if (idx >= 0 && items.value[idx].read_at !== null) {
    items.value[idx] = { ...items.value[idx], read_at: null }
    unreadCount.value = unreadCount.value + 1
  }
}

async function remove(id: number): Promise<void> {
  const ok = await memex.notificationDelete(id)
  if (!ok) return
  const target = items.value.find((n) => n.id === id)
  items.value = items.value.filter((n) => n.id !== id)
  if (target && target.read_at === null) {
    unreadCount.value = Math.max(0, unreadCount.value - 1)
  }
}

async function clearAll(): Promise<void> {
  const n = await memex.notificationsClearAll()
  if (n <= 0) return
  items.value = []
  unreadCount.value = 0
}

function startPolling() {
  if (timer) return
  timer = setInterval(() => {
    void refreshUnread()
  }, POLL_INTERVAL_MS)
}

function stopPolling() {
  if (timer) {
    clearInterval(timer)
    timer = null
  }
}

/**
 * 通知中心轮询 composable。模块单例 + 引用计数：所有持有者卸载后停止轮询。
 *
 * 后台 3s 轮询 `notifications_unread_count`（仅 1 个 COUNT 查询，开销可忽略）
 * 给 Bell badge 用。打开 Popover 时调用方应主动 `refreshList()` 拉具体列表，
 * 避免每次轮询都把 50 条 payload 序列化过 IPC。
 *
 * `markRead` / `markAllRead` 走「乐观更新」—— 立即改本地 ref，IPC 成功就保留，
 * 失败会被下一轮 `refreshUnread` 自动纠正（unreadCount 与 db 重新对齐）。
 */
export function useNotifications() {
  onMounted(() => {
    refCount += 1
    void refreshUnread()
    startPolling()
  })
  onScopeDispose(() => {
    refCount -= 1
    if (refCount <= 0) {
      refCount = 0
      stopPolling()
    }
  })
  return {
    items,
    unreadCount,
    loading,
    lastError,
    refreshUnread,
    refreshList,
    markRead,
    markAllRead,
    markUnread,
    remove,
    clearAll,
  }
}

export const notificationsState = {
  items,
  unreadCount,
  loading,
  lastError,
  refreshUnread,
  refreshList,
  markRead,
  markAllRead,
  markUnread,
  remove,
  clearAll,
}
