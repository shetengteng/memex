<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useMemex } from '@/composables/useMemex'
import { Switch } from '@/components/ui/switch'
import { Separator } from '@/components/ui/separator'

const { toggleAdapter: ipcToggleAdapter, getConfig, setConfig } = useMemex()

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

async function toggleAdapter(key: string) {
  const a = adapters.value.find((x) => x.key === key)
  if (!a) return
  const newVal = !a.enabled
  try {
    await ipcToggleAdapter(key, newVal)
    a.enabled = newVal
  } catch { /* ignore */ }
}

async function toggleOllama() {
  llm.value.ollamaChecking = true
  const available = await checkOllamaAvailability()
  llm.value.ollamaAvailable = available
  llm.value.ollamaChecking = false

  if (!available && !llm.value.ollamaEnabled) {
    return
  }

  const newVal = !llm.value.ollamaEnabled
  try {
    await setConfig('llm.ollama_enabled', String(newVal))
    llm.value.ollamaEnabled = newVal
  } catch { /* ignore */ }
}

async function togglePrivacy(field: 'autoRedact' | 'privateFromMcp') {
  const newVal = !privacy.value[field]
  const keyMap = { autoRedact: 'privacy.auto_redact', privateFromMcp: 'privacy.private_from_mcp' }
  try {
    await setConfig(keyMap[field], String(newVal))
    privacy.value[field] = newVal
  } catch { /* ignore */ }
}

async function toggleCloudFallback() {
  const newVal = !llm.value.claudeFallback
  try {
    await setConfig('llm.cloud_fallback', String(newVal))
    llm.value.claudeFallback = newVal
  } catch { /* ignore */ }
}
</script>

<template>
  <div class="h-full space-y-1 overflow-y-auto px-3.5 py-2.5">
    <!-- Adapters -->
    <p class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">Adapters</p>
    <div
      v-for="(a, i) in adapters"
      :key="a.key"
      class="flex cursor-pointer items-center justify-between py-1.5"
      :class="{ 'border-t border-border/40': i > 0 }"
      @click="toggleAdapter(a.key)"
    >
      <span class="flex items-center gap-1.5 text-xs">
        <span class="inline-block h-1.5 w-1.5 rounded-full" :class="a.enabled ? 'bg-success' : 'bg-muted-foreground'" />
        {{ a.label }}
      </span>
      <Switch :checked="a.enabled" class="scale-75" @click.stop="toggleAdapter(a.key)" />
    </div>

    <Separator class="my-1.5" />

    <!-- LLM -->
    <p class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">LLM</p>
    <div class="flex cursor-pointer items-center justify-between py-1.5" @click="toggleOllama">
      <span class="flex items-center gap-1.5 text-xs">
        <span
          class="inline-block h-1.5 w-1.5 rounded-full"
          :class="llm.ollamaEnabled && llm.ollamaAvailable ? 'bg-success' : llm.ollamaChecking ? 'bg-warning animate-pulse' : 'bg-muted-foreground'"
        />
        Ollama ({{ llm.ollamaModel }})
      </span>
      <div class="flex items-center gap-1.5">
        <span
          class="mono text-[10px]"
          :class="llm.ollamaAvailable ? 'text-success' : 'text-destructive'"
        >
          {{ llm.ollamaChecking ? '...' : llm.ollamaAvailable ? 'local' : 'offline' }}
        </span>
        <Switch :checked="llm.ollamaEnabled" class="scale-75" @click.stop="toggleOllama" />
      </div>
    </div>
    <div class="flex cursor-pointer items-center justify-between border-t border-border/40 py-1.5" @click="toggleCloudFallback">
      <span class="flex items-center gap-1.5 text-xs">
        <span class="inline-block h-1.5 w-1.5 rounded-full" :class="llm.claudeFallback ? 'bg-success' : 'bg-muted-foreground'" />
        Claude fallback
      </span>
      <Switch :checked="llm.claudeFallback" class="scale-75" @click.stop="toggleCloudFallback" />
    </div>

    <Separator class="my-1.5" />

    <!-- Privacy -->
    <p class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">隐私</p>
    <div class="flex cursor-pointer items-center justify-between py-1.5" @click="togglePrivacy('autoRedact')">
      <span class="text-xs">自动脱敏</span>
      <Switch :checked="privacy.autoRedact" class="scale-75" @click.stop="togglePrivacy('autoRedact')" />
    </div>
    <div class="flex cursor-pointer items-center justify-between border-t border-border/40 py-1.5" @click="togglePrivacy('privateFromMcp')">
      <span class="text-xs">Private session 隐藏</span>
      <Switch :checked="privacy.privateFromMcp" class="scale-75" @click.stop="togglePrivacy('privateFromMcp')" />
    </div>
  </div>
</template>
