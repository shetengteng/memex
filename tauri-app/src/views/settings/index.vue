<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useMemex } from '@/composables/useMemex'
import { Switch } from '@/components/ui/switch'
import { Separator } from '@/components/ui/separator'
import { Button } from '@/components/ui/button'
import { RefreshCw, Download } from 'lucide-vue-next'
import { useI18n, setLocale, LOCALE_OPTIONS, type Locale } from '@/i18n'

const { t, locale } = useI18n()
const { toggleAdapter: ipcToggleAdapter, getConfig, setConfig } = useMemex()

interface UpdateInfo {
  available: boolean
  current_version: string
  latest_version: string | null
  notes: string | null
}

const update = ref<{
  checking: boolean
  installing: boolean
  info: UpdateInfo | null
  error: string
  message: string
}>({
  checking: false,
  installing: false,
  info: null,
  error: '',
  message: '',
})

async function checkForUpdates() {
  update.value.checking = true
  update.value.error = ''
  update.value.message = ''
  try {
    update.value.info = await invoke<UpdateInfo>('check_for_updates')
    if (!update.value.info.available) {
      update.value.message = t('settings.update.already_latest', { version: update.value.info.current_version })
    }
  } catch (e: unknown) {
    update.value.error = e instanceof Error ? e.message : String(e)
  } finally {
    update.value.checking = false
  }
}

async function installUpdate() {
  update.value.installing = true
  update.value.error = ''
  try {
    await invoke('install_update')
  } catch (e: unknown) {
    update.value.error = e instanceof Error ? e.message : String(e)
  } finally {
    update.value.installing = false
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

interface IdeRow {
  ide: string
  label: string
  status: IdeStatus | null
  loading: boolean
  error: string
}

const ideRows = ref<IdeRow[]>([
  { ide: 'cursor', label: 'Cursor', status: null, loading: false, error: '' },
  { ide: 'claude-code', label: 'Claude Code', status: null, loading: false, error: '' },
  { ide: 'codex', label: 'Codex', status: null, loading: false, error: '' },
  { ide: 'opencode', label: 'OpenCode', status: null, loading: false, error: '' },
])

async function refreshIdeStatuses() {
  try {
    const list = await invoke<IdeStatus[]>('ide_list_status')
    for (const s of list) {
      const row = ideRows.value.find((r) => r.ide === s.ide)
      if (row) row.status = s
    }
  } catch (e) {
    // CLI 找不到时 4 个 row 都保持 null，UI 会显示「未配置」
    console.warn('ide_list_status failed', e)
  }
}

async function toggleIde(row: IdeRow, next: boolean) {
  if (row.loading) return
  const prevStatus = row.status
  row.loading = true
  row.error = ''
  try {
    const cmd = next ? 'ide_install' : 'ide_uninstall'
    row.status = await invoke<IdeStatus>(cmd, { ide: row.ide })
  } catch (e) {
    row.status = prevStatus
    row.error = e instanceof Error ? e.message : String(e)
  } finally {
    row.loading = false
  }
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
      class="flex items-center justify-between py-2.5"
      :class="{ 'border-t border-border/40': i > 0 }"
    >
      <span class="flex items-center gap-2.5 text-base">
        <span
          class="inline-block h-2.5 w-2.5 rounded-full"
          :class="row.status?.installed ? 'bg-success' : 'bg-muted-foreground'"
        />
        <span class="flex flex-col">
          <span>{{ row.label }}</span>
          <span class="text-xs text-muted-foreground">
            <template v-if="row.loading">
              {{ row.status?.installed ? t('settings.integrations.uninstalling') : t('settings.integrations.installing') }}
            </template>
            <template v-else-if="row.error">
              <span class="text-destructive">{{ t('settings.integrations.error_prefix') }}{{ row.error }}</span>
            </template>
            <template v-else-if="!row.status?.config_exists">
              {{ t('settings.integrations.status.no_config') }}
            </template>
            <template v-else-if="row.status?.installed">
              {{ t('settings.integrations.status.installed') }}
            </template>
            <template v-else>
              {{ t('settings.integrations.status.not_installed') }}
            </template>
          </span>
        </span>
      </span>
      <Switch
        :model-value="row.status?.installed ?? false"
        :disabled="row.loading"
        class="h-7 w-12 [&_>span]:h-6 [&_>span]:w-6 [&[data-state=checked]_>span]:translate-x-5"
        @update:model-value="(v: boolean) => toggleIde(row, v)"
      />
    </div>
    <p class="text-xs text-muted-foreground italic">{{ t('settings.integrations.restart_hint') }}</p>

    <Separator class="my-2" />

    <!-- 关于 / 更新 -->
    <p class="text-sm font-semibold uppercase tracking-wider text-muted-foreground">{{ t('settings.section.about') }}</p>
    <div class="flex items-center justify-between py-2.5">
      <span class="flex flex-col gap-0.5">
        <span class="text-base">{{ t('settings.about.version') }}</span>
        <span class="text-xs text-muted-foreground">{{ update.info?.current_version ?? '0.1.0' }}</span>
      </span>
      <Button variant="ghost" size="sm" :disabled="update.checking || update.installing" class="h-8 gap-1.5" @click="checkForUpdates">
        <RefreshCw class="h-3.5 w-3.5" :class="{ 'animate-spin': update.checking }" />
        {{ update.checking ? t('settings.update.checking') : t('settings.update.check') }}
      </Button>
    </div>

    <div v-if="update.info?.available" class="rounded-md border border-primary/30 bg-primary/5 p-3">
      <div class="flex items-baseline justify-between gap-2">
        <span class="text-sm font-medium text-primary">
          {{ t('settings.update.available', { version: update.info.latest_version ?? '' }) }}
        </span>
        <Button variant="ghost" size="sm" :disabled="update.installing" class="h-7 gap-1.5 text-xs" @click="installUpdate">
          <Download class="h-3 w-3" :class="{ 'animate-pulse': update.installing }" />
          {{ update.installing ? t('settings.update.installing') : t('settings.update.install') }}
        </Button>
      </div>
      <div v-if="update.info.notes" class="mt-2 text-xs text-muted-foreground">
        <div class="mb-1 font-medium">{{ t('settings.update.notes') }}</div>
        <pre class="whitespace-pre-wrap font-sans">{{ update.info.notes }}</pre>
      </div>
    </div>
    <p v-else-if="update.message" class="text-xs text-muted-foreground">{{ update.message }}</p>
    <p v-if="update.error" class="text-xs text-destructive">{{ update.error }}</p>
  </div>
</template>
