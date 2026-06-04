<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useMemex } from '@/composables/useMemex'
import { useI18n } from '@/i18n'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import { Badge } from '@/components/ui/badge'
import {
  Plus,
  Trash2,
  FlaskConical,
  RefreshCw,
  Eye,
  EyeOff,
  Star,
  Check,
  X,
  Server,
  Pencil,
  Cloud,
  Bot,
} from 'lucide-vue-next'
import type { LlmProvider } from '@/types'

const { t } = useI18n()
const {
  llmProviderList,
  llmProviderUpsert,
  llmProviderDelete,
  llmProviderTest,
  llmListModels,
} = useMemex()

const providers = ref<LlmProvider[]>([])
const loading = ref(false)

const templates = [
  { name: 'OpenAI', kind: 'openai_compat', baseUrl: 'https://api.openai.com/v1', model: 'gpt-4o-mini', icon: Bot },
  { name: 'Moonshot Kimi', kind: 'openai_compat', baseUrl: 'https://api.moonshot.cn/v1', model: 'moonshot-v1-8k', icon: Cloud },
  { name: 'SiliconFlow', kind: 'openai_compat', baseUrl: 'https://api.siliconflow.cn/v1', model: 'Qwen/Qwen2.5-32B-Instruct', icon: Cloud },
  { name: 'Together AI', kind: 'openai_compat', baseUrl: 'https://api.together.xyz/v1', model: 'meta-llama/Llama-3.3-70B', icon: Cloud },
  { name: 'Groq', kind: 'openai_compat', baseUrl: 'https://api.groq.com/openai/v1', model: 'llama-3.3-70b-versatile', icon: Cloud },
  { name: 'Ollama Local', kind: 'ollama', baseUrl: 'http://127.0.0.1:11434', model: '', icon: Server },
]

interface EditingState {
  id: string
  name: string
  kind: string
  baseUrl: string
  model: string
  apiKey: string
  enabled: boolean
  isDefault: boolean
}

const showAdd = ref(false)
const editing = ref<EditingState | null>(null)
const showApiKey = ref(false)
const testing = ref<string | null>(null)
const models = ref<string[]>([])
const modelsLoading = ref(false)

function newEditing(): EditingState {
  return {
    id: crypto.randomUUID(),
    name: '',
    kind: 'openai_compat',
    baseUrl: '',
    model: '',
    apiKey: '',
    enabled: true,
    isDefault: providers.value.length === 0,
  }
}

function pickTemplate(tpl: typeof templates[number]) {
  if (!editing.value) editing.value = newEditing()
  editing.value.name = editing.value.name || tpl.name
  editing.value.kind = tpl.kind
  editing.value.baseUrl = tpl.baseUrl
  editing.value.model = editing.value.model || tpl.model
  models.value = []
}

const canSave = computed(() => {
  const e = editing.value
  return e && e.name.trim() && e.baseUrl.trim()
})

const canFetchModels = computed(() => {
  const e = editing.value
  if (!e?.baseUrl?.trim()) return false
  if (e.kind === 'ollama') return true
  return !!e.apiKey?.trim()
})

async function fetchModels() {
  const e = editing.value
  if (!e?.baseUrl?.trim()) return
  modelsLoading.value = true
  try {
    const ids = await llmListModels(e.kind, e.baseUrl, e.apiKey)
    models.value = ids
  } catch (err) {
    console.warn('fetch models failed:', err)
    models.value = []
  } finally {
    modelsLoading.value = false
  }
}

let fetchTimer: ReturnType<typeof setTimeout> | null = null
watch(
  () => [editing.value?.baseUrl, editing.value?.apiKey, editing.value?.kind],
  () => {
    if (fetchTimer) clearTimeout(fetchTimer)
    if (!showAdd.value || !canFetchModels.value) return
    fetchTimer = setTimeout(() => {
      fetchTimer = null
      void fetchModels()
    }, 800)
  },
)

async function save() {
  const e = editing.value
  if (!e) return
  try {
    await llmProviderUpsert({
      id: e.id,
      name: e.name.trim(),
      kind: e.kind,
      baseUrl: e.baseUrl.trim(),
      model: e.model.trim(),
      apiKey: e.apiKey,
      enabled: e.enabled,
      isDefault: e.isDefault,
    })
    showAdd.value = false
    editing.value = null
    models.value = []
    await refresh()
  } catch (err) {
    console.error('save provider failed:', err)
  }
}

async function remove(id: string) {
  try {
    await llmProviderDelete(id)
    await refresh()
  } catch (err) {
    console.error('delete provider failed:', err)
  }
}

async function testProvider(id: string) {
  testing.value = id
  try {
    await llmProviderTest(id)
    await refresh()
  } catch (err) {
    console.error('test provider failed:', err)
  } finally {
    testing.value = null
  }
}

