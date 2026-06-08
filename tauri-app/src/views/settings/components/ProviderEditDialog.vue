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
const listingModels = ref(false)
const availableModels = ref<string[]>([])

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

function pickTemplate(tpl: (typeof providerTemplates)[number]) {
  emit('update:editing', {
    ...props.editing,
    name: props.editing.name?.trim() ? props.editing.name : tpl.name,
    kind: tpl.kind,
    baseUrl: tpl.baseUrl,
    model: props.editing.model?.trim() ? props.editing.model : tpl.model,
  })
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

  // Ollama 不需要 apiKey；其它类型必填 name / baseUrl / model
  if (!name || !baseUrl || !model) {
    toast.error('请先填写 名称 / Base URL / 模型 再测试')
    return
  }
  if (kind !== 'ollama' && !apiKey) {
    toast.error('请先填写 API Key 再测试')
    return
  }

  testing.value = true
  // 立刻给视觉反馈，避免 ureq 阻塞期间用户以为没反应
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
      toast.error('测试失败', {
        description: fe.friendly,
        duration: 8000,
      })
    }
  } catch (err) {
    toast.dismiss(loadingId)
    const fe = humanizeBackendError(err)
    toast.error('测试失败', {
      description: fe.friendly,
      duration: 8000,
    })
  } finally {
    testing.value = false
  }
}

// 抽出核心拉取逻辑。`silent=true` 时不弹 toast、不报错，用于自动 debounce 调用。
async function performFetchModels(opts: { silent?: boolean } = {}): Promise<boolean> {
  const silent = opts.silent === true
  if (listingModels.value) return false
  const e = props.editing
  const kind = (e.kind || 'openai_compat').trim()
  const baseUrl = (e.baseUrl || '').trim()
  const apiKey = (e.apiKey || '').trim()
  if (!baseUrl) {
    if (!silent) toast.error('请先填写 Base URL')
    return false
  }
  if (kind !== 'ollama' && !apiKey) {
    if (!silent) toast.error('请先填写 API Key')
    return false
  }
  listingModels.value = true
  try {
    const models = await memex.llmListModels(kind, baseUrl, apiKey)
    if (!models.length) {
      availableModels.value = []
      if (!silent) toast.info('未查到模型清单')
      return false
    }
    availableModels.value = models
    if (!props.editing.model?.trim()) {
      emit('update:editing', { ...props.editing, model: models[0] })
    }
    if (!silent) {
      toast.success(`发现 ${models.length} 个可用模型`, {
        description: '可在下拉中切换或在输入框搜索',
      })
    }
    return true
  } catch (err) {
    if (!silent) {
      toast.error('拉取模型失败', {
        description: String(err),
        duration: 8000,
      })
    }
    return false
  } finally {
    listingModels.value = false
  }
}

async function fetchModels() {
  await performFetchModels()
}

// 自动拉取：当 kind / baseUrl / apiKey 变化时 debounce 500ms 后静默重拉。
// 只在三个字段都有值且 dialog 已打开时触发。
let autoRefreshTimer: ReturnType<typeof setTimeout> | null = null
watch(
  () => [props.open, props.editing.kind, props.editing.baseUrl, props.editing.apiKey] as const,
  ([isOpen, kind, baseUrl, apiKey]) => {
    if (autoRefreshTimer) {
      clearTimeout(autoRefreshTimer)
      autoRefreshTimer = null
    }
    if (!isOpen) return
    const baseTrim = (baseUrl || '').trim()
    const keyTrim = (apiKey || '').trim()
    if (!baseTrim) return
    if (kind !== 'ollama' && !keyTrim) return
    autoRefreshTimer = setTimeout(() => {
      void performFetchModels({ silent: true })
    }, 500)
  },
)

// 下拉框内的模型搜索过滤
const modelSearch = ref('')
const filteredAvailableModels = computed(() => {
  const q = modelSearch.value.trim().toLowerCase()
  if (!q) return availableModels.value
  return availableModels.value.filter((m) => m.toLowerCase().includes(q))
})
</script>

<template>
  <Dialog :open="open" @update:open="(v) => emit('update:open', v)">
    <DialogContent
      class="flex max-h-[85vh] flex-col gap-0 p-0 sm:max-w-[720px] lg:max-w-[860px]"
    >
      <!-- 固定头部：标题 + 描述。不滚动 -->
      <DialogHeader class="shrink-0 border-b px-5 py-4">
        <DialogTitle>{{ isEdit ? '编辑 Provider' : '添加 Provider' }}</DialogTitle>
        <DialogDescription>
          配置 LLM 服务商：从模板快速填充，或手动输入 Base URL 与 API Key
        </DialogDescription>
      </DialogHeader>

      <!-- 中间滚动区：弹框高度不够时只滚动这一段，不影响 header / footer -->
      <div class="min-h-0 flex-1 space-y-4 overflow-y-auto px-5 py-4">

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
              :disabled="listingModels"
              @click="fetchModels"
            >
              <RefreshCw :class="['mr-1 size-3', listingModels && 'animate-spin']" />
              {{ listingModels ? '拉取中…' : '拉取可用模型' }}
            </Button>
          </div>
          <div class="flex gap-2">
            <Input
              :model-value="editing.model"
              @update:model-value="(v) => update('model', v)"
              placeholder="deepseek-chat / gpt-4o-mini / qwen2.5:3b"
              class="h-9 flex-1 font-mono text-xs"
            />
            <Select
              v-if="availableModels.length"
              :model-value="editing.model && availableModels.includes(editing.model) ? editing.model : ''"
              @update:model-value="(v) => update('model', v)"
            >
              <SelectTrigger class="h-9 w-44 shrink-0 font-mono text-xs">
                <SelectValue placeholder="从清单选" />
              </SelectTrigger>
              <SelectContent class="max-h-72">
                <div class="sticky top-0 z-10 border-b bg-popover p-1.5">
                  <Input
                    v-model="modelSearch"
                    placeholder="搜索模型…"
                    class="h-7 font-mono text-xs"
                    autofocus
                  />
                </div>
                <SelectItem
                  v-for="m in filteredAvailableModels"
                  :key="m"
                  :value="m"
                  class="font-mono text-xs"
                >
                  {{ m }}
                </SelectItem>
                <div
                  v-if="filteredAvailableModels.length === 0"
                  class="px-2 py-3 text-center text-[11px] text-muted-foreground"
                >
                  无匹配
                </div>
              </SelectContent>
            </Select>
          </div>
          <p class="text-[10px] text-muted-foreground">
            <span v-if="listingModels">
              正在拉取可用模型…
            </span>
            <span v-else-if="availableModels.length">
              共 {{ availableModels.length }} 个可用模型，下拉选择或手动输入。修改 URL/Key 时会自动重新拉取
            </span>
            <span v-else>
              填好 Base URL 与 API Key，会自动拉取该 Provider 支持的模型清单
            </span>
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

      <!-- 固定底部：测试 / 取消 / 保存。不滚动 -->
      <DialogFooter class="shrink-0 border-t px-5 py-3">
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
