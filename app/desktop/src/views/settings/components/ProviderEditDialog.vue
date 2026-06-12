<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import { Switch } from '@/components/ui/switch'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  Bot,
  Cloud,
  Eye,
  EyeOff,
  Loader2,
  RefreshCw,
  Server,
  Sparkles,
  Zap,
} from 'lucide-vue-next'
import { toast } from 'vue-sonner'
import type { Provider } from '../types'
import { useMemex } from '@/composables/useMemex'
import { humanizeBackendError } from '@/lib/utils'
import { useI18n } from '@/i18n'

const props = defineProps<{ open: boolean; editing: Partial<Provider> }>()
const emit = defineEmits<{
  'update:open': [boolean]
  'update:editing': [Partial<Provider>]
  save: []
}>()

const memex = useMemex()
const { t } = useI18n()
const isEdit = computed(() => Boolean(props.editing.id))
const showApiKey = ref(false)
const testing = ref(false)

// 模型目录（从 provider 的 /v1/models 或 /api/tags 拉取）。
// 单 Input + 下方 chip 横排：点 chip 直接填回 Input。
const models = ref<string[]>([])
const modelsLoading = ref(false)
const modelsError = ref<string | null>(null)

const providerTemplates = [
  { name: 'DeepSeek', kind: 'openai_compat' as const, baseUrl: 'https://api.deepseek.com/v1', model: 'deepseek-chat', icon: Sparkles },
  { name: 'OpenAI', kind: 'openai_compat' as const, baseUrl: 'https://api.openai.com/v1', model: 'gpt-4o-mini', icon: Bot },
  { name: 'Moonshot Kimi', kind: 'openai_compat' as const, baseUrl: 'https://api.moonshot.cn/v1', model: 'moonshot-v1-8k', icon: Sparkles },
  { name: 'SiliconFlow', kind: 'openai_compat' as const, baseUrl: 'https://api.siliconflow.cn/v1', model: 'Qwen/Qwen2.5-32B-Instruct', icon: Cloud },
  { name: 'Together AI', kind: 'openai_compat' as const, baseUrl: 'https://api.together.xyz/v1', model: 'meta-llama/Llama-3.3-70B', icon: Cloud },
  { name: 'Groq', kind: 'openai_compat' as const, baseUrl: 'https://api.groq.com/openai/v1', model: 'llama-3.3-70b-versatile', icon: Zap },
  { name: 'Anthropic Claude', kind: 'anthropic' as const, baseUrl: 'https://api.anthropic.com', model: 'claude-3-5-sonnet-latest', icon: Bot },
  { name: 'Ollama Local', kind: 'ollama' as const, baseUrl: 'http://127.0.0.1:11434', model: 'qwen2.5:7b', icon: Server },
]

function update(key: keyof Provider, value: unknown) {
  emit('update:editing', { ...props.editing, [key]: value })
}

// 从模板填充：name / kind / baseUrl 必填，model 留空引导用户从下方 chip 选。
// 与 DiskMind 一致——避免预填的 model 跟模板默认值脱节后给用户错觉。
function pickTemplate(tpl: (typeof providerTemplates)[number]) {
  emit('update:editing', {
    ...props.editing,
    name: props.editing.name?.trim() ? props.editing.name : tpl.name,
    kind: tpl.kind,
    baseUrl: tpl.baseUrl,
    model: props.editing.model?.trim() ? props.editing.model : '',
  })
}

// 表单是否已有足够信息可调模型目录接口：
//   - Ollama 风格 endpoint（11434 / /api/tags）无需 key
//   - 其它类型需 baseUrl + apiKey 都有
const canFetchModels = computed(() => {
  const e = props.editing
  const baseUrl = (e.baseUrl || '').trim()
  const kind = (e.kind || '').trim().toLowerCase()
  if (!baseUrl || !kind) return false
  const looksLikeOllama = baseUrl.includes('11434') || baseUrl.includes('/api/tags')
  if (looksLikeOllama) return true
  return Boolean((e.apiKey || '').trim()) || kind.includes('local')
})

async function fetchModels(quiet = false) {
  if (modelsLoading.value) return
  const e = props.editing
  const kind = (e.kind || 'openai_compat').trim()
  const baseUrl = (e.baseUrl || '').trim()
  const apiKey = (e.apiKey || '').trim()
  if (!baseUrl) {
    if (!quiet) toast.error(t('settings.provider.toast.need_base_url'))
    return
  }
  modelsLoading.value = true
  modelsError.value = null
  try {
    const ids = await memex.llmListModels(kind, baseUrl, apiKey)
    models.value = ids
    if (!quiet) {
      if (ids.length === 0) toast.info(t('settings.provider.toast.no_models'))
      else toast.success(t('settings.provider.toast.found_n', { n: ids.length }))
    }
  } catch (err) {
    const fe = humanizeBackendError(err)
    modelsError.value = fe.friendly
    models.value = []
    if (!quiet) toast.error(t('settings.provider.toast.fetch_failed'), { description: fe.friendly, duration: 8000 })
  } finally {
    modelsLoading.value = false
  }
}

