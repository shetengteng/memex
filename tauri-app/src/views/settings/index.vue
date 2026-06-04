<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useMemex } from '@/composables/useMemex'
import { Switch } from '@/components/ui/switch'
import { Separator } from '@/components/ui/separator'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from '@/components/ui/collapsible'
import {
  RefreshCw,
  CheckCircle2,
  AlertCircle,
  Download,
  Terminal,
  Copy,
  ChevronDown,
  Database,
  Cpu,
  Languages,
  Sparkles,
  Plug,
  Shield,
  Info,
  FlaskConical,
  Stethoscope,
  Trash2,
  TriangleAlert,
} from 'lucide-vue-next'
import { openUrl } from '@tauri-apps/plugin-opener'
import { useI18n, setLocale, LOCALE_OPTIONS, type Locale } from '@/i18n'
import type { CliStatus, LlmTestResult, DoctorRunResult } from '@/types'
import LlmProviders from './LlmProviders.vue'
import IdeIcon from '@/components/IdeIcon.vue'

const { t, locale } = useI18n()
const { toggleAdapter: ipcToggleAdapter, getConfig, setConfig, cliStatus: ipcCliStatus, cliInstall: ipcCliInstall, cliUninstall: ipcCliUninstall, llmTestOllama, triggerIngest, runDoctor, systemResetIndex, systemResetAll } = useMemex()
const APP_VERSION = __APP_VERSION__
const RELEASES_LATEST_PAGE = 'https://github.com/shetengteng/memex/releases/latest'

// IDE 集成那栏的 key 用连字符（cursor / claude-code / codex / opencode），
// 而 adapter 那栏的 key 用下划线（claude_code / continue_dev）。IdeIcon 内部
// 字典只用下划线版本（因为大多数地方都是这个 source），这里做一次映射。
function normalizeIdeKey(ide: string): string {
  if (ide === 'claude-code') return 'claude_code'
  return ide
}

type UpdateStatus = 'idle' | 'checking' | 'latest' | 'outdated' | 'error'

interface UpdateInfo {
  latest_tag: string
  html_url: string
}

const updateStatus = ref<UpdateStatus>('idle')
const remoteVersion = ref<string>('')
const remoteUrl = ref<string>('')
const errorMessage = ref<string>('')

function compareVersion(remote: string, local: string): number {
  const r = remote.replace(/^v/, '').split('.').map(Number)
  const l = local.replace(/^v/, '').split('.').map(Number)
  for (let i = 0; i < 3; i++) {
    const a = r[i] ?? 0
    const b = l[i] ?? 0
    if (a > b) return 1
    if (a < b) return -1
  }
  return 0
}

async function checkForUpdates() {
  if (updateStatus.value === 'checking') return
  updateStatus.value = 'checking'
  remoteVersion.value = ''
  remoteUrl.value = ''
  errorMessage.value = ''
  try {
    const info = await invoke<UpdateInfo>('check_for_updates')
    const tag = (info.latest_tag || '').trim()
    if (!tag) {
      throw new Error('no tag in response')
    }
    remoteVersion.value = tag.replace(/^v/, '')
    remoteUrl.value = info.html_url || RELEASES_LATEST_PAGE
    updateStatus.value = compareVersion(remoteVersion.value, APP_VERSION) > 0 ? 'outdated' : 'latest'
  } catch (e) {
    console.error('check for updates failed:', e)
    errorMessage.value = e instanceof Error ? e.message : String(e)
    updateStatus.value = 'error'
  }
}

async function openReleasePage() {
  try {
    await openUrl(remoteUrl.value || RELEASES_LATEST_PAGE)
  } catch (e) {
    console.error('open release page failed:', e)
  }
}

async function changeLocale(next: Locale) {
  if (next === locale.value) return
  await setLocale(next)
}

type AdapterScanState = 'idle' | 'running' | 'done' | 'empty' | 'error'
interface AdapterRow {
  key: string
  label: string
  enabled: boolean
  scanState: AdapterScanState
  scanMsgs: number
  scanError: string
}

const adapters = ref<AdapterRow[]>([
  { key: 'claude_code', label: 'Claude Code', enabled: true, scanState: 'idle', scanMsgs: 0, scanError: '' },
  { key: 'cursor', label: 'Cursor', enabled: true, scanState: 'idle', scanMsgs: 0, scanError: '' },
  { key: 'codex', label: 'Codex', enabled: true, scanState: 'idle', scanMsgs: 0, scanError: '' },
  { key: 'opencode', label: 'OpenCode', enabled: true, scanState: 'idle', scanMsgs: 0, scanError: '' },
  { key: 'aider', label: 'Aider', enabled: true, scanState: 'idle', scanMsgs: 0, scanError: '' },
  { key: 'continue_dev', label: 'Continue', enabled: true, scanState: 'idle', scanMsgs: 0, scanError: '' },
  { key: 'cline', label: 'Cline', enabled: true, scanState: 'idle', scanMsgs: 0, scanError: '' },
])

const activeAdapterCount = computed(() => adapters.value.filter((a) => a.enabled).length)

const privacy = ref({ autoRedact: false, privateFromMcp: false })
const configLoaded = ref(false)
const llm = ref({
  ollamaEnabled: false,
  ollamaModel: 'qwen2.5:7b',
  ollamaAvailable: false,
  ollamaChecking: false,
})

type TestState = 'idle' | 'testing' | 'ok' | 'fail'
const ollamaTest = ref<{ state: TestState; latency: number; error: string }>({ state: 'idle', latency: 0, error: '' })

const ollamaLabel = computed(() =>
  t('settings.llm.ollama_label', { model: llm.value.ollamaModel }),
)

// 折叠状态：默认 Data Sources / IDE / System 都折起来，保持页面简洁；用户点开后状态保留
const openDataSources = ref(false)
const openIde = ref(false)
const openSystem = ref(false)

async function checkOllamaAvailability(): Promise<boolean> {
  try {
    const resp = await fetch('http://127.0.0.1:11434/api/tags', { signal: AbortSignal.timeout(3000) })
    return resp.ok
  } catch {
    return false
  }
}

onMounted(async () => {
  for (const a of adapters.value) {
    try {
      const val = await getConfig(`adapter.${a.key}.enabled`)
      if (val !== null) a.enabled = val === 'true'
    } catch { /* default */ }
  }
  try {
    const v = await getConfig('llm.ollama_enabled')
    if (v !== null) llm.value.ollamaEnabled = v === 'true'
  } catch { /* default */ }
  try {
    const m = await getConfig('llm.ollama_model')
    if (m) llm.value.ollamaModel = m
  } catch { /* default */ }

  llm.value.ollamaAvailable = await checkOllamaAvailability()
  configLoaded.value = true

  await refreshIdeStatuses()
  await refreshCliStatus()
})

