<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { Textarea } from '@/components/ui/textarea'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import {
  AlertTriangle,
  ArrowRight,
  Bot,
  CheckCircle2,
  Cloud,
  Edit2,
  Plus,
  Server,
  Sparkles,
  Star,
  Trash2,
  XCircle,
  Zap,
} from '@lucide/vue'
import ProviderEditDialog from './ProviderEditDialog.vue'
import type { Provider } from '../types'

const providers = ref<Provider[]>([
  {
    id: 'p-ollama',
    name: 'Ollama Local',
    kind: 'ollama',
    baseUrl: 'http://127.0.0.1:11434',
    model: 'qwen2.5:7b',
    apiKey: '',
    enabled: true,
    isDefault: true,
    status: 'local',
    latencyMs: 320,
    updatedAt: Date.now() - 1000 * 60 * 60,
  },
  {
    id: 'p-anthropic',
    name: 'Anthropic Claude',
    kind: 'anthropic',
    baseUrl: 'https://api.anthropic.com',
    model: 'claude-3-5-haiku-latest',
    apiKey: 'sk-ant-***',
    enabled: true,
    isDefault: false,
    status: 'ok',
    latencyMs: 1180,
    updatedAt: Date.now() - 1000 * 60 * 60 * 5,
  },
  {
    id: 'p-deepseek',
    name: 'DeepSeek',
    kind: 'openai_compat',
    baseUrl: 'https://api.deepseek.com/v1',
    model: 'deepseek-chat',
    apiKey: '',
    enabled: false,
    isDefault: false,
    status: 'untested',
    latencyMs: null,
    updatedAt: Date.now() - 1000 * 60 * 60 * 24,
  },
])

const enabledCount = computed(() => providers.value.filter((p) => p.enabled).length)

const fallbackChain = computed<Provider[]>(() =>
  [...providers.value]
    .filter((p) => p.enabled)
    .sort((a, b) => {
      if (a.isDefault !== b.isDefault) return a.isDefault ? -1 : 1
      return b.updatedAt - a.updatedAt
    }),
)

const editOpen = ref(false)
const editing = ref<Partial<Provider>>({ kind: 'openai_compat', enabled: true })

function openAdd() {
  editing.value = { kind: 'openai_compat', enabled: true, isDefault: false }
  editOpen.value = true
}

function openEdit(p: Provider) {
  editing.value = { ...p }
  editOpen.value = true
}

function saveProvider() {
  const e = editing.value
  if (!e.name?.trim() || !e.baseUrl?.trim() || !e.model?.trim()) return
  if (e.id) {
    const idx = providers.value.findIndex((p) => p.id === e.id)
    if (idx >= 0)
      providers.value[idx] = {
        ...providers.value[idx],
        ...(e as Provider),
        updatedAt: Date.now(),
      }
  } else {
    providers.value.push({
      id: `p-${Date.now()}`,
      name: e.name!,
      kind: (e.kind ?? 'openai_compat') as Provider['kind'],
      baseUrl: e.baseUrl!,
      model: e.model!,
      apiKey: e.apiKey ?? '',
      enabled: e.enabled ?? true,
      isDefault: e.isDefault ?? false,
      status: 'untested',
      latencyMs: null,
      updatedAt: Date.now(),
    })
  }
  editOpen.value = false
}

function setDefault(id: string) {
  providers.value = providers.value.map((p) => ({ ...p, isDefault: p.id === id }))
}

function removeProvider(id: string) {
  providers.value = providers.value.filter((p) => p.id !== id)
}

function toggleEnabled(p: Provider, value: boolean | string) {
  const idx = providers.value.findIndex((x) => x.id === p.id)
  if (idx >= 0) providers.value[idx] = { ...providers.value[idx], enabled: value === true }
}

function iconForKind(kind: string) {
  const k = kind.toLowerCase()
  if (k.includes('ollama')) return Server
  if (k.includes('anthropic') || k.includes('claude')) return Sparkles
  return Cloud
}

function statusLabel(s: Provider['status']) {
  if (s === 'ok') return '在线'
  if (s === 'error') return '错误'
  if (s === 'local') return '本地'
  return '未测试'
}

function kindLabel(k: Provider['kind']) {
  if (k === 'openai_compat') return 'OpenAI 兼容'
  if (k === 'anthropic') return 'Anthropic'
  if (k === 'ollama') return 'Ollama'
  return k
}
</script>