// 自动 debounce 拉取：watch kind / baseUrl / apiKey，停止输入 600ms 后静默重拉。
// 关闭 dialog 时清掉 timer 与状态，避免下次打开看到陈旧的 chip 清单。
let autoFetchTimer: ReturnType<typeof setTimeout> | null = null
watch(
  () => [props.editing.baseUrl, props.editing.apiKey, props.editing.kind] as const,
  () => {
    if (autoFetchTimer) clearTimeout(autoFetchTimer)
    if (!props.open) return
    if (!canFetchModels.value) return
    autoFetchTimer = setTimeout(() => {
      autoFetchTimer = null
      void fetchModels(/* quiet */ true)
    }, 600)
  },
)

watch(
  () => props.open,
  (v) => {
    if (!v) {
      if (autoFetchTimer) {
        clearTimeout(autoFetchTimer)
        autoFetchTimer = null
      }
      models.value = []
      modelsError.value = null
      showApiKey.value = false
    }
  },
)

function pickModel(id: string) {
  update('model', id)
}

// 草稿测试：不依赖 provider.id，直接拿当前表单值传给后端
// `llm_provider_test_draft`。这样用户在「新建」时填一行也能立刻验，无需先 Save。
async function testDraft() {
  if (testing.value) return
  const e = props.editing
  const name = (e.name || '').trim()
  const kind = (e.kind || 'openai_compat').trim()
  const baseUrl = (e.baseUrl || '').trim()
  const model = (e.model || '').trim()
  const apiKey = (e.apiKey || '').trim()

  if (!name || !baseUrl || !model) {
    toast.error(t('settings.provider.toast.need_fields'))
    return
  }
  if (kind !== 'ollama' && !apiKey) {
    toast.error(t('settings.provider.toast.need_apikey'))
    return
  }

  testing.value = true
  const loadingId = toast.loading(t('settings.llm.toast.testing', { name }))
  try {
    const r = await memex.llmProviderTestDraft(name, kind, baseUrl, model, apiKey)
    toast.dismiss(loadingId)
    if (r.ok) {
      toast.success(t('settings.provider.toast.test_ok', { ms: r.latencyMs }), {
        description: r.responseText ? r.responseText.slice(0, 120) : undefined,
      })
    } else {
      const fe = humanizeBackendError(r.error || 'unknown')
      toast.error(t('settings.provider.toast.test_failed'), { description: fe.friendly, duration: 8000 })
    }
  } catch (err) {
    toast.dismiss(loadingId)
    const fe = humanizeBackendError(err)
    toast.error(t('settings.provider.toast.test_failed'), { description: fe.friendly, duration: 8000 })
  } finally {
    testing.value = false
  }
}
</script>