async function setAdapter(key: string, value: boolean) {
  const a = adapters.value.find((x) => x.key === key)
  if (!a || a.enabled === value) return
  const prev = a.enabled
  a.enabled = value
  try {
    await ipcToggleAdapter(key, value)
  } catch {
    a.enabled = prev
  }
}

const rescanState = ref<{ state: 'idle' | 'running' | 'done' | 'empty' | 'error'; msgs: number; error: string }>({
  state: 'idle',
  msgs: 0,
  error: '',
})

async function startRescan() {
  if (rescanState.value.state === 'running') return
  rescanState.value = { state: 'running', msgs: 0, error: '' }
  try {
    const result = await triggerIngest()
    if (result.messages_ingested > 0) {
      rescanState.value = { state: 'done', msgs: result.messages_ingested, error: '' }
    } else {
      rescanState.value = { state: 'empty', msgs: 0, error: '' }
    }
  } catch (e) {
    rescanState.value = { state: 'error', msgs: 0, error: e instanceof Error ? e.message : String(e) }
  }
}

async function rescanAdapter(row: AdapterRow) {
  if (row.scanState === 'running') return
  row.scanState = 'running'
  row.scanMsgs = 0
  row.scanError = ''
  try {
    const result = await triggerIngest(row.key)
    if (result.messages_ingested > 0) {
      row.scanState = 'done'
      row.scanMsgs = result.messages_ingested
    } else {
      row.scanState = 'empty'
    }
  } catch (e) {
    row.scanState = 'error'
    row.scanError = e instanceof Error ? e.message : String(e)
  }
}

const doctorRunning = ref(false)
const doctorResult = ref<DoctorRunResult | null>(null)
const doctorError = ref<string>('')

async function startDoctor() {
  if (doctorRunning.value) return
  doctorRunning.value = true
  doctorError.value = ''
  try {
    doctorResult.value = await runDoctor()
  } catch (e) {
    doctorError.value = e instanceof Error ? e.message : String(e)
    doctorResult.value = null
  } finally {
    doctorRunning.value = false
  }
}

function doctorCursorMessage(probe: DoctorRunResult['cursor_probe']): string {
  switch (probe.status) {
    case 'ok':
      return t('settings.doctor.cursor_ok', { count: probe.composer_count })
    case 'not_found':
      return t('settings.doctor.cursor_not_found')
    case 'permission_denied':
      return t('settings.doctor.cursor_permission')
    case 'error':
      return t('settings.doctor.cursor_error', { msg: probe.message })
  }
}

function doctorCursorTone(probe: DoctorRunResult['cursor_probe']): 'ok' | 'warn' | 'error' | 'muted' {
  switch (probe.status) {
    case 'ok': return 'ok'
    case 'not_found': return 'muted'
    case 'permission_denied': return 'error'
    case 'error': return 'error'
  }
}

async function setOllama(value: boolean) {
  if (llm.value.ollamaEnabled === value) return
  llm.value.ollamaEnabled = value
  try {
    await setConfig('llm.ollama_enabled', String(value))
  } catch {
    llm.value.ollamaEnabled = !value
    return
  }
  if (value) {
    await runOllamaTest()
  } else {
    ollamaTest.value = { state: 'idle', latency: 0, error: '' }
  }
}

async function runOllamaTest() {
  ollamaTest.value = { state: 'testing', latency: 0, error: '' }
  try {
    const r: LlmTestResult = await llmTestOllama()
    if (r.ok) {
      ollamaTest.value = { state: 'ok', latency: r.latency_ms, error: '' }
      llm.value.ollamaAvailable = true
    } else {
      ollamaTest.value = { state: 'fail', latency: r.latency_ms, error: r.error ?? 'unknown' }
      llm.value.ollamaAvailable = false
    }
  } catch (e) {
    ollamaTest.value = { state: 'fail', latency: 0, error: e instanceof Error ? e.message : String(e) }
    llm.value.ollamaAvailable = false
  }
}

async function setPrivacy(field: 'autoRedact' | 'privateFromMcp', value: boolean) {
  if (privacy.value[field] === value) return
  const keyMap = { autoRedact: 'privacy.auto_redact', privateFromMcp: 'privacy.private_from_mcp' }
  const prev = privacy.value[field]
  privacy.value[field] = value
  try {
    await setConfig(keyMap[field], String(value))
  } catch {
    privacy.value[field] = prev
  }
}

interface IdeStatus {
  ide: string
  config_path: string
  config_exists: boolean
  installed: boolean
  command: string | null
}

interface SkillStatus {
  ide: string
  dest_path: string
  installed: boolean
  size: number | null
}

interface HookStatus {
  ide: string
  supported: boolean
  installed: boolean
  config_path: string
  wrapper_path: string | null
}

interface IdeRow {
  ide: string
  label: string
  mcpStatus: IdeStatus | null
  skillStatus: SkillStatus | null
  hookStatus: HookStatus | null
  mcpLoading: boolean
  skillLoading: boolean
  hookLoading: boolean
  mcpError: string
  skillError: string
  hookError: string
}

const ideRows = ref<IdeRow[]>([
  { ide: 'cursor', label: 'Cursor', mcpStatus: null, skillStatus: null, hookStatus: null, mcpLoading: false, skillLoading: false, hookLoading: false, mcpError: '', skillError: '', hookError: '' },
  { ide: 'claude-code', label: 'Claude Code', mcpStatus: null, skillStatus: null, hookStatus: null, mcpLoading: false, skillLoading: false, hookLoading: false, mcpError: '', skillError: '', hookError: '' },
  { ide: 'codex', label: 'Codex', mcpStatus: null, skillStatus: null, hookStatus: null, mcpLoading: false, skillLoading: false, hookLoading: false, mcpError: '', skillError: '', hookError: '' },
  { ide: 'opencode', label: 'OpenCode', mcpStatus: null, skillStatus: null, hookStatus: null, mcpLoading: false, skillLoading: false, hookLoading: false, mcpError: '', skillError: '', hookError: '' },
])

// IDE 集成完整度：MCP 和 Skill 都装才算 1 个
const installedIdeCount = computed(() =>
  ideRows.value.filter((r) => r.mcpStatus?.installed && r.skillStatus?.installed).length,
)