<template>
  <div class="space-y-4">
    <!-- Fallback Chain -->
    <Card>
      <CardHeader class="pb-2">
        <CardTitle class="text-base">默认与备用链路</CardTitle>
        <CardDescription class="text-xs">
          当默认 Provider 失败时，按以下顺序自动切换备用
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div
          v-if="fallbackChain.length === 0"
          class="flex items-start gap-2 rounded-md bg-muted/40 p-3"
        >
          <AlertTriangle class="mt-0.5 size-4 shrink-0 text-amber-500" />
          <p class="text-xs text-muted-foreground">
            暂未启用任何 Provider，请在下方添加并启用至少一个
          </p>
        </div>
        <div v-else class="space-y-3">
          <div class="flex flex-wrap items-center gap-2">
            <template v-for="(p, idx) in fallbackChain" :key="p.id">
              <Badge
                :variant="idx === 0 ? 'default' : 'outline'"
                class="gap-1.5 px-3 py-1.5"
              >
                <component :is="iconForKind(p.kind)" class="size-3" />
                <Star v-if="p.isDefault" class="size-3 fill-current" />
                <span class="max-w-[160px] truncate">{{ p.name }}</span>
              </Badge>
              <ArrowRight
                v-if="idx < fallbackChain.length - 1"
                class="size-3.5 shrink-0 text-muted-foreground"
              />
            </template>
          </div>
          <div
            v-if="fallbackChain.length === 1"
            class="flex items-start gap-2 rounded-md bg-amber-500/5 p-3 ring-1 ring-amber-500/20"
          >
            <AlertTriangle class="mt-0.5 size-4 shrink-0 text-amber-500" />
            <p class="text-xs text-muted-foreground">
              只有一个 Provider，建议至少再启用一个备用，避免单点失效
            </p>
          </div>
        </div>
      </CardContent>
    </Card>

    <!-- Provider List -->
    <Card>
      <CardHeader class="flex flex-row items-center justify-between pb-3">
        <div>
          <CardTitle class="text-base">Provider 列表</CardTitle>
          <CardDescription class="text-xs">
            共 {{ providers.length }} 个，已启用 {{ enabledCount }}
          </CardDescription>
        </div>
        <Button size="sm" @click="openAdd">
          <Plus class="mr-1.5 size-3.5" />
          添加 Provider
        </Button>
      </CardHeader>
      <CardContent class="space-y-2">
        <div
          v-if="providers.length === 0"
          class="rounded-lg border border-dashed py-8 text-center text-xs text-muted-foreground"
        >
          暂无 Provider，点击右上角添加
        </div>
        <div
          v-for="p in providers"
          :key="p.id"
          class="flex items-center gap-3 rounded-lg border px-3 py-3 transition-colors hover:bg-accent/40"
        >
          <Switch
            :model-value="p.enabled"
            @update:model-value="(v) => toggleEnabled(p, v)"
          />
          <div class="min-w-0 flex-1 space-y-0.5">
            <div class="flex items-center gap-2">
              <span class="text-[13px] font-medium">{{ p.name }}</span>
              <Badge v-if="p.isDefault" variant="secondary" class="text-[10px]">默认</Badge>
              <Badge variant="outline" class="text-[10px]">{{ kindLabel(p.kind) }}</Badge>
            </div>
            <Tooltip>
              <TooltipTrigger as-child>
                <div class="cursor-default truncate font-mono text-[11px] text-muted-foreground">
                  {{ p.baseUrl }} · {{ p.model }}
                </div>
              </TooltipTrigger>
              <TooltipContent
                side="top"
                align="start"
                class="max-w-[80vw] break-all font-mono"
              >
                {{ p.baseUrl }}
              </TooltipContent>
            </Tooltip>
          </div>
          <Badge
            v-if="p.enabled"
            variant="outline"
            class="gap-1 text-[10px]"
            :class="{
              'border-emerald-500/30 text-emerald-500': p.status === 'ok',
              'border-sky-500/30 text-sky-500': p.status === 'local',
              'border-rose-500/30 text-rose-500': p.status === 'error',
            }"
          >
            <CheckCircle2 v-if="p.status === 'ok' || p.status === 'local'" class="size-3" />
            <XCircle v-else-if="p.status === 'error'" class="size-3" />
            {{ statusLabel(p.status) }}
            <span v-if="p.latencyMs" class="opacity-70">· {{ p.latencyMs }}ms</span>
          </Badge>
          <Tooltip>
            <TooltipTrigger as-child>
              <Button
                variant="ghost"
                size="icon"
                class="size-8 text-muted-foreground hover:text-sky-500"
              >
                <Zap class="size-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="top">测试连接</TooltipContent>
          </Tooltip>
          <Button
            variant="ghost"
            size="icon"
            class="size-8"
            :class="
              p.isDefault
                ? 'text-amber-500 hover:text-amber-600'
                : 'text-muted-foreground hover:text-amber-500'
            "
            :disabled="p.isDefault"
            @click="setDefault(p.id)"
          >
            <Star class="size-3.5" :class="p.isDefault ? 'fill-current' : ''" />
          </Button>
          <Button variant="ghost" size="icon" class="size-8" @click="openEdit(p)">
            <Edit2 class="size-3.5" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            class="size-8 text-rose-500 hover:text-rose-600"
            @click="removeProvider(p.id)"
          >
            <Trash2 class="size-3.5" />
          </Button>
        </div>
      </CardContent>
    </Card>

    <!-- Prompt Template -->
    <Card>
      <CardHeader>
        <CardDescription>提示词</CardDescription>
        <CardTitle class="text-base">L3 叙述模板</CardTitle>
      </CardHeader>
      <CardContent>
        <Textarea
          rows="6"
          model-value="生成一句话主旨、用户的真实意图、1~3 条关键决策、1~3 条具体的下一步。风格：精炼，不要废话。"
        />
      </CardContent>
      <CardFooter class="gap-2">
        <Button size="sm" variant="outline">恢复默认</Button>
        <Button size="sm" class="ml-auto">保存模板</Button>
      </CardFooter>
    </Card>

    <ProviderEditDialog
      :open="editOpen"
      :editing="editing"
      @update:open="(v) => (editOpen = v)"
      @update:editing="(v) => (editing = v)"
      @save="saveProvider"
    />
  </div>
</template>