async function toggleEnabled(p: LlmProvider) {
  try {
    await llmProviderUpsert({
      id: p.id,
      name: p.name,
      kind: p.kind,
      baseUrl: p.baseUrl,
      model: p.model,
      apiKey: '',
      enabled: !p.enabled,
      isDefault: p.isDefault,
    })
    await refresh()
  } catch (err) {
    console.error('toggle failed:', err)
  }
}

async function setDefault(p: LlmProvider) {
  try {
    await llmProviderUpsert({
      id: p.id,
      name: p.name,
      kind: p.kind,
      baseUrl: p.baseUrl,
      model: p.model,
      apiKey: '',
      enabled: p.enabled,
      isDefault: true,
    })
    await refresh()
  } catch (err) {
    console.error('set default failed:', err)
  }
}

function startEdit(p: LlmProvider) {
  editing.value = {
    id: p.id,
    name: p.name,
    kind: p.kind,
    baseUrl: p.baseUrl,
    model: p.model,
    apiKey: '',
    enabled: p.enabled,
    isDefault: p.isDefault,
  }
  showAdd.value = true
  showApiKey.value = false
  models.value = []
}

function startAdd() {
  editing.value = newEditing()
  showAdd.value = true
  showApiKey.value = false
  models.value = []
}

function cancelAdd() {
  showAdd.value = false
  editing.value = null
  models.value = []
}

async function refresh() {
  loading.value = true
  try {
    providers.value = await llmProviderList()
  } catch (err) {
    console.error('load providers failed:', err)
  } finally {
    loading.value = false
  }
}

onMounted(refresh)

function kindLabel(kind: string) {
  switch (kind) {
    case 'openai_compat': return 'OpenAI'
    case 'anthropic': return 'Anthropic'
    case 'ollama': return 'Ollama'
    default: return kind
  }
}

function statusColor(status: string) {
  switch (status) {
    case 'ok': return 'text-emerald-600'
    case 'error': return 'text-rose-500'
    default: return 'text-muted-foreground'
  }
}
</script>

