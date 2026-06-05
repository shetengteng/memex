<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useMemex } from '@/composables/useMemex'
import { useI18n } from '@/i18n'
import { formatNumber, timeAgo } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent } from '@/components/ui/card'
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from '@/components/ui/collapsible'
import {
  RefreshCw,
  Power,
  ChevronDown,
  Database,
  Plug,
  Brain,
  CheckCircle2,
  AlertTriangle,
  XCircle,
  CircleDot,
} from 'lucide-vue-next'
import type { Stats, DaemonStatus } from '@/types'
import IdeIcon from '@/components/IdeIcon.vue'

const { t } = useI18n()
const { getStats, getConfig, daemonStatus, daemonRestart } = useMemex()

type Tone = 'success' | 'warning' | 'error' | 'muted'

const stats = ref<Stats | null>(null)
const loading = ref(true)
const statsError = ref('')
const daemon = ref<DaemonStatus | null>(null)
const restarting = ref(false)
const restartError = ref('')

const adapterDefs: { key: string; label: string }[] = [
  { key: 'claude_code', label: 'Claude Code' },
  { key: 'cursor', label: 'Cursor' },
  { key: 'codex', label: 'Codex' },
  { key: 'opencode', label: 'OpenCode' },
]

const adapterEnabled = ref<Record<string, boolean>>({})
const llmOllama = ref<boolean>(false)

// 折叠状态：Database/Adapters/LLM 默认全展开，让用户一打开 Status 就看到全部数据；
// 不需要时手动折，保留状态。Daemon Card 不可折叠（永远是 hero）
const openDatabase = ref(true)
const openAdapters = ref(true)
const openLlm = ref(true)

async function probeDaemon() {
  try {
    daemon.value = await daemonStatus()
  } catch {
    daemon.value = { running: false, pid: null, port: null, http_ok: false, started_at: null }
  }
}

async function handleRestart() {
  restarting.value = true
  restartError.value = ''
  try {
    daemon.value = await daemonRestart()
  } catch (e: unknown) {
    restartError.value = e instanceof Error ? e.message : String(e)
  } finally {
    restarting.value = false
  }
}

onMounted(async () => {
  await Promise.allSettled([
    getStats().then((s) => (stats.value = s)).catch((e) => { statsError.value = String(e) }),
    probeDaemon(),
    ...adapterDefs.map(async (a) => {
      try {
        const v = await getConfig(`adapter.${a.key}.enabled`)
        adapterEnabled.value[a.key] = v === null ? true : v === 'true'
      } catch {
        adapterEnabled.value[a.key] = true
      }
    }),
    getConfig('llm.ollama_enabled').then((v) => { llmOllama.value = v === 'true' }).catch(() => {}),
  ])
  loading.value = false
})

function refresh() {
  loading.value = true
  statsError.value = ''
  Promise.allSettled([
    getStats().then((s) => (stats.value = s)).catch((e) => { statsError.value = String(e) }),
    probeDaemon(),
  ]).finally(() => {
    loading.value = false
  })
}

// Daemon 主状态判定（hero card 用）
interface DaemonView {
  tone: Tone
  primaryLabel: string
  secondaryLabel?: string
  meta?: string
  showRestart: boolean
}

const daemonView = computed<DaemonView>(() => {
  const d = daemon.value
  if (!d) {
    return { tone: 'muted', primaryLabel: t('status.daemon.checking'), showRestart: false }
  }
  if (d.running && d.http_ok) {
    return {
      tone: 'success',
      primaryLabel: t('common.running'),
      secondaryLabel: d.port ? t('status.daemon.health_endpoint', { port: d.port }) : undefined,
      meta: d.pid && d.started_at
        ? `PID ${d.pid} · ${t('status.daemon.started_at', { time: timeAgo(d.started_at) })}`
        : d.pid ? `PID ${d.pid}` : undefined,
      showRestart: false,  // success 时不诱导操作，悬浮在 Card 右上角的 ghost 按钮已经够用
    }
  }
  if (d.running && !d.http_ok) {
    return {
      tone: 'warning',
      primaryLabel: t('status.daemon.starting_short'),
      secondaryLabel: t('status.daemon.starting_hint', { pid: d.pid ?? '?' }),
      showRestart: true,
    }
  }
  return {
    tone: 'error',
    primaryLabel: t('status.daemon.offline_short'),
    secondaryLabel: t('status.daemon.offline_hint'),
    showRestart: true,
  }
})

