/**
 * 资料库视图的会话列表。
 *
 * 跟全局 `sessions[]`（最近 200 条、被 Today / Popup / Threads 共享）解耦，
 * 把 adapter/project/time/summary/query/sort 全部下推到后端 SQL 一次过滤，
 * 结果独立维护在 `librarySessions[]`，不互相污染。
 *
 * 修复了 facets counts 走全表、filter 走 200 条窗口的不一致 bug
 * （详见 storage::db::tests::sessions::filtered 与配套 SQL 实现）。
 */

import { reactive, ref } from 'vue'
import type { SessionListFilter } from '@/types'
import { useMemex } from '@/composables/useMemex'
import { rowToSession, type Session } from './memex'

export const librarySessions: Session[] = reactive([])
export const libraryHasMore = ref(false)

const LIBRARY_PAGE_SIZE = 200

/**
 * 用当前 filter 重拉资料库第一页，把 `librarySessions` 全量替换。
 * filter 任何字段变化都应该走这条 —— SQL 端 LIMIT 决定有没有下一页，
 * 前端不再做内存裁剪。
 */
export async function reloadLibrarySessions(filter: SessionListFilter): Promise<void> {
  const memex = useMemex()
  try {
    const rows = await memex.listSessionsFiltered(filter, LIBRARY_PAGE_SIZE, 0)
    librarySessions.splice(0, librarySessions.length, ...rows.map(rowToSession))
    libraryHasMore.value = rows.length === LIBRARY_PAGE_SIZE
  } catch {
    // 静默与其它 refresh* 保持一致；UI 层 watch 报错会触发 retry，
    // toast 由更高层 ingest / 用户主动操作的入口去抛
  }
}

/**
 * 续拉资料库下一页（基于 `librarySessions.length` 当 offset）。
 * 必须带相同 filter，否则会与首页结果错配。
 * 返回 { loaded, hasMore } 给调用方做无限滚动的"已加载全部"判断。
 */
export async function loadMoreLibrarySessions(
  filter: SessionListFilter,
  pageSize = 100,
): Promise<{ loaded: number; hasMore: boolean }> {
  const memex = useMemex()
  try {
    const rows = await memex.listSessionsFiltered(filter, pageSize, librarySessions.length)
    if (rows.length === 0) {
      libraryHasMore.value = false
      return { loaded: 0, hasMore: false }
    }
    librarySessions.push(...rows.map(rowToSession))
    const hasMore = rows.length === pageSize
    libraryHasMore.value = hasMore
    return { loaded: rows.length, hasMore }
  } catch {
    return { loaded: 0, hasMore: false }
  }
}
