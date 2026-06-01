<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useMemex } from '@/composables/useMemex'
import { formatNumber } from '@/lib/utils'
import { Separator } from '@/components/ui/separator'
import { Button } from '@/components/ui/button'
import { RefreshCw, Power } from 'lucide-vue-next'
import type { Stats, DaemonStatus } from '@/types'

const { getStats, getConfig, daemonStatus, daemonRestart } = useMemex()

type Tone = 'success' | 'warning' | 'error' | 'muted'
interface Signal { label: string; value: string; hint?: string; tone: Tone }

const stats = ref<Stats>({
  sessions: 0,
  messages: 0,
  chunks: 0,
  db_exists: false,
  summaries: 0,
  chunks_summarized: 0,
  llm_provider: null,
})
const loading = ref(true)
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
const llmCloud = ref<boolean>(false)

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
  try { stats.value = await getStats() } catch { /* ignore */ }

  for (const a of adapterDefs) {
    try {
      const v = await getConfig(`adapter.${a.key}.enabled`)
      adapterEnabled.value[a.key] = v === null ? true : v === 'true'
    } catch {
      adapterEnabled.value[a.key] = true
    }
  }
  try {
    const v = await getConfig('llm.ollama_enabled')
    llmOllama.value = v === 'true'
  } catch { /* default */ }
  try {
    const v = await getConfig('llm.cloud_fallback')
    llmCloud.value = v === 'true'
  } catch { /* default */ }

  await probeDaemon()
  loading.value = false
})

function refresh() {
  loading.value = true
  Promise.allSettled([getStats().then((s) => (stats.value = s)), probeDaemon()]).finally(() => {
    loading.value = false
  })
}

function daemonSignal(): Signal {
  if (!daemon.value) return { label: '后台服务', value: '检查中…', tone: 'muted' }
  if (daemon.value.running && daemon.value.http_ok) {
    return {
      label: '后台服务',
      value: '运行中',
      hint: daemon.value.port ? `127.0.0.1:${daemon.value.port} · PID ${daemon.value.pid}` : undefined,
      tone: 'success',
    }
  }
  if (daemon.value.running && !daemon.value.http_ok) {
    return {
      label: '后台服务',
      value: '启动中',
      hint: daemon.value.port ? `PID ${daemon.value.pid}，端口 ${daemon.value.port} 暂无响应` : `PID ${daemon.value.pid}，HTTP 暂无响应`,
      tone: 'warning',
    }
  }
  return {
    label: '后台服务',
    value: '未运行',
    hint: '点击右侧"重启"启动 memex-daemon',
    tone: 'error',
  }
}

const systemSignals = (): Signal[] => {
  const out: Signal[] = []
  out.push(daemonSignal())
  out.push(
    stats.value.db_exists
      ? { label: '数据库', value: '就绪', hint: `${formatNumber(stats.value.sessions)} 个会话 · ${formatNumber(stats.value.messages)} 条消息`, tone: 'success' }
      : { label: '数据库', value: '未初始化', hint: '运行 memex ingest', tone: 'error' },
  )
  out.push({
    label: '索引',
    value: formatNumber(stats.value.chunks) + ' 个 chunk',
    hint: stats.value.chunks > 0 ? 'FTS5 已就绪' : '尚未生成索引',
    tone: stats.value.chunks > 0 ? 'success' : 'muted',
  })
  return out
}

const adapterSignals = (): Signal[] =>
  adapterDefs.map((a) => ({
    label: a.label,
    value: adapterEnabled.value[a.key] ? '已启用' : '已禁用',
    tone: adapterEnabled.value[a.key] ? 'success' : 'muted',
  }))

const llmSignals = (): Signal[] => {
  const active = stats.value.llm_provider
  const out: Signal[] = []
  out.push(
    active
      ? { label: '当前提供方', value: active, hint: `${formatNumber(stats.value.summaries)} 个会话 · ${formatNumber(stats.value.chunks_summarized)} 个 chunk 摘要`, tone: 'success' }
      : { label: '当前提供方', value: '未启用', hint: '摘要功能已暂停', tone: 'muted' },
  )
  out.push({
    label: 'Ollama',
    value: llmOllama.value ? '已启用' : '已禁用',
    tone: llmOllama.value ? 'success' : 'muted',
  })
  out.push({
    label: '云端兜底',
    value: llmCloud.value ? '已开启' : '关闭',
    hint: llmCloud.value ? '发送前会做脱敏' : undefined,
    tone: llmCloud.value ? 'warning' : 'muted',
  })
  return out
}

