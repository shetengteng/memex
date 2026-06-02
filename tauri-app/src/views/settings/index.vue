<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useMemex } from '@/composables/useMemex'
import { Switch } from '@/components/ui/switch'
import { Separator } from '@/components/ui/separator'
import { useI18n, setLocale, LOCALE_OPTIONS, type Locale } from '@/i18n'

const { t, locale } = useI18n()
const { toggleAdapter: ipcToggleAdapter, getConfig, setConfig } = useMemex()

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
  </div>
</template>
