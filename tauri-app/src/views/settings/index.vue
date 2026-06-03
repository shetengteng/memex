<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useMemex } from '@/composables/useMemex'
import { Switch } from '@/components/ui/switch'
import { Separator } from '@/components/ui/separator'
import { Button } from '@/components/ui/button'
import { RefreshCw, CheckCircle2, AlertCircle, Download, Terminal, Copy } from 'lucide-vue-next'
import { openUrl } from '@tauri-apps/plugin-opener'
import { useI18n, setLocale, LOCALE_OPTIONS, type Locale } from '@/i18n'
import type { CliStatus } from '@/types'

const { t, locale } = useI18n()
const { toggleAdapter: ipcToggleAdapter, getConfig, setConfig, cliStatus: ipcCliStatus, cliInstall: ipcCliInstall, cliUninstall: ipcCliUninstall } = useMemex()
const APP_VERSION = __APP_VERSION__
const RELEASES_LATEST_PAGE = 'https://github.com/shetengteng/memex/releases/latest'

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

interface AdapterRow { key: string; label: string; enabled: boolean }

const adapters = ref<AdapterRow[]>([
  { key: 'claude_code', label: 'Claude Code', enabled: true },
  { key: 'cursor', label: 'Cursor', enabled: true },
  { key: 'codex', label: 'Codex', enabled: true },
  { key: 'opencode', label: 'OpenCode', enabled: true },
  { key: 'aider', label: 'Aider', enabled: true },
  { key: 'continue_dev', label: 'Continue', enabled: true },
  { key: 'cline', label: 'Cline', enabled: true },
])

const privacy = ref({ autoRedact: false, privateFromMcp: false })
const configLoaded = ref(false)
const llm = ref({
  ollamaEnabled: false,
  ollamaModel: 'qwen2.5:7b',
  ollamaAvailable: false,
  ollamaChecking: false,
  claudeFallback: false,
})

const ollamaLabel = computed(() =>
  t('settings.llm.ollama_label', { model: llm.value.ollamaModel }),
)

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
    const v = await getConfig('llm.cloud_fallback')
    if (v !== null) llm.value.claudeFallback = v === 'true'
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