const dotClass: Record<Tone, string> = {
  success: 'bg-success',
  warning: 'bg-warning',
  error: 'bg-destructive',
  muted: 'bg-muted-foreground/40',
}

const valueClass: Record<Tone, string> = {
  success: 'text-success',
  warning: 'text-warning',
  error: 'text-destructive',
  muted: 'text-muted-foreground',
}
</script>

<template>
  <div class="h-full overflow-y-auto px-4 py-4">
    <header class="mb-4 flex items-baseline justify-between">
      <h2 class="text-base font-semibold">系统状态</h2>
      <Button variant="ghost" size="sm" :disabled="loading" class="gap-1.5" @click="refresh">
        <RefreshCw class="h-3.5 w-3.5" :class="{ 'animate-spin': loading }" />
        {{ loading ? '刷新中' : '刷新' }}
      </Button>
    </header>

    <div v-if="loading && daemon === null" class="text-sm text-muted-foreground">加载中…</div>

    <template v-else>
      <!-- 系统 -->
      <section>
        <div class="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">系统</div>
        <ul class="space-y-2">
          <li v-for="(s, i) in systemSignals()" :key="s.label" class="flex items-center gap-2.5">
            <span class="h-2 w-2 shrink-0 rounded-full" :class="dotClass[s.tone]" />
            <span class="flex-1 text-sm text-foreground">{{ s.label }}</span>
            <span class="text-sm font-medium" :class="valueClass[s.tone]">{{ s.value }}</span>
            <Button
              v-if="i === 0 && (s.tone === 'error' || s.tone === 'warning')"
              variant="outline"
              size="sm"
              class="h-6 gap-1 px-2 text-xs"
              :disabled="restarting"
              @click="handleRestart"
            >
              <Power class="h-3 w-3" :class="{ 'animate-pulse': restarting }" />
              {{ restarting ? '启动中' : '重启' }}
            </Button>
            <Button
              v-else-if="i === 0 && s.tone === 'success'"
              variant="ghost"
              size="sm"
              class="h-6 gap-1 px-2 text-xs text-muted-foreground"
              :disabled="restarting"
              @click="handleRestart"
              title="重启 memex-daemon"
            >
              <Power class="h-3 w-3" :class="{ 'animate-pulse': restarting }" />
              {{ restarting ? '重启中' : '重启' }}
            </Button>
          </li>
        </ul>
        <ul class="mt-1.5 space-y-1 pl-4">
          <li
            v-for="s in systemSignals().filter((x) => x.hint)"
            :key="s.label + '-hint'"
            class="text-xs text-muted-foreground"
          >{{ s.label }}: {{ s.hint }}</li>
          <li v-if="restartError" class="text-xs text-destructive">重启失败: {{ restartError }}</li>
        </ul>
      </section>

      <Separator class="my-4" />

      <!-- 适配器 -->
      <section>
        <div class="mb-2 flex items-baseline justify-between">
          <div class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">适配器</div>
          <div class="text-xs text-muted-foreground">{{ adapterSignals().filter((s) => s.tone === 'success').length }} / {{ adapterDefs.length }} 已启用</div>
        </div>
        <ul class="space-y-2">
          <li v-for="s in adapterSignals()" :key="s.label" class="flex items-center gap-2.5">
            <span class="h-2 w-2 shrink-0 rounded-full" :class="dotClass[s.tone]" />
            <span class="flex-1 text-sm">{{ s.label }}</span>
            <span class="text-sm" :class="valueClass[s.tone]">{{ s.value }}</span>
          </li>
        </ul>
      </section>

      <Separator class="my-4" />

      <!-- LLM -->
      <section>
        <div class="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">LLM</div>
        <ul class="space-y-2">
          <li v-for="s in llmSignals()" :key="s.label" class="flex items-center gap-2.5">
            <span class="h-2 w-2 shrink-0 rounded-full" :class="dotClass[s.tone]" />
            <span class="flex-1 text-sm">{{ s.label }}</span>
            <span class="text-sm" :class="valueClass[s.tone]">{{ s.value }}</span>
          </li>
        </ul>
        <ul class="mt-1.5 space-y-1 pl-4">
          <li
            v-for="s in llmSignals().filter((x) => x.hint)"
            :key="s.label + '-hint'"
            class="text-xs text-muted-foreground"
          >{{ s.label }}: {{ s.hint }}</li>
        </ul>
      </section>
    </template>
  </div>
</template>
