<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { invoke } from '@tauri-apps/api/core'
import {
  Activity,
  AlertTriangle,
  ChevronRight,
  RefreshCw,
  Settings as SettingsIcon,
  Sparkles,
} from 'lucide-vue-next'
import { toast } from 'vue-sonner'
import { Button } from '@/components/ui/button'
import { sessions, totals, daemon, daemonStatus, stats, ADAPTER_MAP, refreshSessions } from '@/stores/memex'
import { useMemex } from '@/composables/useMemex'
import { useDaemon } from '@/composables/useDaemon'
import { formatNumber } from '@/lib/utils'
import { toastBackendError } from '@/lib/toast-error'
import { useI18n } from '@/i18n'

const { t } = useI18n()
const memex = useMemex()
// useDaemon 内部会启动 5s 轮询 + onMounted refresh，把结果写回 stores/memex.daemon。
// popup 失焦自动隐藏后会 onScopeDispose 自动停止轮询，无需手动管理。
const { restart: restartDaemon, loading: daemonLoading } = useDaemon()

const appWindow = getCurrentWindow()
const recent = computed(() => sessions.slice(0, 5))
const llmModel = computed(() => daemonStatus.llmModel ?? 'qwen2.5')

const daemonRunning = computed(() => daemon.value?.running ?? false)
const daemonLabel = computed(() => {
  if (!daemon.value) return t('tray.daemon.querying')
  if (!daemon.value.running) return t('tray.daemon.offline')
  return daemon.value.pid
    ? t('tray.daemon.running_with_pid', { pid: daemon.value.pid })
    : t('tray.daemon.running')
})

async function onRestartDaemon(e: MouseEvent) {
  e.stopPropagation()
  if (daemonLoading.value) return
  try {
    const r = await restartDaemon()
    if (r?.running) toast.success(t('tray.daemon.toast.restarted'))
    else toast.error(t('tray.daemon.toast.restart_not_running'))
  } catch (err) {
    toastBackendError(t('tray.daemon.toast.restart_failed'), err)
  }
}

