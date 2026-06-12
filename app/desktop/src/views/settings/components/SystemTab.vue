<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { getVersion } from '@tauri-apps/api/app'
import { openUrl } from '@tauri-apps/plugin-opener'
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  AlertTriangle,
  CheckCircle2,
  Download,
  FileText,
  RefreshCw,
  Server,
  Stethoscope,
  Terminal,
  Trash2,
  XCircle,
} from 'lucide-vue-next'
import { toast } from 'vue-sonner'
import { useMemex } from '@/composables/useMemex'
import { useDaemon } from '@/composables/useDaemon'
import { toastBackendError } from '@/lib/toast-error'
import { humanizeBackendError } from '@/lib/utils'
import { useI18n } from '@/i18n'
import type { CliStatus, DoctorRunResult, UpdateInfo } from '@/types'

const router = useRouter()
const memex = useMemex()
const { t } = useI18n()
const cli = ref<CliStatus | null>(null)
const cliBusy = ref(false)
const doctor = ref<DoctorRunResult | null>(null)
const doctorRunning = ref(false)
const appVersion = ref('—')
const update = ref<UpdateInfo | null>(null)
const updateChecking = ref(false)
// 用 state-object 而不是裸字符串，避免 i18n 之后 startsWith('检测到') 失效。
type UpdateMessageState = { kind: 'latest' | 'found' | 'error'; text: string }
const updateMessage = ref<UpdateMessageState | null>(null)

// Daemon 状态 + 重启 —— 让用户在设置里看到 daemon 健康度并能手动重启。
// 仅调用现有的 `daemon_status` / `daemon_restart` 命令，不依赖任何 daemon 架构改造。
const { status: daemonStatus, loading: daemonLoading, restart: restartDaemon } = useDaemon()

// HTTP 探活在弱网下会偶尔超时，导致 5s 轮询期间 badge 闪烁。这里改成：
// 1) Badge 颜色只看 PID 是否存活（少抖动）
// 2) HTTP 异常用文字 "HTTP 未就绪" 单独提示
const daemonProcessAlive = computed(() => daemonStatus.value?.running === true)
const daemonRunning = computed(
  () => daemonStatus.value?.running === true && daemonStatus.value?.http_ok === true,
)
const daemonStateLabel = computed(() => {
  if (!daemonStatus.value) return t('settings.sys.daemon.checking')
  if (daemonRunning.value) return t('settings.sys.daemon.running')
  if (daemonProcessAlive.value) return t('settings.sys.daemon.http_pending')
  return t('settings.sys.daemon.stopped')
})
const daemonStartedLabel = computed(() => {
  const iso = daemonStatus.value?.started_at
  if (!iso) return '—'
  return iso.replace('T', ' ').slice(0, 19)
})

async function onRestartDaemon() {
  try {
    const s = await restartDaemon()
    if (s && s.running && s.http_ok) {
      toast.success(t('settings.sys.daemon.toast.restarted'))
    } else {
      toast.warning(t('settings.sys.daemon.toast.partial_restart'))
    }
  } catch (e) {
    toastBackendError(t('settings.sys.daemon.toast.restart_failed'), e)
  }
}

// 跳转到应用内置的日志查看页（/logs）。
// 之前 invoke `daemon_log_path` 然后 openUrl(file://) 是错的：daemon 用
// `tracing_appender::rolling::daily` 写到 `~/.memex/logs/daemon.log.YYYY-MM-DD`，
// 没有 stdout.log 这个单文件，所以 file:// 必然失败。/logs 页面里 invoke
// `list_daemon_log_files` / `read_daemon_log` 把 rolling 文件读出来，并且支持
// 选文件 / 调行数 / 过滤关键字 / 自动刷新。
function onOpenDaemonLog() {
  router.push('/logs')
}

const cliInstalled = computed(() => cli.value?.installed ?? false)
const cliPath = computed(() => cli.value?.target_dir ?? '—')

// doctor 改为手动触发；未运行时显示"未检查"，正在跑时显示"检查中…"
function pendingLabel(): string {
  return doctorRunning.value ? t('settings.sys.doctor.checking') : t('settings.sys.doctor.pending')
}
const schemaLabel = computed(() => {
  if (!doctor.value) return pendingLabel()
  const v = doctor.value.report.schema_version
  return v == null ? t('settings.sys.doctor.unknown') : t('settings.sys.doctor.schema_fmt', { v })
})
const ftsLabel = computed(() => {
  if (!doctor.value) return pendingLabel()
  return doctor.value.report.fts_ok ? t('settings.sys.doctor.fts_ok') : t('settings.sys.doctor.fts_err')
})
const cursorLabel = computed(() => {
  if (!doctor.value) return pendingLabel()
  const p = doctor.value.cursor_probe
  if (p.status === 'ok') return t('settings.sys.doctor.cursor_count', { count: p.composer_count.toLocaleString() })
  if (p.status === 'not_found') return t('settings.sys.doctor.cursor_not_found')
  if (p.status === 'permission_denied') return t('settings.sys.doctor.cursor_perm')
  return t('settings.sys.doctor.cursor_err')
})
const dataDir = computed(() => doctor.value?.data_dir ?? pendingLabel())