async function refreshIdeStatuses() {
  try {
    const [mcps, skills, hooks] = await Promise.all([
      invoke<IdeStatus[]>('ide_list_status'),
      invoke<SkillStatus[]>('skill_list_status'),
      invoke<HookStatus[]>('hook_list_status'),
    ])
    for (const s of mcps) {
      const row = ideRows.value.find((r) => r.ide === s.ide)
      if (row) row.mcpStatus = s
    }
    for (const s of skills) {
      const row = ideRows.value.find((r) => r.ide === s.ide)
      if (row) row.skillStatus = s
    }
    for (const h of hooks) {
      const row = ideRows.value.find((r) => r.ide === h.ide)
      if (row) row.hookStatus = h
    }
  } catch (e) {
    console.warn('ide/skill/hook list_status failed', e)
  }
}

async function toggleMcp(row: IdeRow, next: boolean) {
  if (row.mcpLoading) return
  const prev = row.mcpStatus
  row.mcpLoading = true
  row.mcpError = ''
  try {
    const cmd = next ? 'ide_install' : 'ide_uninstall'
    row.mcpStatus = await invoke<IdeStatus>(cmd, { ide: row.ide })
  } catch (e) {
    row.mcpStatus = prev
    row.mcpError = e instanceof Error ? e.message : String(e)
  } finally {
    row.mcpLoading = false
  }
}

async function toggleSkill(row: IdeRow, next: boolean) {
  if (row.skillLoading) return
  const prev = row.skillStatus
  row.skillLoading = true
  row.skillError = ''
  try {
    const cmd = next ? 'skill_install' : 'skill_uninstall'
    row.skillStatus = await invoke<SkillStatus>(cmd, { ide: row.ide })
  } catch (e) {
    row.skillStatus = prev
    row.skillError = e instanceof Error ? e.message : String(e)
  } finally {
    row.skillLoading = false
  }
}

async function toggleHook(row: IdeRow, next: boolean) {
  if (row.hookLoading) return
  if (row.hookStatus && !row.hookStatus.supported) return
  const prev = row.hookStatus
  row.hookLoading = true
  row.hookError = ''
  try {
    const cmd = next ? 'hook_install' : 'hook_uninstall'
    row.hookStatus = await invoke<HookStatus>(cmd, { ide: row.ide })
  } catch (e) {
    row.hookStatus = prev
    row.hookError = e instanceof Error ? e.message : String(e)
  } finally {
    row.hookLoading = false
  }
}

// ----- CLI 安装到 PATH（仿 VS Code 的 "Install 'code' command in PATH"）-----
const cli = ref<CliStatus | null>(null)
const cliLoading = ref(false)
const cliError = ref('')
const cliCopied = ref(false)

async function refreshCliStatus() {
  try {
    cli.value = await ipcCliStatus()
  } catch (e) {
    console.warn('cli_status failed', e)
  }
}

async function installCli() {
  if (cliLoading.value) return
  cliLoading.value = true
  cliError.value = ''
  try {
    cli.value = await ipcCliInstall()
  } catch (e) {
    cliError.value = e instanceof Error ? e.message : String(e)
  } finally {
    cliLoading.value = false
  }
}

async function uninstallCli() {
  if (cliLoading.value) return
  cliLoading.value = true
  cliError.value = ''
  try {
    cli.value = await ipcCliUninstall()
  } catch (e) {
    cliError.value = e instanceof Error ? e.message : String(e)
  } finally {
    cliLoading.value = false
  }
}

async function copyExportHint() {
  if (!cli.value) return
  try {
    await navigator.clipboard.writeText(cli.value.path_export_hint)
    cliCopied.value = true
    setTimeout(() => { cliCopied.value = false }, 1500)
  } catch (e) {
    console.warn('clipboard write failed', e)
  }
}

// ----- Ollama 引导 (未装时显示下载卡片) -----
const OLLAMA_DOWNLOAD_URL = 'https://ollama.com/download'
const OLLAMA_BREW_CMD = 'brew install ollama'
const ollamaBrewCopied = ref(false)

async function openOllamaDownload() {
  try {
    await openUrl(OLLAMA_DOWNLOAD_URL)
  } catch (e) {
    console.warn('open ollama download failed', e)
  }
}

async function copyBrewCmd() {
  try {
    await navigator.clipboard.writeText(OLLAMA_BREW_CMD)
    ollamaBrewCopied.value = true
    setTimeout(() => { ollamaBrewCopied.value = false }, 1500)
  } catch (e) {
    console.warn('clipboard write failed', e)
  }
}

async function recheckOllama() {
  llm.value.ollamaChecking = true
  llm.value.ollamaAvailable = await checkOllamaAvailability()
  llm.value.ollamaChecking = false
}

// ----- 重置 / 清空数据 -----
type ResetMode = 'index' | 'all'
type ResetState = 'idle' | 'running' | 'done' | 'error'

const resetConfirm = ref<ResetMode | null>(null)
const resetState = ref<ResetState>('idle')
const resetMessage = ref<string>('')

function openResetConfirm(mode: ResetMode) {
  if (resetState.value === 'running') return
  resetConfirm.value = mode
  resetState.value = 'idle'
  resetMessage.value = ''
}

