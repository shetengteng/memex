<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
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
import { Input } from '@/components/ui/input'
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
  RefreshCw,
  Server,
  Sparkles,
  Star,
  Trash2,
  XCircle,
  Zap,
} from 'lucide-vue-next'
import { toast } from 'vue-sonner'
import ProviderEditDialog from './ProviderEditDialog.vue'
import type { Provider } from '../types'
import { useMemex } from '@/composables/useMemex'
import type { LlmProvider } from '@/types'
import { humanizeBackendError } from '@/lib/utils'
import { toastBackendError } from '@/lib/toast-error'
import { useI18n } from '@/i18n'

const memex = useMemex()
const { t } = useI18n()
const providers = ref<Provider[]>([])
const loading = ref(false)
const testing = ref<Record<string, boolean>>({})
// 取默认 prompt：跟随 locale 重算（zh / en 不同）。
const defaultPrompt = computed(() => t('settings.llm.prompt_default'))
const prompt = ref<string>('')
const savingPrompt = ref(false)

// 摘要节流：通过 set_config 持久化到 config.toml 的 llm.summarize_interval_ms
// 默认 2000ms — 与 LlmConfig::default 保持一致
const summarizeIntervalMs = ref<number>(2000)
const savingInterval = ref(false)

function fromBackend(p: LlmProvider): Provider {
  return {
    id: p.id,
    name: p.name,
    kind: p.kind,
    baseUrl: p.baseUrl,
    model: p.model,
    apiKey: p.apiKey,
    enabled: p.enabled,
    isDefault: p.isDefault,
    status: (p.status || 'untested') as Provider['status'],
    latencyMs: p.latencyMs,
    updatedAt: p.updatedAt ? Date.parse(p.updatedAt) || Date.now() : Date.now(),
  }
}

async function loadProviders() {
  loading.value = true
  try {
    const list = await memex.llmProviderList()
    providers.value = list.map(fromBackend)
  } catch (e) {
    toastBackendError(t('settings.llm.toast.list_failed'), e)
  } finally {
    loading.value = false
  }
}

async function loadPrompt() {
  try {
    const v = await memex.getConfig('llm.prompt_template')
    prompt.value = v || defaultPrompt.value
  } catch {
    prompt.value = defaultPrompt.value
  }
}

async function loadSummarizeInterval() {
  try {
    const v = await memex.getConfig('llm.summarize_interval_ms')
    if (v) {
      const n = Number.parseInt(v, 10)
      if (!Number.isNaN(n) && n >= 0) summarizeIntervalMs.value = n
    }
  } catch {
    /* ignore — keep default 2000 */
  }
}

async function saveSummarizeInterval() {
  if (savingInterval.value) return
  const v = Math.max(0, Math.floor(summarizeIntervalMs.value || 0))
  savingInterval.value = true
  try {
    await memex.setConfig('llm.summarize_interval_ms', String(v))
    summarizeIntervalMs.value = v
    toast.success(t('settings.llm.throttle_toast', { ms: v }))
  } catch (err) {
    toastBackendError(t('settings.llm.toast.save_failed'), err)
  } finally {
    savingInterval.value = false
  }
}

onMounted(async () => {
  await Promise.all([loadProviders(), loadPrompt(), loadSummarizeInterval()])
})

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

async function saveProvider() {
  const e = editing.value
  if (!e.name?.trim() || !e.baseUrl?.trim() || !e.model?.trim()) return
  const payload = {
    id: e.id || `p-${Date.now()}`,
    name: e.name!,
    kind: (e.kind || 'openai_compat') as string,
    baseUrl: e.baseUrl!,
    model: e.model!,
    apiKey: e.apiKey ?? '',
    enabled: e.enabled ?? true,
    isDefault: e.isDefault ?? false,
  }
  try {
    const saved = await memex.llmProviderUpsert(payload)
    const idx = providers.value.findIndex((p) => p.id === saved.id)
    const next = fromBackend(saved)
    if (idx >= 0) providers.value[idx] = next
    else providers.value.push(next)
    // 如果当前保存为 default，需要 refresh 其它项的 default flag
    if (next.isDefault) {
      providers.value = providers.value.map((p) => ({ ...p, isDefault: p.id === next.id }))
    }
    editOpen.value = false
    toast.success(t('settings.llm.toast.provider_saved', { name: next.name }))
  } catch (err) {
    toastBackendError(t('settings.llm.toast.save_failed'), err)
  }
}