function timeAgo(iso: string): string {
  const diff = (Date.now() - new Date(iso).getTime()) / 1000
  if (diff < 60) return `${Math.floor(diff)}s`
  if (diff < 3600) return `${Math.floor(diff / 60)}m`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h`
  return `${Math.floor(diff / 86400)}d`
}

async function hideAndNavigate(path: string) {
  // close-to-tray 场景下主窗口可能被隐藏，「前端发 emit」存在竞态：
  // 主窗口刚 show 时它的 navigate listener 还没 mount → 事件丢失，用户点 popup 无反应。
  // 解法：把 navigate 一起交给后端，由后端在 show 主窗口的同时直接 emit 到 main window，
  // 避免依赖前端 mount 时序。
  await invoke('show_main_window', { navigate: path }).catch(() => {})
  await appWindow.hide()
}

async function openMain() {
  await hideAndNavigate('/today')
}

async function openSettings() {
  await hideAndNavigate('/settings')
}

async function openSession(id: string) {
  await hideAndNavigate(`/library?session=${id}`)
}

onMounted(async () => {
  // 每次弹出时拉最新 5 条 session（store 已经初始化过，这里只是增量刷新）
  void refreshSessions(5)

  // 顺手拉一次 stats 让"sessions/messages"数字也保持准确（轻量调用）。
  // 这里必须把 IPC 结果写回 `stats` ref，否则 `totals.messages` 永远保持初始值 0
  // —— popup 是临时窗口，没有 useStats 的轮询常驻给它兜底。
  void memex
    .getStats()
    .then((v) => {
      stats.value = v
    })
    .catch(() => {})

  appWindow.onFocusChanged(({ payload: focused }) => {
    if (!focused) appWindow.hide().catch(() => {})
  })
  window.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') appWindow.hide().catch(() => {})
  })
})
</script>

<template>
  <div class="flex h-screen w-screen p-2" style="background: transparent;">
    <div
      class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border border-border/80 bg-card shadow-[0_8px_32px_-8px_rgba(15,23,42,0.18),0_4px_12px_-4px_rgba(15,23,42,0.12)]"
    >
      <header class="flex items-center justify-between border-b px-3 py-2.5">
        <div class="flex items-center gap-2">
          <div
            class="flex aspect-square size-6 items-center justify-center rounded-md text-[11px] font-bold text-white"
            style="background: linear-gradient(135deg, #18181b, #4f46e5);"
          >
            M
          </div>
          <div class="flex flex-col leading-tight">
            <span class="text-[12px] font-bold tracking-tight">Memex</span>
            <span
              class="text-[10px] text-muted-foreground tabular-nums"
              :title="`${totals.sessions.toLocaleString()} ${t('tray.totals.sessions_suffix')}`"
            >
              {{ formatNumber(totals.sessions) }} {{ t('tray.totals.sessions_suffix') }}
            </span>
          </div>
        </div>
        <Button variant="ghost" size="icon" class="h-7 w-7" @click="openSettings">
          <SettingsIcon class="size-3.5" />
        </Button>
      </header>

      <!-- 数据状态条：日常 90% 时间内只露 LLM 模型 + 消息数；仅当内嵌服务异常时
           才会弹出 amber 警告 + 重启按钮。daemon 已经跟 app 同生命周期，正常态
           不需要单独提示用户「后台运行中」。 -->
      <div class="mx-2 mt-2 flex items-center gap-2 rounded-lg border border-border/60 bg-muted/30 px-3 py-2">
        <div class="flex size-7 shrink-0 items-center justify-center rounded-md bg-emerald-500/15 text-emerald-600">
          <Activity class="size-4" />
        </div>
        <div class="min-w-0 flex-1 truncate text-[11px] text-muted-foreground">
          {{ llmModel }} · {{ t('tray.status.message_count', { n: formatNumber(totals.messages) }) }}
        </div>
      </div>

      <div
        v-if="!daemonRunning && daemon"
        class="mx-2 mt-2 flex items-center gap-2 rounded-lg border border-amber-500/40 bg-amber-500/10 px-3 py-2"
      >
        <div class="flex size-7 shrink-0 items-center justify-center rounded-md bg-amber-500/20 text-amber-600">
          <AlertTriangle class="size-4" />
        </div>
        <div class="min-w-0 flex-1 leading-tight">
          <div class="text-[11px] font-semibold text-amber-700">{{ daemonLabel }}</div>
          <div class="mt-0.5 truncate text-[10px] text-muted-foreground">
            {{ t('tray.daemon.alert.port_busy') }}
          </div>
        </div>
        <Button
          variant="outline"
          size="sm"
          class="h-7 shrink-0 gap-1 text-[11px]"
          :disabled="daemonLoading"
          @click="onRestartDaemon"
        >
          <RefreshCw :class="['size-3', daemonLoading && 'animate-spin']" />
          {{ daemonLoading ? t('tray.daemon.action.restarting') : t('tray.daemon.action.restart') }}
        </Button>
      </div>

      <section class="min-h-0 flex-1 overflow-y-auto">
        <div class="px-3 pt-2 pb-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          {{ t('tray.section.recent') }}
        </div>
        <button
          v-for="s in recent"
          :key="s.id"
          class="grid w-full grid-cols-[1fr_auto] items-start gap-2 px-3 py-2 text-left transition-colors hover:bg-accent"
          @click="openSession(s.id)"
        >
          <div class="min-w-0">
            <div class="truncate text-[12px] font-semibold">{{ s.title }}</div>
            <div class="truncate text-[10px] text-muted-foreground">
              {{ s.project }} · {{ ADAPTER_MAP[s.adapter].label }}
            </div>
          </div>
          <div class="shrink-0 text-[10px] text-muted-foreground tabular-nums">
            {{ timeAgo(s.startedAt) }}
          </div>
        </button>
        <div v-if="recent.length === 0" class="flex flex-col items-center gap-2 px-4 py-8 text-center">
          <Sparkles class="size-6 text-muted-foreground/50" />
          <p class="text-[11px] text-muted-foreground">{{ t('tray.empty.no_sessions') }}</p>
        </div>
      </section>

      <footer class="flex items-center justify-end border-t bg-muted/40 px-3 py-2">
        <Button variant="default" size="sm" class="h-7 gap-1 text-[11px]" @click="openMain">
          {{ t('tray.footer.open_main') }}
          <ChevronRight class="size-3" />
        </Button>
      </footer>
    </div>
  </div>
</template>