function closeResetConfirm() {
  if (resetState.value === 'running') return
  resetConfirm.value = null
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`
}

async function performReset() {
  const mode = resetConfirm.value
  if (!mode || resetState.value === 'running') return
  resetState.value = 'running'
  resetMessage.value = ''
  try {
    const result = mode === 'index' ? await systemResetIndex() : await systemResetAll()
    resetState.value = 'done'
    resetMessage.value = t('settings.reset.done', {
      files: result.report.removed_files,
      size: formatBytes(result.report.removed_bytes),
    })
  } catch (e) {
    resetState.value = 'error'
    resetMessage.value = t('settings.reset.failed', {
      err: e instanceof Error ? e.message : String(e),
    })
  }
}
</script>

<template>
  <div class="h-full space-y-3 overflow-y-auto px-3 py-3">
    <!-- Card 1: 常用 — 语言 + Ollama 快捷开关 + 自定义 LLM Providers -->
    <Card>
      <CardHeader>
        <CardTitle class="flex items-center gap-1.5">
          <Sparkles class="h-3 w-3" />
          {{ t('settings.card.general') }}
        </CardTitle>
      </CardHeader>
      <CardContent class="gap-1">
        <!-- 语言 -->
        <div class="flex items-center justify-between py-2">
          <span class="flex items-center gap-2 text-sm">
            <Languages class="h-3.5 w-3.5 text-muted-foreground" />
            {{ t('settings.general.language') }}
          </span>
          <div class="inline-flex rounded-md border border-border p-0.5">
            <button
              v-for="opt in LOCALE_OPTIONS"
              :key="opt.value"
              class="cursor-pointer rounded px-2.5 py-0.5 text-xs font-medium transition-colors"
              :class="locale === opt.value ? 'bg-muted text-foreground' : 'text-muted-foreground hover:text-foreground'"
              @click="changeLocale(opt.value)"
            >{{ opt.label }}</button>
          </div>
        </div>

        <!-- LLM: Ollama -->
        <div class="flex items-center justify-between border-t border-border/40 py-2">
          <span class="flex items-center gap-2 text-sm">
            <Cpu class="h-3.5 w-3.5" :class="llm.ollamaEnabled && llm.ollamaAvailable ? 'text-success' : 'text-muted-foreground'" />
            {{ ollamaLabel }}
          </span>
          <div class="flex items-center gap-2">
            <Button
              v-if="llm.ollamaEnabled"
              variant="ghost"
              size="sm"
              class="h-6 gap-1 px-2 text-xs"
              :disabled="ollamaTest.state === 'testing'"
              @click="runOllamaTest"
            >
              <FlaskConical class="h-3 w-3" :class="ollamaTest.state === 'testing' ? 'animate-pulse' : ''" />
              {{ ollamaTest.state === 'testing' ? t('settings.llm.testing') : t('settings.llm.test') }}
            </Button>
            <Switch
              :model-value="llm.ollamaEnabled"
              @update:model-value="(v: boolean) => setOllama(v)"
            />
          </div>
        </div>
        <!-- Ollama test result -->
        <div
          v-if="ollamaTest.state === 'ok'"
          class="mb-1 flex items-center gap-1.5 rounded-md border border-emerald-500/30 bg-emerald-500/5 px-2 py-1 text-[11px] text-emerald-600 dark:text-emerald-400"
        >
          <CheckCircle2 class="h-3 w-3 shrink-0" />
          <span>{{ t('settings.llm.test_ok', { ms: ollamaTest.latency }) }}</span>
        </div>
        <div
          v-else-if="ollamaTest.state === 'fail'"
          class="mb-1 flex items-start gap-1.5 rounded-md border border-red-500/30 bg-red-500/5 px-2 py-1.5 text-[11px] text-red-600 dark:text-red-400"
        >
          <AlertCircle class="mt-0.5 h-3 w-3 shrink-0" />
          <span class="break-all leading-snug">{{ t('settings.llm.test_fail', { err: ollamaTest.error }) }}</span>
        </div>

        <!-- Ollama 未装引导卡（嵌入在 LLM 行之下） -->
        <div
          v-if="configLoaded && !llm.ollamaAvailable && !llm.ollamaChecking"
          class="my-1 flex flex-col gap-2 rounded-md border border-amber-500/30 bg-amber-500/5 px-2.5 py-2 text-xs"
        >
          <div class="flex items-start gap-1.5 text-amber-700 dark:text-amber-400">
            <AlertCircle class="mt-0.5 h-3 w-3 shrink-0" />
            <span class="flex-1 leading-snug">
              <strong>{{ t('settings.llm.ollama_missing.title') }}</strong>
              <span class="ml-1 text-muted-foreground">{{ t('settings.llm.ollama_missing.intro') }}</span>
            </span>
          </div>

          <div class="flex flex-wrap items-center gap-1.5">
            <Button variant="default" size="sm" class="h-7 shrink-0 gap-1 px-2 text-xs" @click="openOllamaDownload">
              <Download class="h-3 w-3" />
              {{ t('settings.llm.ollama_missing.download') }}
            </Button>
            <span class="text-[11px] text-muted-foreground">{{ t('settings.llm.ollama_missing.or') }}</span>
            <code class="mono flex-1 truncate rounded bg-muted px-1.5 py-0.5 text-[11px]">$ {{ OLLAMA_BREW_CMD }}</code>
            <Button variant="ghost" size="sm" class="h-6 shrink-0 px-1.5 text-[11px]" @click="copyBrewCmd">
              <Copy class="mr-0.5 h-3 w-3" />
              {{ ollamaBrewCopied ? t('common.copied') : t('common.copy') }}
            </Button>
          </div>

          <div class="flex items-center justify-between border-t border-amber-500/20 pt-1.5">
            <p class="text-[11px] text-muted-foreground">{{ t('settings.llm.ollama_missing.after_install_hint') }}</p>
            <Button variant="ghost" size="sm" class="h-6 px-1.5 text-[11px]" :disabled="llm.ollamaChecking" @click="recheckOllama">
              <RefreshCw class="mr-0.5 h-3 w-3" :class="llm.ollamaChecking ? 'animate-spin' : ''" />
              {{ t('settings.llm.ollama_missing.recheck') }}
            </Button>
          </div>

          <div class="border-t border-amber-500/20 pt-1.5">
            <p class="mb-1 text-[11px] font-medium text-amber-700 dark:text-amber-400">{{ t('settings.llm.ollama_missing.impact_title') }}</p>
            <ul class="space-y-0.5 text-[11px] text-muted-foreground">
              <li class="flex items-baseline gap-1.5">
                <span class="text-success">✓</span>
                <span>{{ t('settings.llm.ollama_missing.unaffected') }}</span>
              </li>
              <li class="flex items-baseline gap-1.5">
                <span class="text-destructive">✗</span>
                <span>{{ t('settings.llm.ollama_missing.affected_summaries') }}</span>
              </li>
              <li class="flex items-baseline gap-1.5">
                <span class="text-destructive">✗</span>
                <span>{{ t('settings.llm.ollama_missing.affected_projects') }}</span>
              </li>
              <li class="flex items-baseline gap-1.5">
                <span class="text-destructive">✗</span>
                <span>{{ t('settings.llm.ollama_missing.affected_reports') }}</span>
              </li>
            </ul>
            <p class="mt-1.5 text-[11px] italic text-muted-foreground">
              {{ t('settings.llm.ollama_missing.provider_suggest') }}
            </p>
          </div>
        </div>

        <!-- Custom LLM Providers -->
        <div class="border-t border-border/40 pt-3 mt-2">
          <h3 class="mb-2 flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
            <Plug class="h-3 w-3" />
            {{ t('settings.providers.title') }}
          </h3>
          <LlmProviders />
        </div>
      </CardContent>
    </Card>

    <!-- Card 2: 数据源（折叠） -->
    <Collapsible v-model:open="openDataSources">
      <Card>
        <CollapsibleTrigger
          class="group flex w-full cursor-pointer items-center justify-between gap-2 rounded-lg px-4 py-3 text-left transition-colors hover:bg-muted/30"
        >
          <span class="flex items-center gap-1.5">
            <Database class="h-3 w-3 text-muted-foreground" />
            <span class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              {{ t('settings.card.data_sources') }}
            </span>
            <Badge variant="secondary" class="ml-1 text-[10px]">
              {{ t('settings.card.adapters_summary', { active: activeAdapterCount, total: adapters.length }) }}
            </Badge>
          </span>
          <ChevronDown
            class="h-4 w-4 text-muted-foreground transition-transform duration-200"
            :class="openDataSources ? 'rotate-180' : ''"
          />
        </CollapsibleTrigger>
        <CollapsibleContent class="overflow-hidden data-[state=closed]:animate-collapsible-up data-[state=open]:animate-collapsible-down">
          <div class="px-4 pb-3 pt-1">
            <div
              v-for="(a, i) in adapters"
              :key="a.key"
              class="py-1.5"
              :class="{ 'border-t border-border/40': i > 0 }"
            >
              <div class="flex items-center justify-between gap-2">
                <span class="flex items-center gap-2 text-sm">
                  <IdeIcon :source="a.key" class="h-4 w-4 shrink-0" :class="a.enabled ? '' : 'opacity-40 grayscale'" />
                  {{ a.label }}
                </span>
                <div class="flex items-center gap-1.5">
                  <Button
                    variant="ghost"
                    size="sm"
                    class="h-6 w-6 p-0"
                    :disabled="!a.enabled || a.scanState === 'running'"
                    :title="t('settings.adapters.rescan_one_tip', { name: a.label })"
                    @click="rescanAdapter(a)"
                  >
                    <RefreshCw class="h-3 w-3" :class="a.scanState === 'running' ? 'animate-spin' : ''" />
                  </Button>
                  <Switch
                    :model-value="a.enabled"
                    @update:model-value="(v: boolean) => setAdapter(a.key, v)"
                  />
                </div>
              </div>
              <p
                v-if="a.scanState === 'done'"
                class="ml-4 mt-1 text-[11px] text-success"
              >
                {{ t('settings.adapters.rescan_one_done', { name: a.label, msgs: a.scanMsgs }) }}
              </p>
              <p
                v-else-if="a.scanState === 'empty'"
                class="ml-4 mt-1 text-[11px] text-muted-foreground"
              >
                {{ t('settings.adapters.rescan_one_empty', { name: a.label }) }}
              </p>
              <p
                v-else-if="a.scanState === 'error'"
                class="ml-4 mt-1 text-[11px] text-destructive"
              >
                {{ t('settings.adapters.rescan_one_failed', { name: a.label, err: a.scanError }) }}
              </p>
            </div>
            <div class="mt-3 border-t border-border/40 pt-3">
              <div class="flex items-center justify-between gap-3">
                <p class="text-[11px] text-muted-foreground">{{ t('settings.adapters.rescan_hint') }}</p>
                <Button
                  size="sm"
                  variant="outline"
                  :disabled="rescanState.state === 'running'"
                  class="shrink-0"
                  @click="startRescan"
                >
                  <RefreshCw class="mr-1.5 h-3 w-3" :class="rescanState.state === 'running' ? 'animate-spin' : ''" />
                  {{ rescanState.state === 'running' ? t('settings.adapters.rescanning') : t('settings.adapters.rescan') }}
                </Button>
              </div>
              <p
                v-if="rescanState.state === 'done'"
                class="mt-2 text-[11px] text-success"
              >
                {{ t('settings.adapters.rescan_done', { msgs: rescanState.msgs }) }}
              </p>
              <p
                v-else-if="rescanState.state === 'empty'"
                class="mt-2 text-[11px] text-muted-foreground"
              >
                {{ t('settings.adapters.rescan_empty') }}
              </p>
              <p
                v-else-if="rescanState.state === 'error'"
                class="mt-2 text-[11px] text-destructive"
              >
                {{ t('settings.adapters.rescan_failed', { err: rescanState.error }) }}
              </p>
            </div>
          </div>
        </CollapsibleContent>
      </Card>
    </Collapsible>

    <!-- Card 3: IDE 集成（折叠） -->
    <Collapsible v-model:open="openIde">
      <Card>
        <CollapsibleTrigger
          class="group flex w-full cursor-pointer items-center justify-between gap-2 rounded-lg px-4 py-3 text-left transition-colors hover:bg-muted/30"
        >
          <span class="flex items-center gap-1.5">
            <Plug class="h-3 w-3 text-muted-foreground" />
            <span class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              {{ t('settings.card.ide') }}
            </span>
            <Badge variant="secondary" class="ml-1 text-[10px]">
              {{ t('settings.card.ide_summary', { installed: installedIdeCount, total: ideRows.length }) }}
            </Badge>
          </span>
          <ChevronDown
            class="h-4 w-4 text-muted-foreground transition-transform duration-200"
            :class="openIde ? 'rotate-180' : ''"
          />
        </CollapsibleTrigger>
        <CollapsibleContent class="overflow-hidden data-[state=closed]:animate-collapsible-up data-[state=open]:animate-collapsible-down">
          <div class="px-4 pb-3 pt-1">
            <p class="mb-2 text-[11px] text-muted-foreground">{{ t('settings.integrations.hint') }}</p>
            <div
              v-for="(row, i) in ideRows"
              :key="row.ide"
              class="flex items-center justify-between gap-2 py-2"
              :class="{ 'border-t border-border/40': i > 0 }"
            >
              <span class="flex flex-1 items-center gap-2 text-sm min-w-0">
                <IdeIcon
                  :source="normalizeIdeKey(row.ide)"
                  class="h-5 w-5 shrink-0"
                  :class="(row.mcpStatus?.installed || row.skillStatus?.installed) ? '' : 'opacity-40 grayscale'"
                />
                <span class="flex min-w-0 flex-col">
                  <span class="truncate">{{ row.label }}</span>
                  <span v-if="row.mcpError || row.skillError || row.hookError" class="truncate text-[10px] text-destructive">
                    {{ t('settings.integrations.error_prefix') }}{{ row.mcpError || row.skillError || row.hookError }}
                  </span>
                  <span v-else-if="!row.mcpStatus?.config_exists" class="text-[10px] text-muted-foreground">
                    {{ t('settings.integrations.status.no_config') }}
                  </span>
                </span>
              </span>

              <div class="flex items-center gap-1">
                <span class="text-[10px] font-medium text-muted-foreground" :class="{ 'opacity-50': row.mcpLoading }">
                  {{ t('settings.integrations.mcp_label') }}
                </span>
                <Switch
                  :model-value="row.mcpStatus?.installed ?? false"
                  :disabled="row.mcpLoading"
                  @update:model-value="(v: boolean) => toggleMcp(row, v)"
                />
              </div>

              <div class="flex items-center gap-1">
                <span class="text-[10px] font-medium text-muted-foreground" :class="{ 'opacity-50': row.skillLoading }">
                  {{ t('settings.integrations.skill_label') }}
                </span>
                <Switch
                  :model-value="row.skillStatus?.installed ?? false"
                  :disabled="row.skillLoading"
                  @update:model-value="(v: boolean) => toggleSkill(row, v)"
                />
              </div>

              <div class="flex items-center gap-1" :title="row.hookStatus && !row.hookStatus.supported ? t('settings.integrations.hook_unsupported_tip') : t('settings.integrations.hook_tip')">
                <span class="text-[10px] font-medium text-muted-foreground" :class="{ 'opacity-50': row.hookLoading || (row.hookStatus && !row.hookStatus.supported) }">
                  {{ t('settings.integrations.hook_label') }}
                </span>
                <Switch
                  :model-value="row.hookStatus?.installed ?? false"
                  :disabled="row.hookLoading || (row.hookStatus !== null && !row.hookStatus.supported)"
                  @update:model-value="(v: boolean) => toggleHook(row, v)"
                />
              </div>
            </div>
            <p class="mt-2 text-[10px] italic text-muted-foreground">{{ t('settings.integrations.restart_hint') }}</p>
          </div>
        </CollapsibleContent>
      </Card>
    </Collapsible>

    <!-- Card 4: 系统（折叠） — Privacy + CLI + About -->
    <Collapsible v-model:open="openSystem">
      <Card>
        <CollapsibleTrigger
          class="group flex w-full cursor-pointer items-center justify-between gap-2 rounded-lg px-4 py-3 text-left transition-colors hover:bg-muted/30"
        >
          <span class="flex items-center gap-1.5">
            <Shield class="h-3 w-3 text-muted-foreground" />
            <span class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              {{ t('settings.card.system') }}
            </span>
          </span>
          <ChevronDown
            class="h-4 w-4 text-muted-foreground transition-transform duration-200"
            :class="openSystem ? 'rotate-180' : ''"
          />
        </CollapsibleTrigger>
        <CollapsibleContent class="overflow-hidden data-[state=closed]:animate-collapsible-up data-[state=open]:animate-collapsible-down">
          <div class="flex flex-col gap-3 px-4 pb-3 pt-1">

            <!-- 隐私 -->
            <div>
              <p class="mb-1 flex items-center gap-1.5 text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
                <Shield class="h-3 w-3" />
                {{ t('settings.section.privacy') }}
              </p>
              <div class="flex items-center justify-between py-1.5">
                <span class="text-sm">{{ t('settings.privacy.auto_redact') }}</span>
                <Switch
                  :model-value="privacy.autoRedact"
                  @update:model-value="(v: boolean) => setPrivacy('autoRedact', v)"
                />
              </div>
              <div class="flex items-center justify-between border-t border-border/40 py-1.5">
                <span class="text-sm">{{ t('settings.privacy.hide_private') }}</span>
                <Switch
                  :model-value="privacy.privateFromMcp"
                  @update:model-value="(v: boolean) => setPrivacy('privateFromMcp', v)"
                />
              </div>
            </div>

            <Separator />

            <!-- CLI -->
            <div>
              <p class="mb-1 flex items-center gap-1.5 text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
                <Terminal class="h-3 w-3" />
                {{ t('settings.section.cli') }}
              </p>
              <div class="flex items-center justify-between gap-2 py-1.5">
                <span class="flex min-w-0 flex-1 flex-col">
                  <span class="text-sm">{{ t('settings.cli.title') }}</span>
                  <span v-if="cli" class="mono truncate text-[10px] text-muted-foreground">
                    {{ cli.target_dir }}/memex
                  </span>
                </span>
                <Button
                  v-if="!cli?.installed"
                  variant="default"
                  size="sm"
                  class="h-7 shrink-0 text-xs"
                  :disabled="cliLoading"
                  @click="installCli"
                >
                  <RefreshCw v-if="cliLoading" class="mr-1 h-3 w-3 animate-spin" />
                  {{ cliLoading ? t('settings.cli.installing') : t('settings.cli.install') }}
                </Button>
                <Button
                  v-else
                  variant="outline"
                  size="sm"
                  class="h-7 shrink-0 text-xs"
                  :disabled="cliLoading"
                  @click="uninstallCli"
                >
                  <RefreshCw v-if="cliLoading" class="mr-1 h-3 w-3 animate-spin" />
                  {{ cliLoading ? t('settings.cli.uninstalling') : t('settings.cli.uninstall') }}
                </Button>
              </div>

              <div
                v-if="cli?.installed && cli.path_contains_target_dir"
                class="mt-1 flex items-center gap-1.5 rounded-md border border-emerald-500/30 bg-emerald-500/5 px-2 py-1.5 text-[11px] text-emerald-600 dark:text-emerald-400"
              >
                <CheckCircle2 class="h-3 w-3 shrink-0" />
                <span>{{ t('settings.cli.ready') }}</span>
              </div>

              <div
                v-else-if="cli?.installed && !cli.path_contains_target_dir"
                class="mt-1 flex flex-col gap-1.5 rounded-md border border-amber-500/30 bg-amber-500/5 px-2 py-1.5 text-[11px] text-amber-700 dark:text-amber-400"
              >
                <span class="flex items-start gap-1.5">
                  <AlertCircle class="mt-0.5 h-3 w-3 shrink-0" />
                  <span>{{ t('settings.cli.path_missing') }}</span>
                </span>
                <div class="flex items-center gap-1.5">
                  <code class="mono flex-1 truncate rounded bg-amber-500/10 px-1.5 py-0.5 text-[10px]">{{ cli.path_export_hint }}</code>
                  <Button variant="ghost" size="sm" class="h-6 shrink-0 px-1.5 text-[10px]" @click="copyExportHint">
                    <Copy class="mr-0.5 h-3 w-3" />
                    {{ cliCopied ? t('common.copied') : t('common.copy') }}
                  </Button>
                </div>
              </div>

              <div
                v-else-if="cliError"
                class="mt-1 flex items-center gap-1.5 rounded-md border border-red-500/30 bg-red-500/5 px-2 py-1.5 text-[11px] text-red-600 dark:text-red-400"
              >
                <AlertCircle class="h-3 w-3 shrink-0" />
                <span class="truncate">{{ cliError }}</span>
              </div>
            </div>

            <Separator />

            <!-- Diagnostics (Doctor) -->
            <div>
              <p class="mb-1 flex items-center gap-1.5 text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
                <Stethoscope class="h-3 w-3" />
                {{ t('settings.section.doctor') }}
              </p>
              <div class="flex items-center justify-between gap-2 py-1.5">
                <span class="flex min-w-0 flex-1 flex-col">
                  <span class="text-sm">{{ t('settings.doctor.hint') }}</span>
                </span>
                <Button
                  variant="outline"
                  size="sm"
                  class="h-7 shrink-0 gap-1 text-xs"
                  :disabled="doctorRunning"
                  @click="startDoctor"
                >
                  <RefreshCw class="h-3 w-3" :class="doctorRunning ? 'animate-spin' : ''" />
                  {{ doctorRunning ? t('settings.doctor.running') : t('settings.doctor.run') }}
                </Button>
              </div>

              <div
                v-if="doctorError"
                class="mt-1 flex items-center gap-1.5 rounded-md border border-red-500/30 bg-red-500/5 px-2 py-1.5 text-[11px] text-red-600 dark:text-red-400"
              >
                <AlertCircle class="h-3 w-3 shrink-0" />
                <span class="truncate">{{ t('settings.doctor.failed', { err: doctorError }) }}</span>
              </div>

              <div
                v-else-if="doctorResult"
                class="mt-2 flex flex-col gap-1.5 rounded-md border border-border bg-muted/30 px-3 py-2 text-[11px]"
              >
                <div class="flex items-center justify-between gap-2">
                  <span class="text-muted-foreground">{{ t('settings.doctor.data_dir') }}</span>
                  <code class="mono truncate text-[10px]">{{ doctorResult.data_dir }}</code>
                </div>
                <div class="flex items-center justify-between gap-2">
                  <span class="text-muted-foreground">{{ t('settings.doctor.db') }}</span>
                  <span :class="doctorResult.report.db_exists ? 'text-success' : 'text-amber-600 dark:text-amber-400'">
                    <template v-if="doctorResult.report.db_exists">
                      v{{ doctorResult.report.schema_version ?? '?' }}
                    </template>
                    <template v-else>{{ t('settings.doctor.db_missing') }}</template>
                  </span>
                </div>
                <div v-if="doctorResult.report.db_exists" class="flex items-center justify-between gap-2">
                  <span class="text-muted-foreground">{{ t('settings.doctor.fts') }}</span>
                  <span :class="doctorResult.report.fts_ok ? 'text-success' : 'text-destructive'">
                    {{ doctorResult.report.fts_ok ? t('settings.doctor.fts_ok') : t('settings.doctor.fts_error') }}
                  </span>
                </div>
                <div v-if="doctorResult.report.db_exists" class="flex items-center justify-between gap-2">
                  <span class="text-muted-foreground">{{ t('settings.doctor.counts') }}</span>
                  <span class="mono text-[10px]">
                    {{ t('settings.doctor.counts_value', {
                      sessions: doctorResult.report.session_count,
                      messages: doctorResult.report.message_count,
                      chunks: doctorResult.report.chunk_count,
                    }) }}
                  </span>
                </div>

                <div v-if="doctorResult.report.adapters.length > 0" class="mt-1 border-t border-border/40 pt-1.5">
                  <p class="mb-1 text-[10px] uppercase tracking-wider text-muted-foreground">{{ t('settings.doctor.adapters_title') }}</p>
                  <div
                    v-for="a in doctorResult.report.adapters"
                    :key="a.name"
                    class="flex items-center justify-between gap-2 py-0.5 text-[10px] text-muted-foreground"
                  >
                    <span>{{ t('settings.doctor.adapter_line', {
                      name: a.name,
                      files: a.file_count,
                      scan: a.last_scan ?? t('settings.doctor.adapter_never'),
                    }) }}</span>
                  </div>
                </div>

                <div class="mt-1 border-t border-border/40 pt-1.5">
                  <p class="mb-1 text-[10px] uppercase tracking-wider text-muted-foreground">{{ t('settings.doctor.cursor_title') }}</p>
                  <p
                    class="text-[11px]"
                    :class="{
                      'text-success': doctorCursorTone(doctorResult.cursor_probe) === 'ok',
                      'text-destructive': doctorCursorTone(doctorResult.cursor_probe) === 'error',
                      'text-muted-foreground': doctorCursorTone(doctorResult.cursor_probe) === 'muted',
                    }"
                  >
                    {{ doctorCursorMessage(doctorResult.cursor_probe) }}
                  </p>
                </div>
              </div>
            </div>

            <Separator />

            <!-- Reset / 清空数据 -->
            <div>
              <p class="mb-1 flex items-center gap-1.5 text-[11px] font-medium uppercase tracking-wider text-destructive">
                <TriangleAlert class="h-3 w-3" />
                {{ t('settings.section.reset') }}
              </p>
              <p class="mb-2 text-[11px] text-muted-foreground">{{ t('settings.reset.intro') }}</p>

              <div class="flex items-center justify-between gap-2 py-1.5">
                <span class="flex min-w-0 flex-1 flex-col">
                  <span class="text-sm">{{ t('settings.reset.index.title') }}</span>
                  <span class="text-[10px] leading-snug text-muted-foreground">{{ t('settings.reset.index.desc') }}</span>
                </span>
                <Button
                  variant="outline"
                  size="sm"
                  class="h-7 shrink-0 gap-1 border-amber-500/50 text-xs text-amber-700 hover:bg-amber-500/10 dark:text-amber-400"
                  :disabled="resetState === 'running'"
                  @click="openResetConfirm('index')"
                >
                  <RefreshCw class="h-3 w-3" />
                  {{ t('settings.reset.index.button') }}
                </Button>
              </div>

              <div class="flex items-center justify-between gap-2 border-t border-border/40 py-1.5">
                <span class="flex min-w-0 flex-1 flex-col">
                  <span class="text-sm text-destructive">{{ t('settings.reset.all.title') }}</span>
                  <span class="text-[10px] leading-snug text-muted-foreground">{{ t('settings.reset.all.desc') }}</span>
                </span>
                <Button
                  variant="outline"
                  size="sm"
                  class="h-7 shrink-0 gap-1 border-destructive/60 text-xs text-destructive hover:bg-destructive/10"
                  :disabled="resetState === 'running'"
                  @click="openResetConfirm('all')"
                >
                  <Trash2 class="h-3 w-3" />
                  {{ t('settings.reset.all.button') }}
                </Button>
              </div>
            </div>

            <Separator />

            <!-- 关于 -->
            <div>
              <p class="mb-1 flex items-center gap-1.5 text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
                <Info class="h-3 w-3" />
                {{ t('settings.section.about') }}
              </p>
              <div class="flex items-center justify-between gap-2 py-1.5">
                <span class="flex min-w-0 flex-col">
                  <span class="text-sm">{{ t('settings.about.version') }}</span>
                  <span class="text-[10px] text-muted-foreground">{{ t('settings.about.upgrade_hint') }}</span>
                </span>
                <div class="flex items-center gap-2 shrink-0">
                  <span class="mono text-[11px] text-muted-foreground">v{{ APP_VERSION }}</span>
                  <Button
                    variant="outline"
                    size="sm"
                    class="h-7 gap-1 text-xs"
                    :disabled="updateStatus === 'checking'"
                    @click="checkForUpdates"
                  >
                    <RefreshCw class="h-3 w-3" :class="updateStatus === 'checking' ? 'animate-spin' : ''" />
                    {{ updateStatus === 'checking' ? t('settings.about.checking') : t('settings.about.check_update') }}
                  </Button>
                </div>
              </div>

              <div
                v-if="updateStatus === 'latest'"
                class="mt-1 flex items-center gap-1.5 rounded-md border border-emerald-500/30 bg-emerald-500/5 px-2 py-1.5 text-[11px] text-emerald-600 dark:text-emerald-400"
              >
                <CheckCircle2 class="h-3 w-3 shrink-0" />
                <span>{{ t('settings.about.up_to_date') }}</span>
              </div>

              <div
                v-else-if="updateStatus === 'outdated'"
                class="mt-1 flex items-center justify-between gap-1.5 rounded-md border border-amber-500/30 bg-amber-500/5 px-2 py-1.5 text-[11px]"
              >
                <span class="flex min-w-0 items-center gap-1.5 text-amber-700 dark:text-amber-400">
                  <Download class="h-3 w-3 shrink-0" />
                  <span class="truncate">{{ t('settings.about.new_version_available', { v: remoteVersion }) }}</span>
                </span>
                <Button variant="link" size="sm" class="h-auto shrink-0 p-0 text-[11px] text-amber-700 dark:text-amber-400" @click="openReleasePage">
                  {{ t('settings.about.view_release') }}
                </Button>
              </div>

              <div
                v-else-if="updateStatus === 'error'"
                class="mt-1 flex items-center justify-between gap-1.5 rounded-md border border-red-500/30 bg-red-500/5 px-2 py-1.5 text-[11px] text-red-600 dark:text-red-400"
              >
                <span class="flex min-w-0 items-center gap-1.5">
                  <AlertCircle class="h-3 w-3 shrink-0" />
                  <span class="truncate">{{ t('settings.about.check_failed', { err: errorMessage }) }}</span>
                </span>
                <Button variant="link" size="sm" class="h-auto shrink-0 p-0 text-[11px] text-red-600 dark:text-red-400" @click="openReleasePage">
                  {{ t('settings.about.open_in_browser') }}
                </Button>
              </div>
            </div>

          </div>
        </CollapsibleContent>
      </Card>
    </Collapsible>

    <!-- 重置确认弹框 -->
    <Teleport to="body">
      <div
        v-if="resetConfirm !== null"
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm"
        @click.self="closeResetConfirm"
      >
        <div class="w-[420px] max-w-[92vw] rounded-lg border border-border bg-background p-4 shadow-xl">
          <div class="mb-3 flex items-start gap-2">
            <TriangleAlert
              class="mt-0.5 h-5 w-5 shrink-0"
              :class="resetConfirm === 'all' ? 'text-destructive' : 'text-amber-500'"
            />
            <div class="min-w-0 flex-1">
              <p class="text-sm font-semibold">{{ t('settings.reset.confirm.title') }}</p>
              <p class="mt-0.5 text-xs text-muted-foreground">
                {{ resetConfirm === 'all'
                  ? t('settings.reset.confirm.subtitle_all')
                  : t('settings.reset.confirm.subtitle_index') }}
              </p>
            </div>
          </div>

          <div class="space-y-2 rounded-md bg-muted/50 px-3 py-2 text-[11px]">
            <div>
              <span class="font-medium text-destructive">{{ t('settings.reset.confirm.removed_label') }}：</span>
              <span class="ml-1">{{ resetConfirm === 'all'
                ? t('settings.reset.confirm.removed_all')
                : t('settings.reset.confirm.removed_index') }}</span>
            </div>
            <div>
              <span class="font-medium text-success">{{ t('settings.reset.confirm.kept_label') }}：</span>
              <span class="ml-1">{{ resetConfirm === 'all'
                ? t('settings.reset.confirm.kept_all')
                : t('settings.reset.confirm.kept_index') }}</span>
            </div>
            <div class="border-t border-border/60 pt-1.5">
              <span class="font-medium text-amber-600 dark:text-amber-400">{{ t('settings.reset.confirm.side_effect_title') }}：</span>
              <span class="ml-1 text-muted-foreground">{{ resetConfirm === 'all'
                ? t('settings.reset.confirm.side_effect_all')
                : t('settings.reset.confirm.side_effect_index') }}</span>
            </div>
          </div>

          <p class="mt-2 flex items-start gap-1.5 text-[11px] italic text-muted-foreground">
            <Info class="mt-0.5 h-3 w-3 shrink-0" />
            <span>{{ t('settings.reset.confirm.exit_hint') }}</span>
          </p>

          <div
            v-if="resetState === 'error' && resetMessage"
            class="mt-2 flex items-start gap-1.5 rounded-md border border-red-500/30 bg-red-500/5 px-2 py-1.5 text-[11px] text-red-600 dark:text-red-400"
          >
            <AlertCircle class="mt-0.5 h-3 w-3 shrink-0" />
            <span class="break-all leading-snug">{{ resetMessage }}</span>
          </div>

          <div
            v-else-if="resetState === 'done' && resetMessage"
            class="mt-2 flex items-start gap-1.5 rounded-md border border-emerald-500/30 bg-emerald-500/5 px-2 py-1.5 text-[11px] text-emerald-600 dark:text-emerald-400"
          >
            <CheckCircle2 class="mt-0.5 h-3 w-3 shrink-0" />
            <span class="leading-snug">{{ resetMessage }}</span>
          </div>

          <div class="mt-4 flex items-center justify-end gap-2">
            <Button
              variant="ghost"
              size="sm"
              class="h-8 text-xs"
              :disabled="resetState === 'running' || resetState === 'done'"
              @click="closeResetConfirm"
            >
              {{ t('settings.reset.confirm.cancel') }}
            </Button>
            <Button
              size="sm"
              class="h-8 gap-1 text-xs"
              :class="resetConfirm === 'all'
                ? 'bg-destructive text-destructive-foreground hover:bg-destructive/90'
                : 'bg-amber-600 text-white hover:bg-amber-600/90'"
              :disabled="resetState === 'running' || resetState === 'done'"
              @click="performReset"
            >
              <RefreshCw v-if="resetState === 'running'" class="h-3 w-3 animate-spin" />
              <Trash2 v-else class="h-3 w-3" />
              {{ resetState === 'running'
                ? t('settings.reset.running')
                : t('settings.reset.confirm.proceed') }}
            </Button>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>