const activeAdapterCount = computed(
  () => adapterDefs.filter((a) => adapterEnabled.value[a.key]).length,
)

const llmProviderLabel = computed(() =>
  stats.value?.llm_provider ?? t('status.card.llm_provider_none'),
)

const llmProviderTone = computed<Tone>(() => {
  if (stats.value?.llm_provider) return 'success'
  if (llmOllama.value) return 'warning'
  return 'muted'
})

const dotClass: Record<Tone, string> = {
  success: 'bg-success',
  warning: 'bg-warning',
  error: 'bg-destructive',
  muted: 'bg-muted-foreground/40',
}

const toneTextClass: Record<Tone, string> = {
  success: 'text-success',
  warning: 'text-warning',
  error: 'text-destructive',
  muted: 'text-muted-foreground',
}

const toneIconClass: Record<Tone, string> = {
  success: 'text-success',
  warning: 'text-warning',
  error: 'text-destructive',
  muted: 'text-muted-foreground',
}

const heroBgClass: Record<Tone, string> = {
  success: 'border-success/30 bg-success/5',
  warning: 'border-warning/30 bg-warning/5',
  error: 'border-destructive/30 bg-destructive/5',
  muted: 'border-border bg-muted/30',
}

const heroIcon = computed(() => {
  switch (daemonView.value.tone) {
    case 'success': return CheckCircle2
    case 'warning': return AlertTriangle
    case 'error': return XCircle
    default: return CircleDot
  }
})
</script>