<template>
  <div class="space-y-3">
    <!-- Provider 列表 -->
    <div v-if="providers.length > 0" class="space-y-2">
      <div
        v-for="p in providers"
        :key="p.id"
        class="flex items-center gap-3 rounded-lg border bg-card px-3 py-2.5"
      >
        <Switch
          :model-value="p.enabled"
          @update:model-value="toggleEnabled(p)"
          class="scale-75"
        />
        <div class="flex-1 min-w-0">
          <div class="flex items-center gap-1.5">
            <span class="text-xs font-medium truncate">{{ p.name }}</span>
            <Badge v-if="p.isDefault" variant="secondary" class="text-[9px] px-1 py-0">
              {{ t('settings.providers.default') }}
            </Badge>
            <Badge variant="outline" class="text-[9px] px-1 py-0">{{ kindLabel(p.kind) }}</Badge>
          </div>
          <div class="text-[10px] text-muted-foreground truncate">
            {{ p.model || '—' }} · {{ p.baseUrl }}
          </div>
        </div>
        <div class="flex items-center gap-1 shrink-0">
          <span v-if="p.status === 'ok'" :class="statusColor(p.status)" class="text-[10px]">
            <Check class="inline h-3 w-3" /> {{ p.latencyMs }}ms
          </span>
          <span v-else-if="p.status === 'error'" :class="statusColor(p.status)" class="text-[10px]">
            <X class="inline h-3 w-3" /> error
          </span>
          <Button
            variant="ghost"
            size="sm"
            class="h-6 w-6 p-0"
            :disabled="testing === p.id"
            @click="testProvider(p.id)"
          >
            <FlaskConical v-if="testing !== p.id" class="h-3 w-3" />
            <RefreshCw v-else class="h-3 w-3 animate-spin" />
          </Button>
          <Button
            v-if="!p.isDefault && p.enabled"
            variant="ghost"
            size="sm"
            class="h-6 w-6 p-0"
            :title="t('settings.providers.set_default')"
            @click="setDefault(p)"
          >
            <Star class="h-3 w-3" />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            class="h-6 w-6 p-0 text-muted-foreground hover:text-foreground"
            @click="startEdit(p)"
          >
            <Pencil class="h-3 w-3" />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            class="h-6 w-6 p-0 text-muted-foreground hover:text-rose-500"
            @click="remove(p.id)"
          >
            <Trash2 class="h-3 w-3" />
          </Button>
        </div>
      </div>
    </div>
    <p v-else-if="!loading" class="text-xs text-muted-foreground">
      {{ t('settings.providers.empty') }}
    </p>

    <!-- 添加/编辑表单 -->
    <div v-if="showAdd && editing" class="rounded-lg border bg-muted/30 p-3 space-y-3">
      <!-- 模板快选 -->
      <div class="space-y-1.5">
        <span class="text-[10px] font-medium text-muted-foreground">{{ t('settings.providers.from_template') }}</span>
        <div class="grid grid-cols-4 gap-1.5">
          <button
            v-for="tpl in templates"
            :key="tpl.name"
            class="flex items-center gap-1 rounded border bg-card px-1.5 py-1 text-[10px] transition-colors hover:border-primary/40 hover:bg-accent truncate"
            @click="pickTemplate(tpl)"
          >
            <component :is="tpl.icon" class="h-3 w-3 shrink-0 text-primary" />
            <span class="truncate">{{ tpl.name }}</span>
          </button>
        </div>
      </div>

      <!-- 表单字段 -->
      <div class="grid grid-cols-2 gap-2">
        <div class="space-y-1">
          <label class="text-[10px] font-medium">{{ t('settings.providers.name') }}</label>
          <Input v-model="editing.name" placeholder="OpenAI" class="h-7 text-xs" />
        </div>
        <div class="space-y-1">
          <label class="text-[10px] font-medium">{{ t('settings.providers.kind') }}</label>
          <select v-model="editing.kind" class="h-7 w-full rounded-md border bg-background px-2 text-xs">
            <option value="openai_compat">OpenAI Compatible (OpenAI / DeepSeek / Moonshot / SiliconFlow / Together / Groq / ...)</option>
            <option value="anthropic">Anthropic</option>
            <option value="ollama">Ollama</option>
          </select>
        </div>
      </div>
      <div class="space-y-1">
        <label class="text-[10px] font-medium">Base URL</label>
        <Input v-model="editing.baseUrl" placeholder="https://api.example.com/v1" class="h-7 font-mono text-[10px]" />
      </div>
      <div class="space-y-1">
        <label class="text-[10px] font-medium">API Key</label>
        <div class="relative">
          <Input
            v-model="editing.apiKey"
            :type="showApiKey ? 'text' : 'password'"
            placeholder="sk-..."
            class="h-7 pr-7 font-mono text-[10px]"
            autocomplete="off"
          />
          <button
            type="button"
            class="absolute right-1 top-1/2 -translate-y-1/2 rounded p-0.5 text-muted-foreground hover:text-foreground"
            @click="showApiKey = !showApiKey"
          >
            <EyeOff v-if="showApiKey" class="h-3 w-3" />
            <Eye v-else class="h-3 w-3" />
          </button>
        </div>
      </div>
      <div class="space-y-1">
        <div class="flex items-center justify-between">
          <label class="text-[10px] font-medium">{{ t('settings.providers.model') }}</label>
          <Button
            variant="ghost"
            size="sm"
            class="h-5 px-1.5 text-[9px]"
            :disabled="!canFetchModels || modelsLoading"
            @click="fetchModels"
          >
            <RefreshCw v-if="!modelsLoading" class="mr-0.5 h-2.5 w-2.5" />
            <RefreshCw v-else class="mr-0.5 h-2.5 w-2.5 animate-spin" />
            {{ t('settings.providers.fetch_models') }}
          </Button>
        </div>
        <Input v-model="editing.model" placeholder="gpt-4o-mini / qwen2.5:7b" class="h-7 font-mono text-[10px]" />
        <div v-if="models.length > 0" class="flex flex-wrap gap-1 pt-0.5">
          <button
            v-for="m in models.slice(0, 12)"
            :key="m"
            type="button"
            class="rounded border bg-muted/40 px-1 py-0.5 font-mono text-[9px] transition-colors hover:bg-accent"
            :class="{ 'border-primary/40 bg-primary/10 text-primary': editing.model === m }"
            @click="editing.model = m"
          >
            {{ m }}
          </button>
          <span v-if="models.length > 12" class="px-1 py-0.5 text-[9px] text-muted-foreground">
            +{{ models.length - 12 }} more
          </span>
        </div>
      </div>

      <!-- 开关 -->
      <div class="flex items-center gap-3">
        <label class="flex items-center gap-1.5 text-[10px]">
          <Switch v-model="editing.isDefault" class="scale-75" />
          {{ t('settings.providers.set_default') }}
        </label>
      </div>

      <!-- 操作按钮 -->
      <div class="flex items-center gap-2 pt-1">
        <Button size="sm" class="h-7 text-xs" :disabled="!canSave" @click="save">
          {{ t('common.save') }}
        </Button>
        <Button variant="outline" size="sm" class="h-7 text-xs" @click="cancelAdd">
          {{ t('common.cancel') }}
        </Button>
      </div>
    </div>

    <!-- 添加按钮 -->
    <Button
      v-if="!showAdd"
      variant="outline"
      size="sm"
      class="h-7 text-xs"
      @click="startAdd"
    >
      <Plus class="mr-1 h-3 w-3" />
      {{ t('settings.providers.add') }}
    </Button>
  </div>
</template>