async function refreshCli() {
  try {
    cli.value = await memex.cliStatus()
  } catch (e) {
    console.warn('[SystemTab] cli status failed', e)
  }
}

async function installCli() {
  cliBusy.value = true
  try {
    cli.value = await memex.cliInstall()
    toast.success(t('settings.sys.cli.toast.installed'))
  } catch (e) {
    toastBackendError(t('settings.sys.cli.toast.install_failed'), e)
  } finally {
    cliBusy.value = false
  }
}

async function uninstallCli() {
  cliBusy.value = true
  try {
    cli.value = await memex.cliUninstall()
    toast.success(t('settings.sys.cli.toast.uninstalled'))
  } catch (e) {
    toastBackendError(t('settings.sys.cli.toast.uninstall_failed'), e)
  } finally {
    cliBusy.value = false
  }
}

async function runDoctor() {
  doctorRunning.value = true
  try {
    doctor.value = await memex.runDoctor()
    toast.success(t('settings.sys.doctor.toast.done'))
  } catch (e) {
    toastBackendError(t('settings.sys.doctor.toast.failed'), e)
  } finally {
    doctorRunning.value = false
  }
}

async function checkUpdate() {
  updateChecking.value = true
  updateMessage.value = null
  try {
    const info = await memex.checkForUpdates()
    update.value = info
    if (!info.latest_tag) {
      updateMessage.value = { kind: 'latest', text: t('settings.sys.about.latest') }
    } else {
      const stripped = info.latest_tag.replace(/^v/, '')
      if (stripped === appVersion.value) {
        updateMessage.value = { kind: 'latest', text: t('settings.sys.about.latest') }
      } else {
        updateMessage.value = {
          kind: 'found',
          text: t('settings.sys.about.found_fmt', { tag: info.latest_tag }),
        }
      }
    }
  } catch (e) {
    updateMessage.value = {
      kind: 'error',
      text: t('settings.sys.about.check_failed_fmt', { err: humanizeBackendError(e).friendly }),
    }
  } finally {
    updateChecking.value = false
  }
}

async function openReleasePage() {
  if (update.value?.html_url) {
    try {
      await openUrl(update.value.html_url)
    } catch (e) {
      toastBackendError(t('settings.sys.about.toast.open_link_failed'), e)
    }
  }
}

onMounted(async () => {
  try {
    appVersion.value = await getVersion()
  } catch {
    /* ignore */
  }
  await refreshCli()
})
</script>