<template>
  <Dialog :open="open" @update:open="(v) => emit('update:open', v)">
    <DialogContent
      class="sm:max-w-[720px] lg:max-w-[860px]"
    >
      <DialogHeader>
        <DialogTitle>{{ isEdit ? t('settings.provider.title_edit') : t('settings.provider.title_add') }}</DialogTitle>
        <DialogDescription>
          {{ t('settings.provider.desc') }}
        </DialogDescription>
      </DialogHeader>

      <div class="-mx-4 min-h-0 flex-1 space-y-4 overflow-y-auto px-4">

      <div v-if="!isEdit" class="space-y-2">
        <Label class="text-xs">{{ t('settings.provider.from_template') }}</Label>
        <div class="grid grid-cols-3 gap-2">
          <button
            v-for="tpl in providerTemplates"
            :key="tpl.name"
            class="flex items-center gap-2 rounded-lg border bg-card px-2.5 py-2 text-xs transition-colors hover:border-primary/40 hover:bg-accent"
            @click="pickTemplate(tpl)"
          >
            <component :is="tpl.icon" class="size-3.5 shrink-0 text-primary" />
            <span class="truncate">{{ tpl.name }}</span>
          </button>
        </div>
      </div>

      <Separator />

      <div class="grid gap-4 md:grid-cols-2">
        <div class="space-y-1.5">
          <Label class="text-xs">{{ t('settings.provider.field.name') }}</Label>
          <Input
            :model-value="editing.name"
            @update:model-value="(v) => update('name', v)"
            :placeholder="t('settings.provider.field.name_ph')"
            class="h-9"
          />
        </div>
        <div class="space-y-1.5">
          <Label class="text-xs">{{ t('settings.provider.field.kind') }}</Label>
          <Select
            :model-value="editing.kind"
            @update:model-value="(v) => update('kind', v)"
          >
            <SelectTrigger class="h-9 w-full"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="openai_compat">{{ t('settings.llm.kind.openai_compat') }}</SelectItem>
              <SelectItem value="anthropic">{{ t('settings.llm.kind.anthropic') }}</SelectItem>
              <SelectItem value="ollama">{{ t('settings.llm.kind.ollama') }}</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div class="space-y-1.5 md:col-span-2">
          <Label class="text-xs">{{ t('settings.provider.field.base_url') }}</Label>
          <Input
            :model-value="editing.baseUrl"
            @update:model-value="(v) => update('baseUrl', v)"
            :placeholder="t('settings.provider.field.base_url_ph')"
            class="h-9 font-mono text-xs"
          />
        </div>
        <div class="space-y-1.5 md:col-span-2">
          <Label class="text-xs">{{ t('settings.provider.field.api_key') }}</Label>
          <div class="relative">
            <Input
              :model-value="editing.apiKey"
              @update:model-value="(v) => update('apiKey', v)"
              :type="showApiKey ? 'text' : 'password'"
              placeholder="sk-..."
              class="h-9 pr-9 font-mono text-xs"
              autocomplete="off"
            />
            <button
              type="button"
              class="absolute right-1.5 top-1/2 -translate-y-1/2 rounded-md p-1 text-muted-foreground transition-colors hover:text-foreground"
              @click="showApiKey = !showApiKey"
            >
              <EyeOff v-if="showApiKey" class="size-3.5" />
              <Eye v-else class="size-3.5" />
            </button>
          </div>
        </div>
        <div class="space-y-1.5 md:col-span-2">
          <div class="flex items-center justify-between">
            <Label class="text-xs">{{ t('settings.provider.field.model') }}</Label>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              class="h-7 px-2 text-[11px]"
              :disabled="!canFetchModels || modelsLoading"
              @click="fetchModels(false)"
            >
              <Loader2 v-if="modelsLoading" class="mr-1 size-3 animate-spin" />
              <RefreshCw v-else class="mr-1 size-3" />
              {{ modelsLoading ? t('settings.provider.fetching') : t('settings.provider.fetch_models') }}
            </Button>
          </div>
          <!-- 单 Input；下方紧跟 chip 标签横排（DiskMind 同款）：点 chip 直接填入 Input -->
          <Input
            :model-value="editing.model"
            @update:model-value="(v) => update('model', v)"
            :placeholder="t('settings.provider.field.model_ph')"
            class="h-9 font-mono text-xs"
            autocomplete="off"
            spellcheck="false"
          />
          <div v-if="models.length > 0" class="flex flex-wrap gap-1 pt-1">
            <button
              v-for="m in models.slice(0, 12)"
              :key="m"
              type="button"
              class="rounded-md border bg-muted/40 px-1.5 py-0.5 font-mono text-[10px] transition-colors hover:bg-accent"
              :class="{ 'border-primary/40 bg-primary/10 text-primary': editing.model === m }"
              @click="pickModel(m)"
            >
              {{ m }}
            </button>
            <span v-if="models.length > 12" class="px-1 py-0.5 text-[10px] text-muted-foreground">
              {{ t('settings.provider.chip_more', { n: models.length - 12 }) }}
            </span>
          </div>
          <p v-if="modelsError" class="text-[10px] text-rose-500">
            {{ t('settings.provider.fetch_failed', { err: modelsError }) }}
          </p>
          <p v-else-if="modelsLoading" class="text-[10px] text-muted-foreground">
            {{ t('settings.provider.fetching_status') }}
          </p>
          <p v-else-if="!canFetchModels" class="text-[10px] text-muted-foreground">
            {{ t('settings.provider.fetch_hint') }}
          </p>
          <p v-else-if="models.length === 0" class="text-[10px] text-muted-foreground">
            {{ t('settings.provider.fetch_empty') }}
          </p>
          <p v-else class="text-[10px] text-muted-foreground">
            {{ t('settings.provider.fetch_total', { total: models.length }) }}
          </p>
        </div>
        <div class="flex items-center justify-between rounded-md border px-3 py-2 md:col-span-2">
          <div>
            <Label class="text-xs">{{ t('settings.provider.set_default') }}</Label>
            <p class="text-[11px] text-muted-foreground">
              {{ t('settings.provider.set_default_hint') }}
            </p>
          </div>
          <Switch
            :model-value="editing.isDefault"
            @update:model-value="(v) => update('isDefault', v)"
          />
        </div>
      </div>
      </div>

      <DialogFooter>
        <Button
          variant="outline"
          class="mr-auto"
          :disabled="testing"
          @click="testDraft"
        >
          <RefreshCw v-if="testing" class="mr-1.5 size-3.5 animate-spin" />
          <Zap v-else class="mr-1.5 size-3.5" />
          {{ testing ? t('settings.provider.test_busy') : t('settings.provider.test_btn') }}
        </Button>
        <Button variant="outline" @click="emit('update:open', false)">{{ t('settings.provider.cancel') }}</Button>
        <Button @click="emit('save')">{{ t('settings.provider.save') }}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
