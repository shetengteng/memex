import { invoke } from '@tauri-apps/api/core'
import type { Stats, SessionRow, SearchResult, SessionDetail, StatsBreakdown, TimelineEntry } from '@/types'

export function useMemex() {
  async function getStats(): Promise<Stats> {
    return invoke<Stats>('get_stats')
  }

  async function getBreakdown(): Promise<StatsBreakdown> {
    return invoke<StatsBreakdown>('get_breakdown')
  }

  async function getTimeline(days = 30): Promise<TimelineEntry[]> {
    return invoke<TimelineEntry[]>('get_timeline', { days })
  }

  async function listRecent(limit = 20, offset = 0): Promise<SessionRow[]> {
    return invoke<SessionRow[]>('list_recent', { limit, offset })
  }

  async function searchMemex(query: string, limit = 20, offset = 0): Promise<SearchResult[]> {
    return invoke<SearchResult[]>('search_memex', { query, limit, offset })
  }

  async function getSession(sessionId: string): Promise<SessionDetail | null> {
    return invoke<SessionDetail | null>('get_session', { sessionId })
  }

  async function toggleAdapter(adapter: string, enabled: boolean): Promise<void> {
    return invoke<void>('toggle_adapter', { adapter, enabled })
  }

  async function getConfig(key: string): Promise<string | null> {
    return invoke<string | null>('get_config', { key })
  }

  async function setConfig(key: string, value: string): Promise<void> {
    return invoke<void>('set_config', { key, value })
  }

  return { getStats, getBreakdown, getTimeline, listRecent, searchMemex, getSession, toggleAdapter, getConfig, setConfig }
}