<template>
  <div class="space-y-4">
    <Card>
      <CardHeader>
        <CardDescription>{{ t('settings.sys.daemon_section') }}</CardDescription>
        <CardTitle class="text-base">{{ t('settings.sys.daemon_title') }}</CardTitle>
        <CardAction>
          <Badge
            :variant="daemonProcessAlive ? 'default' : daemonStatus ? 'destructive' : 'secondary'"
            class="gap-1"
          >
            <Server class="size-3" />
            {{ daemonStateLabel }}
          </Badge>
        </CardAction>
      </CardHeader>
      <CardContent class="space-y-3">
        <p class="text-[12px] text-muted-foreground">
          {{ t('settings.sys.daemon.desc') }}
        </p>
        <div class="rounded-md border bg-muted/30 p-4 text-[12px]">
          <div class="grid grid-cols-2 gap-3">
            <div class="flex items-center justify-between">
              <span class="text-muted-foreground">{{ t('settings.sys.daemon.pid_label') }}</span>
              <code class="font-mono text-[11px]">
                {{ daemonStatus?.pid ?? '—' }}
              </code>
            </div>
            <div class="flex items-center justify-between">
              <span class="text-muted-foreground">{{ t('settings.sys.daemon.port_label') }}</span>
              <code class="font-mono text-[11px]">
                {{ daemonStatus?.port ?? '—' }}
              </code>
            </div>
            <div class="flex items-center justify-between">
              <span class="text-muted-foreground">{{ t('settings.sys.daemon.http_label') }}</span>
              <span
                :class="[
                  !daemonStatus
                    ? 'text-muted-foreground/70'
                    : daemonStatus.http_ok
                      ? 'text-emerald-600'
                      : 'text-rose-500',
                ]"
              >
                {{ daemonStatus?.http_ok
                  ? t('settings.sys.daemon.http_ok')
                  : daemonStatus
                    ? t('settings.sys.daemon.http_err')
                    : t('settings.sys.daemon.checking') }}
              </span>
            </div>
            <div class="flex items-center justify-between">
              <span class="text-muted-foreground">{{ t('settings.sys.daemon.started_at') }}</span>
              <code class="font-mono text-[11px]">{{ daemonStartedLabel }}</code>
            </div>
          </div>
        </div>
        <div
          v-if="!daemonRunning && daemonStatus"
          class="flex items-center gap-1.5 rounded-md border border-amber-500/40 bg-amber-500/5 px-3 py-2 text-[12px] text-amber-700"
        >
          <AlertTriangle class="size-3.5 shrink-0" />
          <span>
            {{ t('settings.sys.daemon.http_warn') }}
          </span>
        </div>
      </CardContent>
      <CardFooter class="gap-2">
        <Button
          size="sm"
          variant="outline"
          class="gap-1.5"
          :disabled="daemonLoading"
          @click="onRestartDaemon"
        >
          <RefreshCw :class="['size-3.5', daemonLoading && 'animate-spin']" />
          {{ daemonLoading ? t('settings.sys.daemon.restart_busy') : t('settings.sys.daemon.restart_btn') }}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          class="gap-1.5"
          @click="onOpenDaemonLog"
        >
          <FileText class="size-3.5" />
          {{ t('settings.sys.daemon.view_log') }}
        </Button>
      </CardFooter>
    </Card>

    <Card>
      <CardHeader>
        <CardDescription>{{ t('settings.sys.cli_section') }}</CardDescription>
        <CardTitle class="text-base">{{ t('settings.sys.cli_title') }}</CardTitle>
        <CardAction>
          <Badge :variant="cliInstalled ? 'default' : 'outline'" class="gap-1">
            <Terminal class="size-3" />
            {{ cliInstalled ? t('settings.sys.cli.installed') : t('settings.sys.cli.not_installed') }}
          </Badge>
        </CardAction>
      </CardHeader>
      <CardContent class="space-y-3">
        <div class="flex items-center justify-between text-sm">
          <span>{{ t('settings.sys.cli.install_path') }}</span>
          <code class="font-mono text-xs text-muted-foreground">{{ cliPath }}</code>
        </div>
        <div
          v-if="cliInstalled && cli?.path_contains_target_dir"
          class="flex items-center gap-1.5 rounded-md border border-emerald-500/30 bg-emerald-500/5 px-3 py-2 text-[12px] text-emerald-600"
        >
          <CheckCircle2 class="size-3.5 shrink-0" />
          <span>{{ t('settings.sys.cli.in_path') }}</span>
        </div>
        <div
          v-else-if="cliInstalled && cli && !cli.path_contains_target_dir"
          class="space-y-1 rounded-md border border-amber-500/40 bg-amber-500/5 p-3 text-[12px] text-amber-700"
        >
          <div class="flex items-center gap-1.5">
            <AlertTriangle class="size-3.5 shrink-0" />
            <span>{{ t('settings.sys.cli.not_in_path', { path: cliPath }) }}</span>
          </div>
          <code class="block break-all font-mono text-[11px]">{{ cli.path_export_hint }}</code>
        </div>
        <div
          v-else
          class="flex items-center gap-1.5 rounded-md border border-zinc-500/30 bg-muted/30 px-3 py-2 text-[12px] text-muted-foreground"
        >
          <XCircle class="size-3.5 shrink-0" />
          <span>{{ t('settings.sys.cli.absent') }}</span>
        </div>
      </CardContent>
      <CardFooter class="gap-2">
        <Button
          v-if="cliInstalled"
          size="sm"
          variant="outline"
          class="gap-1.5"
          :disabled="cliBusy"
          @click="uninstallCli"
        >
          <Trash2 class="size-3.5" />
          {{ cliBusy ? t('settings.sys.cli.busy') : t('settings.sys.cli.uninstall') }}
        </Button>
        <Button v-else size="sm" class="gap-1.5" :disabled="cliBusy" @click="installCli">
          <Download class="size-3.5" />
          {{ cliBusy ? t('settings.sys.cli.busy') : t('settings.sys.cli.install') }}
        </Button>
      </CardFooter>
    </Card>

    <Card>
      <CardHeader>
        <CardDescription>{{ t('settings.sys.doctor_section') }}</CardDescription>
        <CardTitle class="text-base">{{ t('settings.sys.doctor_title') }}</CardTitle>
      </CardHeader>
      <CardContent class="space-y-3">
        <div class="rounded-md border bg-muted/30 p-4 text-[12px]">
          <div class="grid grid-cols-2 gap-3">
            <div class="flex items-center justify-between">
              <span class="text-muted-foreground">{{ t('settings.sys.doctor.data_dir') }}</span>
              <code
                :class="[
                  'font-mono text-[11px]',
                  !doctor && 'text-muted-foreground/70',
                ]"
              >
                {{ dataDir }}
              </code>
            </div>
            <div class="flex items-center justify-between">
              <span class="text-muted-foreground">{{ t('settings.sys.doctor.db') }}</span>
              <span
                :class="[
                  !doctor
                    ? 'text-muted-foreground/70'
                    : doctor.report.db_exists
                      ? 'text-emerald-600'
                      : 'text-rose-500',
                ]"
              >
                {{ doctor
                  ? (doctor.report.db_exists ? schemaLabel : t('settings.sys.doctor.db_missing'))
                  : schemaLabel }}
              </span>
            </div>
            <div class="flex items-center justify-between">
              <span class="text-muted-foreground">{{ t('settings.sys.doctor.fts') }}</span>
              <span
                :class="[
                  !doctor
                    ? 'text-muted-foreground/70'
                    : doctor.report.fts_ok
                      ? 'text-emerald-600'
                      : 'text-rose-500',
                ]"
              >
                {{ ftsLabel }}
              </span>
            </div>
            <div class="flex items-center justify-between">
              <span class="text-muted-foreground">{{ t('settings.sys.doctor.cursor') }}</span>
              <span
                :class="[
                  !doctor
                    ? 'text-muted-foreground/70'
                    : doctor.cursor_probe.status === 'ok'
                      ? 'text-emerald-600'
                      : 'text-amber-600',
                ]"
              >
                {{ cursorLabel }}
              </span>
            </div>
          </div>
        </div>
      </CardContent>
      <CardFooter>
        <Button size="sm" variant="outline" class="gap-1.5" :disabled="doctorRunning" @click="runDoctor">
          <Stethoscope :class="['size-3.5', doctorRunning && 'animate-spin']" />
          {{ doctorRunning ? t('settings.sys.doctor.running') : t('settings.sys.doctor.run') }}
        </Button>
      </CardFooter>
    </Card>

    <Card>
      <CardHeader>
        <CardDescription>{{ t('settings.sys.about_section') }}</CardDescription>
        <CardTitle class="text-base">{{ t('settings.sys.about_title') }}</CardTitle>
        <CardAction>
          <Badge variant="secondary" class="font-mono text-[11px]">v{{ appVersion }}</Badge>
        </CardAction>
      </CardHeader>
      <CardContent class="space-y-3">
        <div class="flex items-center justify-between text-sm">
          <span>{{ t('settings.sys.about.current') }}</span>
          <div class="flex items-center gap-2">
            <span class="font-mono text-xs text-muted-foreground">v{{ appVersion }}</span>
            <Button
              size="sm"
              variant="outline"
              class="h-7 gap-1 text-xs"
              :disabled="updateChecking"
              @click="checkUpdate"
            >
              <RefreshCw :class="['size-3', updateChecking && 'animate-spin']" />
              {{ updateChecking ? t('settings.sys.about.checking') : t('settings.sys.about.check_update') }}
            </Button>
          </div>
        </div>
        <div
          v-if="updateMessage"
          :class="
            updateMessage.kind === 'found'
              ? 'flex items-center gap-1.5 rounded-md border border-sky-500/40 bg-sky-500/5 px-3 py-2 text-[12px] text-sky-600'
              : updateMessage.kind === 'error'
                ? 'flex items-center gap-1.5 rounded-md border border-rose-500/40 bg-rose-500/5 px-3 py-2 text-[12px] text-rose-600'
                : 'flex items-center gap-1.5 rounded-md border border-emerald-500/30 bg-emerald-500/5 px-3 py-2 text-[12px] text-emerald-600'
          "
        >
          <CheckCircle2 v-if="updateMessage.kind === 'latest'" class="size-3.5 shrink-0" />
          <AlertTriangle v-else-if="updateMessage.kind === 'error'" class="size-3.5 shrink-0" />
          <Download v-else class="size-3.5 shrink-0" />
          <span class="flex-1">{{ updateMessage.text }}</span>
          <Button
            v-if="updateMessage.kind === 'found' && update?.html_url"
            size="sm"
            variant="ghost"
            class="h-6 text-xs"
            @click="openReleasePage"
          >
            {{ t('settings.sys.about.open_release') }}
          </Button>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
