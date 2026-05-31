import { invoke } from '@tauri-apps/api/core'
import type { Stats, SessionRow, SearchResult } from '@/types'

export function useMemex() {
  async function getStats(): Promise<Stats> {
    return invoke<Stats>('get_stats')
  }

  async function listRecent(limit = 10): Promise<SessionRow[]> {
    return invoke<SessionRow[]>('list_recent', { limit })
  }

  async function searchMemex(query: string, limit = 20): Promise<SearchResult[]> {
    return invoke<SearchResult[]>('search_memex', { query, limit })
  }

  return { getStats, listRecent, searchMemex }
}
