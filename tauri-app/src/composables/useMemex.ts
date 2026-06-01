import { invoke } from '@tauri-apps/api/core'
import type { Stats, SessionRow, SearchResult, SessionDetail, StatsBreakdown, TimelineEntry, ProjectSummary, AggregateSummary, DaemonStatus } from '@/types'

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

  async function retrySummary(sessionId: string): Promise<boolean> {
    return invoke<boolean>('retry_summary', { sessionId })
  }

  async function batchSummarize(): Promise<number> {
    return invoke<number>('batch_summarize')
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

  async function listProjects(): Promise<ProjectSummary[]> {
    return invoke<ProjectSummary[]>('list_projects')
  }

  async function listReports(scope: 'daily' | 'weekly', limit = 60): Promise<AggregateSummary[]> {
    return invoke<AggregateSummary[]>('list_reports', { scope, limit })
  }

  async function regenerateReport(scope: 'daily' | 'weekly'): Promise<AggregateSummary | null> {
    return invoke<AggregateSummary | null>('regenerate_report', { scope })
  }

  async function daemonStatus(): Promise<DaemonStatus> {
    return invoke<DaemonStatus>('daemon_status')
  }

  async function daemonRestart(): Promise<DaemonStatus> {
    return invoke<DaemonStatus>('daemon_restart')
  }

  return {
    getStats, getBreakdown, getTimeline, listRecent, searchMemex, getSession,
    retrySummary, batchSummarize, toggleAdapter, getConfig, setConfig,
    listProjects, listReports, regenerateReport, daemonStatus, daemonRestart,
  }
}
