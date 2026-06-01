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
  { key: 'opencode', label: 'OpenCode', enabled: false },
  { key: 'aider', label: 'Aider', enabled: true },
  { key: 'continue_dev', label: 'Continue', enabled: true },
  { key: 'cline', label: 'Cline', enabled: true },
])

const privacy = ref({ autoRedact: true, privateFromMcp: true })
const llm = ref({ ollamaModel: 'qwen2.5:7b', claudeFallback: false })

onMounted(async () => {
  for (const a of adapters.value) {
    try {
      const val = await getConfig(`adapter.${a.key}.enabled`)
      if (val !== null) a.enabled = val === 'true'
    } catch { /* default */ }
  }
  try {
    const v = await getConfig('llm.cloud_fallback')
    if (v !== null) llm.value.claudeFallback = v === 'true'
  } catch { /* default */ }
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
      <Switch :checked="a.enabled" class="scale-75" @click.stop @update:checked="toggleAdapter(a.key)" />
    </div>

    <Separator class="my-1.5" />

    <!-- LLM -->
    <p class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">LLM</p>
    <div class="flex items-center justify-between py-1.5">
      <span class="flex items-center gap-1.5 text-xs">
        <span class="inline-block h-1.5 w-1.5 rounded-full bg-success" />
        Ollama ({{ llm.ollamaModel }})
      </span>
      <span class="mono text-[10px] text-success">local</span>
    </div>
    <div class="flex cursor-pointer items-center justify-between border-t border-border/40 py-1.5" @click="toggleCloudFallback">
      <span class="flex items-center gap-1.5 text-xs">
        <span class="inline-block h-1.5 w-1.5 rounded-full" :class="llm.claudeFallback ? 'bg-success' : 'bg-muted-foreground'" />
        Claude fallback
      </span>
      <Switch :checked="llm.claudeFallback" class="scale-75" @click.stop @update:checked="toggleCloudFallback" />
    </div>

    <Separator class="my-1.5" />

    <!-- Privacy -->
    <p class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">隐私</p>
    <div class="flex cursor-pointer items-center justify-between py-1.5" @click="togglePrivacy('autoRedact')">
      <span class="text-xs">自动脱敏</span>
      <Switch :checked="privacy.autoRedact" class="scale-75" @click.stop @update:checked="togglePrivacy('autoRedact')" />
    </div>
    <div class="flex cursor-pointer items-center justify-between border-t border-border/40 py-1.5" @click="togglePrivacy('privateFromMcp')">
      <span class="text-xs">Private session 隐藏</span>
      <Switch :checked="privacy.privateFromMcp" class="scale-75" @click.stop @update:checked="togglePrivacy('privateFromMcp')" />
    </div>
  </div>
</template>
