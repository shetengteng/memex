import { onMounted, onScopeDispose, ref } from 'vue'
import type { DaemonStatus } from '@/types'
import { useMemex } from './useMemex'
import { daemon as storeDaemon } from '@/stores/memex'

// 复用 stores/memex 的 daemon ref，让 view 和 composable 共享同一份状态
const status = storeDaemon
const loading = ref(false)
const lastError = ref<string | null>(null)

let timer: ReturnType<typeof setInterval> | null = null
let refCount = 0

const POLL_INTERVAL_MS = 5_000

const memex = useMemex()

async function refresh(): Promise<DaemonStatus | null> {
  if (loading.value) return status.value
  loading.value = true
  try {
    const v = await memex.daemonStatus()
    status.value = v
    lastError.value = null
    return v
  } catch (e) {
    lastError.value = String(e)
    return status.value
  } finally {
    loading.value = false
  }
}

async function restart(): Promise<DaemonStatus | null> {
  loading.value = true
  try {
    const v = await memex.daemonRestart()
    status.value = v
    lastError.value = null
    return v
  } catch (e) {
    lastError.value = String(e)
    return status.value
  } finally {
    loading.value = false
  }
}

function startPolling() {
  if (timer) return
  timer = setInterval(() => {
    void refresh()
  }, POLL_INTERVAL_MS)
}

function stopPolling() {
  if (timer) {
    clearInterval(timer)
    timer = null
  }
}

/**
 * Daemon 心跳，5s 轮询 `daemon_status`。同 useStats 一样是模块单例 + 自动
 * 引用计数：所有持有者都卸载后停轮询。
 */
export function useDaemon() {
  onMounted(() => {
    refCount += 1
    void refresh()
    startPolling()
  })
  onScopeDispose(() => {
    refCount -= 1
    if (refCount <= 0) {
      refCount = 0
      stopPolling()
    }
  })
  return { status, loading, lastError, refresh, restart }
}

export const daemonState = { status, loading, lastError, refresh, restart }
