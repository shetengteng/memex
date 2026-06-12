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

const props = defineProps<{ open: boolean; editing: Partial<Provider> }>()
const emit = defineEmits<{
  'update:open': [boolean]
  'update:editing': [Partial<Provider>]
  save: []
}>()

const memex = useMemex()
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
    if (!quiet) toast.error('请先填写 Base URL')
    return
  }
  modelsLoading.value = true
  modelsError.value = null
  try {
    const ids = await memex.llmListModels(kind, baseUrl, apiKey)
    models.value = ids
    if (!quiet) {
      if (ids.length === 0) toast.info('未查到模型清单')
      else toast.success(`发现 ${ids.length} 个可用模型`)
    }
  } catch (err) {
    const fe = humanizeBackendError(err)
    modelsError.value = fe.friendly
    models.value = []
    if (!quiet) toast.error('拉取模型失败', { description: fe.friendly, duration: 8000 })
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
    toast.error('请先填写 名称 / Base URL / 模型 再测试')
    return
  }
  if (kind !== 'ollama' && !apiKey) {
    toast.error('请先填写 API Key 再测试')
    return
  }

  testing.value = true
  const loadingId = toast.loading(`正在测试 ${name}…`)
  try {
    const r = await memex.llmProviderTestDraft(name, kind, baseUrl, model, apiKey)
    toast.dismiss(loadingId)
    if (r.ok) {
      toast.success(`测试通过 · ${r.latencyMs}ms`, {
        description: r.responseText ? r.responseText.slice(0, 120) : undefined,
      })
    } else {
      const fe = humanizeBackendError(r.error || 'unknown')
      toast.error('测试失败', { description: fe.friendly, duration: 8000 })
    }
  } catch (err) {
    toast.dismiss(loadingId)
    const fe = humanizeBackendError(err)
    toast.error('测试失败', { description: fe.friendly, duration: 8000 })
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
        <DialogTitle>{{ isEdit ? '编辑 Provider' : '添加 Provider' }}</DialogTitle>
        <DialogDescription>
          配置 LLM 服务商：从模板快速填充，或手动输入 Base URL 与 API Key
        </DialogDescription>
      </DialogHeader>

      <div class="-mx-4 min-h-0 flex-1 space-y-4 overflow-y-auto px-4">

      <div v-if="!isEdit" class="space-y-2">
        <Label class="text-xs">从模板新建</Label>
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
          <Label class="text-xs">名称</Label>
          <Input
            :model-value="editing.name"
            @update:model-value="(v) => update('name', v)"
            placeholder="给这个 Provider 起个名"
            class="h-9"
          />
        </div>
        <div class="space-y-1.5">
          <Label class="text-xs">类型</Label>
          <Select
            :model-value="editing.kind"
            @update:model-value="(v) => update('kind', v)"
          >
            <SelectTrigger class="h-9 w-full"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="openai_compat">OpenAI 兼容</SelectItem>
              <SelectItem value="anthropic">Anthropic</SelectItem>
              <SelectItem value="ollama">Ollama</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div class="space-y-1.5 md:col-span-2">
          <Label class="text-xs">Base URL</Label>
          <Input
            :model-value="editing.baseUrl"
            @update:model-value="(v) => update('baseUrl', v)"
            placeholder="https://api.example.com/v1"
            class="h-9 font-mono text-xs"
          />
        </div>
        <div class="space-y-1.5 md:col-span-2">
          <Label class="text-xs">API Key</Label>
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
            <Label class="text-xs">模型</Label>
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
              {{ modelsLoading ? '拉取中…' : '拉取可用模型' }}
            </Button>
          </div>
          <!-- 单 Input；下方紧跟 chip 标签横排（DiskMind 同款）：点 chip 直接填入 Input -->
          <Input
            :model-value="editing.model"
            @update:model-value="(v) => update('model', v)"
            placeholder="deepseek-chat / gpt-4o-mini / qwen2.5:3b"
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
              +{{ models.length - 12 }} 个未展示，可直接在输入框搜索
            </span>
          </div>
          <p v-if="modelsError" class="text-[10px] text-rose-500">
            拉取失败：{{ modelsError }}
          </p>
          <p v-else-if="modelsLoading" class="text-[10px] text-muted-foreground">
            正在拉取可用模型…
          </p>
          <p v-else-if="!canFetchModels" class="text-[10px] text-muted-foreground">
            填好 Base URL 与 API Key 后会自动拉取该 Provider 支持的模型
          </p>
          <p v-else-if="models.length === 0" class="text-[10px] text-muted-foreground">
            未发现模型，可点上方「拉取可用模型」或直接手动输入
          </p>
          <p v-else class="text-[10px] text-muted-foreground">
            共 {{ models.length }} 个可用模型 · 点击下方 chip 填入或手动编辑
          </p>
        </div>
        <div class="flex items-center justify-between rounded-md border px-3 py-2 md:col-span-2">
          <div>
            <Label class="text-xs">设为默认</Label>
            <p class="text-[11px] text-muted-foreground">
              LLM 调用会优先使用默认 Provider，失败后按链路 fallback
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
          {{ testing ? '测试中…' : '测试连接' }}
        </Button>
        <Button variant="outline" @click="emit('update:open', false)">取消</Button>
        <Button @click="emit('save')">保存</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