async function setDefault(id: string) {
  const target = providers.value.find((p) => p.id === id)
  if (!target) return
  try {
    const saved = await memex.llmProviderUpsert({
      id: target.id,
      name: target.name,
      kind: target.kind,
      baseUrl: target.baseUrl,
      model: target.model,
      apiKey: target.apiKey,
      enabled: target.enabled,
      isDefault: true,
    })
    providers.value = providers.value.map((p) => ({ ...p, isDefault: p.id === saved.id }))
    toast.success(t('settings.llm.toast.set_default', { name: saved.name }))
  } catch (err) {
    toastBackendError(t('settings.llm.toast.set_default_failed'), err)
  }
}

async function removeProvider(id: string) {
  try {
    await memex.llmProviderDelete(id)
    providers.value = providers.value.filter((p) => p.id !== id)
    toast.success(t('settings.llm.toast.deleted'))
  } catch (err) {
    toastBackendError(t('settings.llm.toast.delete_failed'), err)
  }
}

async function toggleEnabled(p: Provider, value: boolean | string) {
  const enabled = value === true
  try {
    const saved = await memex.llmProviderUpsert({
      id: p.id,
      name: p.name,
      kind: p.kind,
      baseUrl: p.baseUrl,
      model: p.model,
      apiKey: p.apiKey,
      enabled,
      isDefault: p.isDefault,
    })
    const idx = providers.value.findIndex((x) => x.id === p.id)
    if (idx >= 0) providers.value[idx] = fromBackend(saved)
  } catch (err) {
    toastBackendError(t('settings.llm.toast.toggle_failed'), err)
  }
}

async function testProvider(p: Provider) {
  if (testing.value[p.id]) return
  testing.value[p.id] = true
  // 立刻给视觉反馈：HTTP 调用最长可能要 30s（特别是 ollama 服务连不上时），
  // 旧版只切按钮 spin 图标用户经常没看到，会以为"按了没反应"。
  const loadingId = toast.loading(t('settings.llm.toast.testing', { name: p.name }))
  try {
    const r = await memex.llmProviderTest(p.id)
    const idx = providers.value.findIndex((x) => x.id === p.id)
    if (idx >= 0) {
      providers.value[idx] = {
        ...providers.value[idx],
        status: r.ok ? (p.kind === 'ollama' ? 'local' : 'ok') : 'error',
        latencyMs: r.latencyMs,
      }
    }
    toast.dismiss(loadingId)
    if (r.ok) toast.success(t('settings.llm.toast.test_ok', { name: p.name, ms: r.latencyMs }))
    else {
      const fe = humanizeBackendError(r.error || 'unknown')
      toast.error(t('settings.llm.toast.test_failed_fmt', { name: p.name }), {
        description: fe.friendly,
        duration: 8000,
      })
    }
  } catch (err) {
    toast.dismiss(loadingId)
    const fe = humanizeBackendError(err)
    toast.error(t('settings.llm.toast.test_failed'), { description: fe.friendly, duration: 8000 })
  } finally {
    testing.value[p.id] = false
  }
}

async function savePromptTemplate() {
  savingPrompt.value = true
  try {
    await memex.setConfig('llm.prompt_template', prompt.value)
    toast.success(t('settings.llm.toast.template_saved'))
  } catch (err) {
    toastBackendError(t('settings.llm.toast.save_failed'), err)
  } finally {
    savingPrompt.value = false
  }
}

function resetPromptTemplate() {
  prompt.value = defaultPrompt.value
}

function iconForKind(kind: string) {
  const k = kind.toLowerCase()
  if (k.includes('ollama')) return Server
  if (k.includes('anthropic') || k.includes('claude')) return Sparkles
  return Cloud
}

function statusLabel(s: Provider['status']) {
  if (s === 'ok') return t('settings.llm.status.ok')
  if (s === 'error') return t('settings.llm.status.error')
  if (s === 'local') return t('settings.llm.status.local')
  return t('settings.llm.status.untested')
}

function kindLabel(k: Provider['kind']) {
  if (k === 'openai_compat') return t('settings.llm.kind.openai_compat')
  if (k === 'anthropic') return t('settings.llm.kind.anthropic')
  if (k === 'ollama') return t('settings.llm.kind.ollama')
  return k
}
</script>

