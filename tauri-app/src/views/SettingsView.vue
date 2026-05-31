<script setup lang="ts">
import { ref } from 'vue'
import { ToggleRight, Shield, Brain, Database } from 'lucide-vue-next'
import ViewHeader from '@/components/ViewHeader.vue'

interface AdapterSetting {
  key: string
  label: string
  path: string
  enabled: boolean
}

const adapters = ref<AdapterSetting[]>([
  { key: 'claude_code', label: 'Claude Code', path: '~/.claude/projects', enabled: true },
  { key: 'cursor', label: 'Cursor', path: '~/Library/.../workspaceStorage', enabled: true },
  { key: 'codex', label: 'Codex', path: '~/.codex/sessions', enabled: true },
  { key: 'opencode', label: 'OpenCode', path: '~/.opencode', enabled: false },
])

const privacy = ref({
  autoRedact: true,
  privateFromMcp: true,
  privateMode: false,
})

const llm = ref({
  ollamaModel: 'qwen2.5:7b',
  claudeFallback: false,
})

function toggleAdapter(key: string) {
  const a = adapters.value.find((x) => x.key === key)
  if (a) a.enabled = !a.enabled
}
</script>

<template>
  <div class="flex h-full flex-col">
    <ViewHeader title="Settings" show-back />

    <div class="flex-1 space-y-4 overflow-y-auto px-3 py-3">
      <!-- Adapters -->
      <section>
        <h2 class="mb-2 flex items-center gap-1.5 text-xs font-medium uppercase tracking-wider text-muted-foreground">
          <Database class="h-3.5 w-3.5" />
          Adapters
        </h2>
        <div class="space-y-1">
          <button
            v-for="a in adapters"
            :key="a.key"
            class="flex w-full items-center justify-between rounded-lg px-3 py-2 transition-colors hover:bg-accent"
            @click="toggleAdapter(a.key)"
          >
            <div>
              <p class="text-sm font-medium">{{ a.label }}</p>
              <p class="text-xs text-muted-foreground">{{ a.path }}</p>
            </div>
            <ToggleRight
              :class="[
                'h-5 w-5 transition-colors',
                a.enabled ? 'text-primary' : 'text-muted-foreground',
              ]"
            />
          </button>
        </div>
      </section>

      <!-- Privacy -->
      <section>
        <h2 class="mb-2 flex items-center gap-1.5 text-xs font-medium uppercase tracking-wider text-muted-foreground">
          <Shield class="h-3.5 w-3.5" />
          Privacy
        </h2>
        <div class="space-y-1">
          <label class="flex cursor-pointer items-center justify-between rounded-lg px-3 py-2 hover:bg-accent">
            <span class="text-sm">Auto redact sensitive data</span>
            <input v-model="privacy.autoRedact" type="checkbox" class="accent-primary" />
          </label>
          <label class="flex cursor-pointer items-center justify-between rounded-lg px-3 py-2 hover:bg-accent">
            <span class="text-sm">Hide private sessions from MCP</span>
            <input v-model="privacy.privateFromMcp" type="checkbox" class="accent-primary" />
          </label>
          <label class="flex cursor-pointer items-center justify-between rounded-lg px-3 py-2 hover:bg-accent">
            <span class="text-sm">Private Mode</span>
            <input v-model="privacy.privateMode" type="checkbox" class="accent-primary" />
          </label>
        </div>
      </section>

      <!-- LLM -->
      <section>
        <h2 class="mb-2 flex items-center gap-1.5 text-xs font-medium uppercase tracking-wider text-muted-foreground">
          <Brain class="h-3.5 w-3.5" />
          LLM
        </h2>
        <div class="space-y-1">
          <div class="flex items-center justify-between rounded-lg px-3 py-2">
            <span class="text-sm">Local Summary (Ollama)</span>
            <span class="text-xs text-muted-foreground">{{ llm.ollamaModel }}</span>
          </div>
          <label class="flex cursor-pointer items-center justify-between rounded-lg px-3 py-2 hover:bg-accent">
            <div>
              <span class="text-sm">Claude Cloud Fallback</span>
              <p class="text-xs text-muted-foreground">Requires ANTHROPIC_API_KEY</p>
            </div>
            <input v-model="llm.claudeFallback" type="checkbox" class="accent-primary" />
          </label>
        </div>
      </section>
    </div>

    <!-- Footer -->
    <div class="shrink-0 border-t border-border px-3 py-2">
      <p class="text-center text-[10px] text-muted-foreground">
        Memex v0.1.0 · Data stays local
      </p>
    </div>
  </div>
</template>
