import { onMounted, onScopeDispose, ref } from 'vue'
import type { Stats } from '@/types'
import { useMemex } from './useMemex'
import { stats as storeStats } from '@/stores/memex'

// 模块单例：直接复用 stores/memex 里的 stats ref，所有调用方共享同一份数据
// + 同一个轮询定时器（refCount=0 时停轮询）
const stats = storeStats
const loading = ref(false)
const lastError = ref<string | null>(null)

let timer: ReturnType<typeof setInterval> | null = null
let refCount = 0

const POLL_INTERVAL_MS = 10_000

const memex = useMemex()

async function refresh(): Promise<Stats | null> {
  if (loading.value) return stats.value
  loading.value = true
  try {
    const v = await memex.getStats()
    stats.value = v
    lastError.value = null
    return v
  } catch (e) {
    lastError.value = String(e)
    return stats.value
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
 * 轮询当前 Memex 数据库统计。模块级单例 —— 多处调用 useStats 共享同一份数据
 * 和同一个 setInterval；当所有持有者都卸载（refCount 回到 0）时自动停轮询，
 * 避免在 tray-popup 临时窗口里也跑后台心跳。
 */
export function useStats() {
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
  return { stats, loading, lastError, refresh }
}

/** 给非组件代码（test / event listener）用的直接访问入口 */
export const statsState = { stats, loading, lastError, refresh }