<template>
  <div class="space-y-4">
    <!-- Fallback Chain -->
    <Card>
      <CardHeader class="pb-2">
        <CardTitle class="text-base">{{ t('settings.llm.fallback_title') }}</CardTitle>
        <CardDescription class="text-xs">
          {{ t('settings.llm.fallback_desc') }}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div
          v-if="fallbackChain.length === 0"
          class="flex items-start gap-2 rounded-md bg-muted/40 p-3"
        >
          <AlertTriangle class="mt-0.5 size-4 shrink-0 text-amber-500" />
          <p class="text-xs text-muted-foreground">
            {{ t('settings.llm.fallback_empty') }}
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
              {{ t('settings.llm.fallback_single_warn') }}
            </p>
          </div>
        </div>
      </CardContent>
    </Card>

    <!-- Provider List -->
    <Card>
      <CardHeader class="flex flex-row items-center justify-between pb-3">
        <div>
          <CardTitle class="text-base">{{ t('settings.llm.list_title') }}</CardTitle>
          <CardDescription class="text-xs">
            {{ t('settings.llm.list_count', { total: providers.length, enabled: enabledCount }) }}
          </CardDescription>
        </div>
        <div class="flex items-center gap-2">
          <Button size="sm" variant="ghost" :disabled="loading" @click="loadProviders">
            <RefreshCw :class="['mr-1.5 size-3.5', loading && 'animate-spin']" />
            {{ t('settings.llm.refresh') }}
          </Button>
          <Button size="sm" @click="openAdd">
            <Plus class="mr-1.5 size-3.5" />
            {{ t('settings.llm.add_provider') }}
          </Button>
        </div>
      </CardHeader>
      <CardContent class="space-y-2">
        <div
          v-if="providers.length === 0"
          class="rounded-lg border border-dashed py-8 text-center text-xs text-muted-foreground"
        >
          {{ loading ? t('settings.llm.list_loading') : t('settings.llm.list_empty') }}
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
              <Badge v-if="p.isDefault" variant="secondary" class="text-[10px]">{{ t('settings.llm.default_badge') }}</Badge>
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
                :disabled="testing[p.id]"
                @click="testProvider(p)"
              >
                <RefreshCw v-if="testing[p.id]" class="size-3.5 animate-spin" />
                <Zap v-else class="size-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="top">{{ t('settings.llm.test_tooltip') }}</TooltipContent>
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

    <!-- 摘要节流：每条会话摘要（L2）之间的间隔，避免 Ollama 一次性把 GPU 拉满。 -->
    <Card>
      <CardHeader class="pb-2">
        <CardTitle class="text-base">{{ t('settings.llm.throttle_title') }}</CardTitle>
        <CardDescription class="text-xs">
          {{ t('settings.llm.throttle_desc') }}
        </CardDescription>
      </CardHeader>
      <CardContent class="flex items-center gap-3">
        <Input
          v-model.number="summarizeIntervalMs"
          type="number"
          min="0"
          step="500"
          class="h-9 w-32 font-mono text-sm"
        />
        <span class="text-xs text-muted-foreground">{{ t('settings.llm.throttle_unit') }}</span>
        <Button
          size="sm"
          variant="outline"
          class="ml-auto"
          :disabled="savingInterval"
          @click="saveSummarizeInterval"
        >
          {{ savingInterval ? t('settings.llm.throttle_saving') : t('settings.llm.throttle_save') }}
        </Button>
      </CardContent>
    </Card>

    <!-- Prompt Template -->
    <Card>
      <CardHeader>
        <CardDescription>{{ t('settings.llm.prompt_section') }}</CardDescription>
        <CardTitle class="text-base">{{ t('settings.llm.prompt_title') }}</CardTitle>
        <CardDescription class="text-xs">
          {{ t('settings.llm.prompt_desc') }}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <Textarea v-model="prompt" rows="6" />
      </CardContent>
      <CardFooter class="gap-2">
        <Button size="sm" variant="outline" @click="resetPromptTemplate">{{ t('settings.llm.prompt_reset') }}</Button>
        <Button size="sm" class="ml-auto" :disabled="savingPrompt" @click="savePromptTemplate">
          {{ savingPrompt ? t('settings.llm.prompt_saving') : t('settings.llm.prompt_save') }}
        </Button>
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