<template>
  <div class="h-full overflow-y-auto px-3 py-3">
    <header class="mb-3 flex items-baseline justify-between px-1">
      <h2 class="text-base font-semibold">{{ t('status.title') }}</h2>
      <Button variant="ghost" size="sm" :disabled="loading" class="h-7 gap-1.5 text-xs" @click="refresh">
        <RefreshCw class="h-3 w-3" :class="{ 'animate-spin': loading }" />
        {{ loading ? t('common.refreshing') : t('common.refresh') }}
      </Button>
    </header>

    <div v-if="loading && daemon === null" class="px-1 text-sm text-muted-foreground">{{ t('common.loading') }}</div>

    <div v-else class="space-y-3">
      <!-- Card 1: Daemon Hero — 永远展开，主状态最显眼 -->
      <Card :class="`transition-colors ${heroBgClass[daemonView.tone]}`">
        <CardContent class="flex items-start gap-3 py-3">
          <component
            :is="heroIcon"
            class="mt-0.5 h-6 w-6 shrink-0"
            :class="toneIconClass[daemonView.tone]"
          />
          <div class="min-w-0 flex-1">
            <div class="flex items-baseline gap-2">
              <span class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                {{ t('status.card.daemon') }}
              </span>
            </div>
            <div class="mt-0.5 flex items-baseline gap-2">
              <span class="text-lg font-semibold" :class="toneTextClass[daemonView.tone]">
                {{ daemonView.primaryLabel }}
              </span>
              <span v-if="daemonView.secondaryLabel" class="mono text-xs text-muted-foreground">
                {{ daemonView.secondaryLabel }}
              </span>
            </div>
            <p v-if="daemonView.meta" class="mt-0.5 text-[11px] text-muted-foreground">{{ daemonView.meta }}</p>
            <p v-if="restartError" class="mt-1 text-xs text-destructive">
              {{ t('status.restart.fail') }}: {{ restartError }}
            </p>
          </div>
          <!-- 重启按钮：success 时低调（ghost）、warning/error 时主动（outline）-->
          <Button
            :variant="daemonView.showRestart ? 'outline' : 'ghost'"
            size="sm"
            class="h-7 shrink-0 gap-1 text-xs"
            :disabled="restarting"
            @click="handleRestart"
          >
            <Power class="h-3 w-3" :class="{ 'animate-pulse': restarting }" />
            {{ restarting ? t('status.restart.in_progress') : t('status.restart.button') }}
          </Button>
        </CardContent>
      </Card>

      <!-- Card 2: Database KPI（可折叠） -->
      <Collapsible v-model:open="openDatabase">
        <Card>
          <CollapsibleTrigger
            class="group flex w-full cursor-pointer items-center justify-between gap-2 rounded-lg px-4 py-2.5 text-left transition-colors hover:bg-muted/30"
          >
            <span class="flex items-center gap-1.5">
              <Database class="h-3 w-3 text-muted-foreground" />
              <span class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                {{ t('status.card.database') }}
              </span>
              <Badge v-if="!stats" variant="secondary" class="ml-1 text-[10px]">
                {{ loading ? '...' : (statsError || t('status.db.not_initialized')) }}
              </Badge>
              <Badge v-else-if="!stats.db_exists" variant="destructive" class="ml-1 text-[10px]">
                {{ t('status.db.not_initialized') }}
              </Badge>
              <Badge v-else variant="secondary" class="ml-1 text-[10px]">
                {{ formatNumber(stats.sessions) }} {{ t('status.kpi.sessions') }}
              </Badge>
            </span>
            <ChevronDown
              class="h-4 w-4 text-muted-foreground transition-transform duration-200"
              :class="openDatabase ? 'rotate-180' : ''"
            />
          </CollapsibleTrigger>
          <CollapsibleContent class="overflow-hidden data-[state=closed]:animate-collapsible-up data-[state=open]:animate-collapsible-down">
            <div v-if="!stats" class="px-4 pb-3 pt-1 text-xs text-muted-foreground">
              {{ loading ? t('common.loading') : statsError }}
            </div>
            <template v-else>
              <div class="grid grid-cols-4 gap-2 px-4 pb-3 pt-1">
                <div class="rounded-md border border-border bg-muted/20 p-2">
                  <div class="text-[10px] font-medium uppercase tracking-wider text-muted-foreground">{{ t('status.kpi.sessions') }}</div>
                  <div class="mt-0.5 text-base font-semibold tabular-nums">{{ formatNumber(stats.sessions) }}</div>
                </div>
                <div class="rounded-md border border-border bg-muted/20 p-2">
                  <div class="text-[10px] font-medium uppercase tracking-wider text-muted-foreground">{{ t('status.kpi.messages') }}</div>
                  <div class="mt-0.5 text-base font-semibold tabular-nums">{{ formatNumber(stats.messages) }}</div>
                </div>
                <div class="rounded-md border border-border bg-muted/20 p-2">
                  <div class="text-[10px] font-medium uppercase tracking-wider text-muted-foreground">{{ t('status.kpi.chunks') }}</div>
                  <div class="mt-0.5 text-base font-semibold tabular-nums">{{ formatNumber(stats.chunks) }}</div>
                </div>
                <div class="rounded-md border border-border bg-muted/20 p-2">
                  <div class="text-[10px] font-medium uppercase tracking-wider text-muted-foreground">{{ t('status.kpi.summaries') }}</div>
                  <div class="mt-0.5 text-base font-semibold tabular-nums">{{ formatNumber(stats.summaries) }}</div>
                </div>
              </div>
              <p v-if="stats.chunks > 0" class="px-4 pb-2 text-[10px] text-muted-foreground">
                {{ t('status.index.fts_ready') }}
              </p>
              <p v-else-if="stats.db_exists" class="px-4 pb-2 text-[10px] text-muted-foreground">
                {{ t('status.index.empty') }}
              </p>
            </template>
          </CollapsibleContent>
        </Card>
      </Collapsible>

      <!-- Card 3: Adapters 2x2 grid（可折叠） -->
      <Collapsible v-model:open="openAdapters">
        <Card>
          <CollapsibleTrigger
            class="group flex w-full cursor-pointer items-center justify-between gap-2 rounded-lg px-4 py-2.5 text-left transition-colors hover:bg-muted/30"
          >
            <span class="flex items-center gap-1.5">
              <Plug class="h-3 w-3 text-muted-foreground" />
              <span class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                {{ t('status.adapters') }}
              </span>
              <Badge variant="secondary" class="ml-1 text-[10px]">
                {{ t('status.card.adapters_summary', { active: activeAdapterCount, total: adapterDefs.length }) }}
              </Badge>
            </span>
            <ChevronDown
              class="h-4 w-4 text-muted-foreground transition-transform duration-200"
              :class="openAdapters ? 'rotate-180' : ''"
            />
          </CollapsibleTrigger>
          <CollapsibleContent class="overflow-hidden data-[state=closed]:animate-collapsible-up data-[state=open]:animate-collapsible-down">
            <div class="grid grid-cols-2 gap-2 px-4 pb-3 pt-1">
              <div
                v-for="a in adapterDefs"
                :key="a.key"
                class="flex items-center gap-2 rounded-md border border-border bg-muted/20 px-2.5 py-1.5"
              >
                <IdeIcon :source="a.key" class="h-4 w-4 shrink-0" :class="adapterEnabled[a.key] ? '' : 'opacity-40 grayscale'" />
                <span class="flex-1 truncate text-xs">{{ a.label }}</span>
                <span class="text-[10px]" :class="adapterEnabled[a.key] ? toneTextClass.success : toneTextClass.muted">
                  {{ adapterEnabled[a.key] ? t('common.enabled') : t('common.disabled') }}
                </span>
              </div>
            </div>
          </CollapsibleContent>
        </Card>
      </Collapsible>

      <!-- Card 4: LLM 信号（可折叠） -->
      <Collapsible v-model:open="openLlm">
        <Card>
          <CollapsibleTrigger
            class="group flex w-full cursor-pointer items-center justify-between gap-2 rounded-lg px-4 py-2.5 text-left transition-colors hover:bg-muted/30"
          >
            <span class="flex items-center gap-1.5">
              <Brain class="h-3 w-3 text-muted-foreground" />
              <span class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                {{ t('status.llm') }}
              </span>
              <Badge :variant="llmProviderTone === 'success' ? 'default' : 'secondary'" class="ml-1 text-[10px]">
                {{ llmProviderLabel }}
              </Badge>
            </span>
            <ChevronDown
              class="h-4 w-4 text-muted-foreground transition-transform duration-200"
              :class="openLlm ? 'rotate-180' : ''"
            />
          </CollapsibleTrigger>
          <CollapsibleContent class="overflow-hidden data-[state=closed]:animate-collapsible-up data-[state=open]:animate-collapsible-down">
            <div class="flex flex-col gap-1 px-4 pb-3 pt-1">
              <!-- 当前 provider -->
              <div class="flex items-center justify-between rounded-md border border-border bg-muted/20 px-2.5 py-1.5">
                <span class="flex items-center gap-2 text-xs">
                  <span class="inline-block h-2 w-2 shrink-0 rounded-full" :class="dotClass[llmProviderTone]" />
                  {{ t('status.llm.active') }}
                </span>
                <span class="text-xs" :class="toneTextClass[llmProviderTone]">
                  {{ llmProviderLabel }}
                </span>
              </div>
              <!-- Ollama 开关 -->
              <div class="flex items-center justify-between rounded-md border border-border bg-muted/20 px-2.5 py-1.5">
                <span class="flex items-center gap-2 text-xs">
                  <span class="inline-block h-2 w-2 shrink-0 rounded-full" :class="llmOllama ? dotClass.success : dotClass.muted" />
                  Ollama
                </span>
                <span class="text-xs" :class="llmOllama ? toneTextClass.success : toneTextClass.muted">
                  {{ llmOllama ? t('common.enabled') : t('common.disabled') }}
                </span>
              </div>

              <!-- 摘要计数 -->
              <p v-if="stats?.llm_provider" class="mt-1 px-1 text-[10px] text-muted-foreground">
                {{ t('status.llm.active_hint', {
                  sessions: formatNumber(stats.summaries),
                  chunks: formatNumber(stats.chunks_summarized),
                }) }}
              </p>
              <p v-else class="mt-1 px-1 text-[10px] text-muted-foreground">{{ t('status.llm.paused') }}</p>
            </div>
          </CollapsibleContent>
        </Card>
      </Collapsible>
    </div>
  </div>
</template>
