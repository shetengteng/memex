<script setup lang="ts">
import { computed, ref } from 'vue'
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
} from '@lucide/vue'
import type { Provider } from '../types'

const props = defineProps<{ open: boolean; editing: Partial<Provider> }>()
const emit = defineEmits<{
  'update:open': [boolean]
  'update:editing': [Partial<Provider>]
  save: []
}>()

const isEdit = computed(() => Boolean(props.editing.id))
const showApiKey = ref(false)

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
</script>

<template>
  <Dialog :open="open" @update:open="(v) => emit('update:open', v)">
    <DialogContent class="sm:max-w-[720px] lg:max-w-[860px]">
      <DialogHeader>
        <DialogTitle>{{ isEdit ? '编辑 Provider' : '添加 Provider' }}</DialogTitle>
        <DialogDescription>
          配置 LLM 服务商：从模板快速填充，或手动输入 Base URL 与 API Key
        </DialogDescription>
      </DialogHeader>

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
            <SelectTrigger class="h-9"><SelectValue /></SelectTrigger>
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
            <Button type="button" variant="ghost" size="sm" class="h-7 px-2 text-[11px]">
              <RefreshCw class="mr-1 size-3" />
              拉取可用模型
            </Button>
          </div>
          <Input
            :model-value="editing.model"
            @update:model-value="(v) => update('model', v)"
            placeholder="deepseek-chat / gpt-4o-mini / qwen2.5:3b"
            class="h-9 font-mono text-xs"
          />
          <p class="text-[10px] text-muted-foreground">
            填好 Base URL 与 API Key 后，可点上方按钮自动拉取该 Provider 支持的模型清单
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

      <DialogFooter>
        <Button variant="outline" class="mr-auto">
          <Zap class="mr-1.5 size-3.5" />
          测试连接
        </Button>
        <Button variant="outline" @click="emit('update:open', false)">取消</Button>
        <Button @click="emit('save')">保存</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