async function setOllama(value: boolean) {
  if (llm.value.ollamaEnabled === value) return
  if (value) {
    llm.value.ollamaChecking = true
    const available = await checkOllamaAvailability()
    llm.value.ollamaAvailable = available
    llm.value.ollamaChecking = false
    if (!available) return
  }
  const prev = llm.value.ollamaEnabled
  llm.value.ollamaEnabled = value
  try {
    await setConfig('llm.ollama_enabled', String(value))
  } catch {
    llm.value.ollamaEnabled = prev
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

async function setCloudFallback(value: boolean) {
  if (llm.value.claudeFallback === value) return
  const prev = llm.value.claudeFallback
  llm.value.claudeFallback = value
  try {
    await setConfig('llm.cloud_fallback', String(value))
  } catch {
    llm.value.claudeFallback = prev
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

interface IdeRow {
  ide: string
  label: string
  mcpStatus: IdeStatus | null
  skillStatus: SkillStatus | null
  mcpLoading: boolean
  skillLoading: boolean
  mcpError: string
  skillError: string
}

const ideRows = ref<IdeRow[]>([
  { ide: 'cursor', label: 'Cursor', mcpStatus: null, skillStatus: null, mcpLoading: false, skillLoading: false, mcpError: '', skillError: '' },
  { ide: 'claude-code', label: 'Claude Code', mcpStatus: null, skillStatus: null, mcpLoading: false, skillLoading: false, mcpError: '', skillError: '' },
  { ide: 'codex', label: 'Codex', mcpStatus: null, skillStatus: null, mcpLoading: false, skillLoading: false, mcpError: '', skillError: '' },
  { ide: 'opencode', label: 'OpenCode', mcpStatus: null, skillStatus: null, mcpLoading: false, skillLoading: false, mcpError: '', skillError: '' },
])

async function refreshIdeStatuses() {
  try {
    const [mcps, skills] = await Promise.all([
      invoke<IdeStatus[]>('ide_list_status'),
      invoke<SkillStatus[]>('skill_list_status'),
    ])
    for (const s of mcps) {
      const row = ideRows.value.find((r) => r.ide === s.ide)
      if (row) row.mcpStatus = s
    }
    for (const s of skills) {
      const row = ideRows.value.find((r) => r.ide === s.ide)
      if (row) row.skillStatus = s
    }
  } catch (e) {
    console.warn('ide/skill list_status failed', e)
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
</script>

<template>
  <div class="h-full space-y-3 overflow-y-auto px-4 py-4">
    <!-- 通用 -->
    <p class="text-sm font-semibold uppercase tracking-wider text-muted-foreground">{{ t('settings.section.general') }}</p>
    <div class="flex items-center justify-between py-2.5">
      <span class="flex flex-col gap-0.5">
        <span class="text-base">{{ t('settings.general.language') }}</span>
        <span class="text-xs text-muted-foreground">{{ t('settings.general.language_hint') }}</span>
      </span>
      <div class="inline-flex rounded-md border border-border p-0.5">
        <button
          v-for="opt in LOCALE_OPTIONS"
          :key="opt.value"
          class="rounded px-3 py-1 text-sm font-medium transition-colors"
          :class="locale === opt.value ? 'bg-muted text-foreground' : 'text-muted-foreground hover:text-foreground'"
          @click="changeLocale(opt.value)"
        >{{ opt.label }}</button>
      </div>
    </div>

    <Separator class="my-2" />

    <!-- 适配器 -->
    <p class="text-sm font-semibold uppercase tracking-wider text-muted-foreground">{{ t('settings.section.adapters') }}</p>
    <div
      v-for="(a, i) in adapters"
      :key="a.key"
      class="flex items-center justify-between py-2.5"
      :class="{ 'border-t border-border/40': i > 0 }"
    >
      <span class="flex items-center gap-2.5 text-base">
        <span class="inline-block h-2.5 w-2.5 rounded-full" :class="a.enabled ? 'bg-success' : 'bg-muted-foreground'" />
        {{ a.label }}
      </span>
      <Switch
        :model-value="a.enabled"
        class="h-7 w-12 [&_>span]:h-6 [&_>span]:w-6 [&[data-state=checked]_>span]:translate-x-5"
        @update:model-value="(v: boolean) => setAdapter(a.key, v)"
      />
    </div>

    <Separator class="my-2" />

    <!-- LLM -->
    <p class="text-sm font-semibold uppercase tracking-wider text-muted-foreground">{{ t('settings.section.llm') }}</p>
    <div class="flex items-center justify-between py-2.5">
      <span class="flex items-center gap-2.5 text-base">
        <span
          class="inline-block h-2.5 w-2.5 rounded-full"
          :class="llm.ollamaEnabled && llm.ollamaAvailable ? 'bg-success' : llm.ollamaChecking ? 'bg-warning animate-pulse' : 'bg-muted-foreground'"
        />
        {{ ollamaLabel }}
      </span>
      <div class="flex items-center gap-2.5">
        <span
          class="text-sm"
          :class="llm.ollamaAvailable ? 'text-success' : 'text-destructive'"
        >
          {{ llm.ollamaChecking ? '…' : llm.ollamaAvailable ? t('settings.adapters.local') : t('settings.adapters.offline') }}
        </span>
        <Switch
          :model-value="llm.ollamaEnabled"
          class="h-7 w-12 [&_>span]:h-6 [&_>span]:w-6 [&[data-state=checked]_>span]:translate-x-5"
          @update:model-value="(v: boolean) => setOllama(v)"
        />
      </div>
    </div>

    <!-- Ollama 未装引导：本地 LLM 离线时给安装路径 + 功能影响清单 -->
    <div
      v-if="configLoaded && !llm.ollamaAvailable && !llm.ollamaChecking"
      class="flex flex-col gap-3 rounded-md border border-amber-500/30 bg-amber-500/5 px-3 py-3 text-xs"
    >
      <div class="flex items-start gap-2 text-amber-700 dark:text-amber-400">
        <AlertCircle class="mt-0.5 h-3.5 w-3.5 shrink-0" />
        <span class="flex-1">
          <strong>{{ t('settings.llm.ollama_missing.title') }}</strong>
          <span class="ml-1">{{ t('settings.llm.ollama_missing.intro') }}</span>
        </span>
      </div>

      <!-- 安装方案：官网下载 + brew 命令 -->
      <div class="flex flex-col gap-2 sm:flex-row sm:items-center">
        <Button variant="default" size="sm" class="shrink-0 gap-1.5" @click="openOllamaDownload">
          <Download class="h-3.5 w-3.5" />
          {{ t('settings.llm.ollama_missing.download') }}
        </Button>
        <span class="text-xs text-muted-foreground">{{ t('settings.llm.ollama_missing.or') }}</span>
        <div class="flex flex-1 items-center gap-1.5">
          <code class="mono flex-1 truncate rounded bg-muted px-2 py-1 text-[11px]">$ {{ OLLAMA_BREW_CMD }}</code>
          <Button variant="ghost" size="sm" class="h-7 shrink-0 px-2" @click="copyBrewCmd">
            <Copy class="mr-1 h-3 w-3" />
            {{ ollamaBrewCopied ? t('common.copied') : t('common.copy') }}
          </Button>
        </div>
      </div>

      <!-- 装完后给用户一个一键复查的按钮，省得让他切来切去 -->
      <div class="flex items-center justify-between border-t border-amber-500/20 pt-2">
        <p class="text-xs text-muted-foreground">{{ t('settings.llm.ollama_missing.after_install_hint') }}</p>
        <Button variant="ghost" size="sm" class="h-7 px-2" :disabled="llm.ollamaChecking" @click="recheckOllama">
          <RefreshCw class="mr-1 h-3 w-3" :class="llm.ollamaChecking ? 'animate-spin' : ''" />
          {{ t('settings.llm.ollama_missing.recheck') }}
        </Button>
      </div>

      <!-- 功能影响清单 -->
      <div class="border-t border-amber-500/20 pt-2">
        <p class="mb-1.5 font-medium text-amber-700 dark:text-amber-400">{{ t('settings.llm.ollama_missing.impact_title') }}</p>
        <ul class="space-y-1 text-muted-foreground">
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
        <p v-if="llm.claudeFallback" class="mt-2 text-emerald-600 dark:text-emerald-400">
          {{ t('settings.llm.ollama_missing.claude_fallback_hint') }}
        </p>
        <p v-else class="mt-2 text-muted-foreground italic">
          {{ t('settings.llm.ollama_missing.claude_fallback_suggest') }}
        </p>
      </div>
    </div>

    <div class="flex items-center justify-between border-t border-border/40 py-2.5">
      <span class="flex items-center gap-2.5 text-base">
        <span class="inline-block h-2.5 w-2.5 rounded-full" :class="llm.claudeFallback ? 'bg-success' : 'bg-muted-foreground'" />
        {{ t('settings.llm.claude_fallback') }}
      </span>
      <Switch
        :model-value="llm.claudeFallback"
        class="h-7 w-12 [&_>span]:h-6 [&_>span]:w-6 [&[data-state=checked]_>span]:translate-x-5"
        @update:model-value="(v: boolean) => setCloudFallback(v)"
      />
    </div>

    <Separator class="my-2" />

    <!-- 隐私 -->
    <p class="text-sm font-semibold uppercase tracking-wider text-muted-foreground">{{ t('settings.section.privacy') }}</p>
    <div class="flex items-center justify-between py-2.5">
      <span class="text-base">{{ t('settings.privacy.auto_redact') }}</span>
      <Switch
        :model-value="privacy.autoRedact"
        class="h-7 w-12 [&_>span]:h-6 [&_>span]:w-6 [&[data-state=checked]_>span]:translate-x-5"
        @update:model-value="(v: boolean) => setPrivacy('autoRedact', v)"
      />
    </div>
    <div class="flex items-center justify-between border-t border-border/40 py-2.5">
      <span class="text-base">{{ t('settings.privacy.hide_private') }}</span>
      <Switch
        :model-value="privacy.privateFromMcp"
        class="h-7 w-12 [&_>span]:h-6 [&_>span]:w-6 [&[data-state=checked]_>span]:translate-x-5"
        @update:model-value="(v: boolean) => setPrivacy('privateFromMcp', v)"
      />
    </div>

    <Separator class="my-2" />

    <!-- IDE 集成（一键安装/卸载 MCP 到目标 IDE 配置） -->
    <div class="flex items-baseline justify-between">
      <p class="text-sm font-semibold uppercase tracking-wider text-muted-foreground">{{ t('settings.section.integrations') }}</p>
      <span class="text-xs text-muted-foreground">{{ t('settings.integrations.hint') }}</span>
    </div>
    <div
      v-for="(row, i) in ideRows"
      :key="row.ide"
      class="flex items-center justify-between gap-3 py-2.5"
      :class="{ 'border-t border-border/40': i > 0 }"
    >
      <span class="flex flex-1 items-center gap-2.5 text-base">
        <span
          class="inline-block h-2.5 w-2.5 rounded-full"
          :class="(row.mcpStatus?.installed && row.skillStatus?.installed) ? 'bg-success'
            : (row.mcpStatus?.installed || row.skillStatus?.installed) ? 'bg-warning'
            : 'bg-muted-foreground'"
        />
        <span class="flex flex-col">
          <span>{{ row.label }}</span>
          <span class="text-xs text-muted-foreground">
            <template v-if="row.mcpError || row.skillError">
              <span class="text-destructive">{{ t('settings.integrations.error_prefix') }}{{ row.mcpError || row.skillError }}</span>
            </template>
            <template v-else-if="!row.mcpStatus?.config_exists">
              {{ t('settings.integrations.status.no_config') }}
            </template>
          </span>
        </span>
      </span>

      <!-- MCP switch -->
      <div class="flex items-center gap-1.5">
        <span class="text-xs font-medium text-muted-foreground" :class="{ 'opacity-50': row.mcpLoading }">
          {{ t('settings.integrations.mcp_label') }}
        </span>
        <Switch
          :model-value="row.mcpStatus?.installed ?? false"
          :disabled="row.mcpLoading"
          class="h-6 w-10 [&_>span]:h-5 [&_>span]:w-5 [&[data-state=checked]_>span]:translate-x-4"
          @update:model-value="(v: boolean) => toggleMcp(row, v)"
        />
      </div>

      <!-- SKILL switch -->
      <div class="flex items-center gap-1.5">
        <span class="text-xs font-medium text-muted-foreground" :class="{ 'opacity-50': row.skillLoading }">
          {{ t('settings.integrations.skill_label') }}
        </span>
        <Switch
          :model-value="row.skillStatus?.installed ?? false"
          :disabled="row.skillLoading"
          class="h-6 w-10 [&_>span]:h-5 [&_>span]:w-5 [&[data-state=checked]_>span]:translate-x-4"
          @update:model-value="(v: boolean) => toggleSkill(row, v)"
        />
      </div>
    </div>
    <p class="text-xs text-muted-foreground italic">{{ t('settings.integrations.restart_hint') }}</p>

    <Separator class="my-2" />

    <!-- CLI: 仿 VS Code "Install 'code' command in PATH" -->
    <div class="flex items-baseline justify-between">
      <p class="text-sm font-semibold uppercase tracking-wider text-muted-foreground">{{ t('settings.section.cli') }}</p>
      <span class="text-xs text-muted-foreground">{{ t('settings.cli.hint') }}</span>
    </div>
    <div class="flex flex-col gap-2 py-2.5">
      <div class="flex items-center justify-between gap-3">
        <span class="flex flex-1 items-center gap-2.5 text-base min-w-0">
          <Terminal class="h-4 w-4 shrink-0 text-muted-foreground" />
          <span class="flex flex-col min-w-0">
            <span>{{ t('settings.cli.title') }}</span>
            <span v-if="cli" class="mono truncate text-xs text-muted-foreground">
              {{ cli.target_dir }}/memex
            </span>
          </span>
        </span>
        <Button
          v-if="!cli?.installed"
          variant="default"
          size="sm"
          :disabled="cliLoading"
          @click="installCli"
        >
          <RefreshCw v-if="cliLoading" class="mr-1.5 h-3.5 w-3.5 animate-spin" />
          {{ cliLoading ? t('settings.cli.installing') : t('settings.cli.install') }}
        </Button>
        <Button
          v-else
          variant="outline"
          size="sm"
          :disabled="cliLoading"
          @click="uninstallCli"
        >
          <RefreshCw v-if="cliLoading" class="mr-1.5 h-3.5 w-3.5 animate-spin" />
          {{ cliLoading ? t('settings.cli.uninstalling') : t('settings.cli.uninstall') }}
        </Button>
      </div>

      <!-- 安装成功且 PATH 已包含目标目录 -->
      <div
        v-if="cli?.installed && cli.path_contains_target_dir"
        class="flex items-center gap-2 rounded-md border border-emerald-500/30 bg-emerald-500/5 px-3 py-2 text-xs text-emerald-600 dark:text-emerald-400"
      >
        <CheckCircle2 class="h-3.5 w-3.5 shrink-0" />
        <span>{{ t('settings.cli.ready') }}</span>
      </div>

      <!-- 安装了但 PATH 里没目标目录 -->
      <div
        v-else-if="cli?.installed && !cli.path_contains_target_dir"
        class="flex flex-col gap-2 rounded-md border border-amber-500/30 bg-amber-500/5 px-3 py-2 text-xs text-amber-700 dark:text-amber-400"
      >
        <span class="flex items-start gap-2">
          <AlertCircle class="mt-0.5 h-3.5 w-3.5 shrink-0" />
          <span>{{ t('settings.cli.path_missing') }}</span>
        </span>
        <div class="flex items-center gap-2">
          <code class="mono flex-1 truncate rounded bg-amber-500/10 px-2 py-1 text-[11px]">{{ cli.path_export_hint }}</code>
          <Button variant="ghost" size="sm" class="h-7 shrink-0 px-2" @click="copyExportHint">
            <Copy class="mr-1 h-3 w-3" />
            {{ cliCopied ? t('common.copied') : t('common.copy') }}
          </Button>
        </div>
      </div>

      <!-- 错误 -->
      <div
        v-else-if="cliError"
        class="flex items-center gap-2 rounded-md border border-red-500/30 bg-red-500/5 px-3 py-2 text-xs text-red-600 dark:text-red-400"
      >
        <AlertCircle class="h-3.5 w-3.5 shrink-0" />
        <span class="truncate">{{ cliError }}</span>
      </div>
    </div>

    <Separator class="my-2" />

    <!-- 关于 -->
    <p class="text-sm font-semibold uppercase tracking-wider text-muted-foreground">{{ t('settings.section.about') }}</p>
    <div class="flex flex-col gap-2 py-2.5">
      <div class="flex items-center justify-between gap-4">
        <span class="flex flex-col gap-0.5 min-w-0">
          <span class="text-base">{{ t('settings.about.version') }}</span>
          <span class="text-xs text-muted-foreground">{{ t('settings.about.upgrade_hint') }}</span>
        </span>
        <div class="flex items-center gap-3 shrink-0">
          <span class="text-xs text-muted-foreground font-mono">v{{ APP_VERSION }}</span>
          <Button
            variant="outline"
            size="sm"
            class="gap-1.5"
            :disabled="updateStatus === 'checking'"
            @click="checkForUpdates"
          >
            <RefreshCw class="h-3.5 w-3.5" :class="updateStatus === 'checking' ? 'animate-spin' : ''" />
            {{ updateStatus === 'checking' ? t('settings.about.checking') : t('settings.about.check_update') }}
          </Button>
        </div>
      </div>

      <!-- 检查结果显示区 -->
      <div
        v-if="updateStatus === 'latest'"
        class="flex items-center gap-2 rounded-md border border-emerald-500/30 bg-emerald-500/5 px-3 py-2 text-xs text-emerald-600 dark:text-emerald-400"
      >
        <CheckCircle2 class="h-3.5 w-3.5 shrink-0" />
        <span>{{ t('settings.about.up_to_date') }}</span>
      </div>

      <div
        v-else-if="updateStatus === 'outdated'"
        class="flex items-center justify-between gap-2 rounded-md border border-amber-500/30 bg-amber-500/5 px-3 py-2 text-xs"
      >
        <span class="flex items-center gap-2 text-amber-700 dark:text-amber-400 min-w-0">
          <Download class="h-3.5 w-3.5 shrink-0" />
          <span class="truncate">{{ t('settings.about.new_version_available', { v: remoteVersion }) }}</span>
        </span>
        <Button variant="link" size="sm" class="h-auto p-0 text-xs text-amber-700 dark:text-amber-400 shrink-0" @click="openReleasePage">
          {{ t('settings.about.view_release') }}
        </Button>
      </div>

      <div
        v-else-if="updateStatus === 'error'"
        class="flex items-center justify-between gap-2 rounded-md border border-red-500/30 bg-red-500/5 px-3 py-2 text-xs text-red-600 dark:text-red-400"
      >
        <span class="flex items-center gap-2 min-w-0">
          <AlertCircle class="h-3.5 w-3.5 shrink-0" />
          <span class="truncate">{{ t('settings.about.check_failed', { err: errorMessage }) }}</span>
        </span>
        <Button variant="link" size="sm" class="h-auto p-0 text-xs text-red-600 dark:text-red-400 shrink-0" @click="openReleasePage">
          {{ t('settings.about.open_in_browser') }}
        </Button>
      </div>
    </div>
  </div>
</template>
