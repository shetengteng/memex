import { invoke } from '@tauri-apps/api/core'
import type { Stats, SessionRow, SearchResult, SessionDetail, StatsBreakdown, TimelineEntry, ProjectSummary, AggregateSummary, DaemonStatus, CliStatus, LlmTestResult, LlmProvider, ProviderTestResult, DoctorRunResult, ReflectEntry, ReflectDetail, ReflectRunResult, WorkloadReport, SystemResetResult, IdeStatus, SkillStatus, HookStatus, UpdateInfo } from '@/types'

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

  /**
   * 中断当前批量摘要任务。返回值表示「中断时是否真的有任务在跑」。
   * 工作线程在跑完当前正在处理的那条之后退出，再 emit summary-progress(aborted=true)。
   */
  async function abortSummarize(): Promise<boolean> {
    return invoke<boolean>('abort_summarize')
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

  async function listReports(scope: 'daily' | 'weekly' | 'monthly', limit = 60): Promise<AggregateSummary[]> {
    return invoke<AggregateSummary[]>('list_reports', { scope, limit })
  }

  async function regenerateReport(scope: 'daily' | 'weekly' | 'monthly', scopeKey?: string): Promise<AggregateSummary | null> {
    return invoke<AggregateSummary | null>('regenerate_report', { scope, scopeKey })
  }

  async function daemonStatus(): Promise<DaemonStatus> {
    return invoke<DaemonStatus>('daemon_status')
  }

  async function daemonRestart(): Promise<DaemonStatus> {
    return invoke<DaemonStatus>('daemon_restart')
  }

  async function triggerIngest(adapter?: string): Promise<{ messages_ingested: number; chunks_created: number }> {
    return invoke<{ messages_ingested: number; chunks_created: number }>('trigger_ingest', { adapter })
  }

  async function runDoctor(): Promise<DoctorRunResult> {
    return invoke<DoctorRunResult>('doctor_run')
  }

  async function cliStatus(): Promise<CliStatus> {
    return invoke<CliStatus>('cli_status')
  }

  async function cliInstall(): Promise<CliStatus> {
    return invoke<CliStatus>('cli_install')
  }

  async function cliUninstall(): Promise<CliStatus> {
    return invoke<CliStatus>('cli_uninstall')
  }

  async function llmTestOllama(): Promise<LlmTestResult> {
    return invoke<LlmTestResult>('llm_test_ollama')
  }

  async function llmProviderList(): Promise<LlmProvider[]> {
    return invoke<LlmProvider[]>('llm_provider_list')
  }

  async function llmProviderUpsert(provider: Partial<LlmProvider> & { id: string; name: string; kind: string; baseUrl: string }): Promise<LlmProvider> {
    return invoke<LlmProvider>('llm_provider_upsert', { provider })
  }

  async function llmProviderDelete(id: string): Promise<number> {
    return invoke<number>('llm_provider_delete', { id })
  }

  async function llmProviderTest(id: string): Promise<ProviderTestResult> {
    return invoke<ProviderTestResult>('llm_provider_test', { id })
  }

  async function llmProviderTestDraft(name: string, kind: string, baseUrl: string, model: string, apiKey: string): Promise<ProviderTestResult> {
    return invoke<ProviderTestResult>('llm_provider_test_draft', { name, kind, baseUrl, model, apiKey })
  }

  async function llmListModels(kind: string, baseUrl: string, apiKey: string): Promise<string[]> {
    return invoke<string[]>('llm_list_models', { kind, baseUrl, apiKey })
  }

  async function reflectList(): Promise<ReflectEntry[]> {
    return invoke<ReflectEntry[]>('reflect_list')
  }

  async function reflectGet(scopeKey: string): Promise<ReflectDetail | null> {
    return invoke<ReflectDetail | null>('reflect_get', { scopeKey })
  }

  async function reflectRun(period: string): Promise<ReflectRunResult> {
    return invoke<ReflectRunResult>('reflect_run', { period })
  }

  async function getWorkload(days = 30): Promise<WorkloadReport> {
    return invoke<WorkloadReport>('get_workload', { days })
  }

  async function systemResetIndex(): Promise<SystemResetResult> {
    return invoke<SystemResetResult>('system_reset_index')
  }

  async function systemResetAll(): Promise<SystemResetResult> {
    return invoke<SystemResetResult>('system_reset_all')
  }

  async function ideListStatus(): Promise<IdeStatus[]> {
    return invoke<IdeStatus[]>('ide_list_status')
  }

  async function ideInstall(ide: string): Promise<IdeStatus> {
    return invoke<IdeStatus>('ide_install', { ide })
  }

  async function ideUninstall(ide: string): Promise<IdeStatus> {
    return invoke<IdeStatus>('ide_uninstall', { ide })
  }

  async function skillListStatus(): Promise<SkillStatus[]> {
    return invoke<SkillStatus[]>('skill_list_status')
  }

  async function skillInstall(ide: string): Promise<SkillStatus> {
    return invoke<SkillStatus>('skill_install', { ide })
  }

  async function skillUninstall(ide: string): Promise<SkillStatus> {
    return invoke<SkillStatus>('skill_uninstall', { ide })
  }

  async function hookListStatus(): Promise<HookStatus[]> {
    return invoke<HookStatus[]>('hook_list_status')
  }

  async function hookInstall(ide: string): Promise<HookStatus> {
    return invoke<HookStatus>('hook_install', { ide })
  }

  async function hookUninstall(ide: string): Promise<HookStatus> {
    return invoke<HookStatus>('hook_uninstall', { ide })
  }

  async function checkForUpdates(): Promise<UpdateInfo> {
    return invoke<UpdateInfo>('check_for_updates')
  }

  return {
    getStats, getBreakdown, getTimeline, listRecent, searchMemex, getSession,
    retrySummary, batchSummarize, abortSummarize, toggleAdapter, getConfig, setConfig,
    listProjects, listReports, regenerateReport, daemonStatus, daemonRestart,
    triggerIngest, runDoctor, cliStatus, cliInstall, cliUninstall,
    llmTestOllama,
    llmProviderList, llmProviderUpsert, llmProviderDelete,
    llmProviderTest, llmProviderTestDraft, llmListModels,
    reflectList, reflectGet, reflectRun,
    getWorkload,
    systemResetIndex, systemResetAll,
    ideListStatus, ideInstall, ideUninstall,
    skillListStatus, skillInstall, skillUninstall,
    hookListStatus, hookInstall, hookUninstall,
    checkForUpdates,
  }
}
