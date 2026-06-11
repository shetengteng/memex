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
import type { CliStatus, DoctorRunResult, UpdateInfo } from '@/types'

const router = useRouter()
const memex = useMemex()
const cli = ref<CliStatus | null>(null)
const cliBusy = ref(false)
const doctor = ref<DoctorRunResult | null>(null)
const doctorRunning = ref(false)
const appVersion = ref('—')
const update = ref<UpdateInfo | null>(null)
const updateChecking = ref(false)
const updateMessage = ref<string>('')

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
  if (!daemonStatus.value) return '检查中…'
  if (daemonRunning.value) return '运行中'
  if (daemonProcessAlive.value) return 'HTTP 未就绪'
  return '已停止'
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
      toast.success('后台服务已重启')
    } else {
      toast.warning('后台服务已重启，但 HTTP 尚未就绪，稍候片刻再试')
    }
  } catch (e) {
    toastBackendError('重启失败', e)
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
  return doctorRunning.value ? '检查中…' : '未检查'
}
const schemaLabel = computed(() => {
  if (!doctor.value) return pendingLabel()
  const v = doctor.value.report.schema_version
  return v == null ? '未知' : `Schema v${v}`
})
const ftsLabel = computed(() => {
  if (!doctor.value) return pendingLabel()
  return doctor.value.report.fts_ok ? '正常' : '异常'
})
const cursorLabel = computed(() => {
  if (!doctor.value) return pendingLabel()
  const p = doctor.value.cursor_probe
  if (p.status === 'ok') return `${p.composer_count.toLocaleString()} composers`
  if (p.status === 'not_found') return '未找到'
  if (p.status === 'permission_denied') return '无权限'
  return '错误'
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
    toast.success('CLI 已安装')
  } catch (e) {
    toastBackendError('安装失败', e)
  } finally {
    cliBusy.value = false
  }
}

async function uninstallCli() {
  cliBusy.value = true
  try {
    cli.value = await memex.cliUninstall()
    toast.success('CLI 已卸载')
  } catch (e) {
    toastBackendError('卸载失败', e)
  } finally {
    cliBusy.value = false
  }
}

async function runDoctor() {
  doctorRunning.value = true
  try {
    doctor.value = await memex.runDoctor()
    toast.success('Doctor 检查完成')
  } catch (e) {
    toastBackendError('检查失败', e)
  } finally {
    doctorRunning.value = false
  }
}

async function checkUpdate() {
  updateChecking.value = true
  updateMessage.value = ''
  try {
    const info = await memex.checkForUpdates()
    update.value = info
    if (!info.latest_tag) {
      updateMessage.value = '当前已是最新版本'
    } else {
      const stripped = info.latest_tag.replace(/^v/, '')
      if (stripped === appVersion.value) updateMessage.value = '当前已是最新版本'
      else updateMessage.value = `检测到新版本 ${info.latest_tag}`
    }
  } catch (e) {
    updateMessage.value = `检查失败：${humanizeBackendError(e).friendly}`
  } finally {
    updateChecking.value = false
  }
}

async function openReleasePage() {
  if (update.value?.html_url) {
    try {
      await openUrl(update.value.html_url)
    } catch (e) {
      toastBackendError('打开链接失败', e)
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
        <CardDescription>后台服务</CardDescription>
        <CardTitle class="text-base">Memex Daemon</CardTitle>
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
        <div class="rounded-md border bg-muted/30 p-4 text-[12px]">
          <div class="grid grid-cols-2 gap-3">
            <div class="flex items-center justify-between">
              <span class="text-muted-foreground">PID</span>
              <code class="font-mono text-[11px]">
                {{ daemonStatus?.pid ?? '—' }}
              </code>
            </div>
            <div class="flex items-center justify-between">
              <span class="text-muted-foreground">监听端口</span>
              <code class="font-mono text-[11px]">
                {{ daemonStatus?.port ?? '—' }}
              </code>
            </div>
            <div class="flex items-center justify-between">
              <span class="text-muted-foreground">HTTP 健康</span>
              <span
                :class="[
                  !daemonStatus
                    ? 'text-muted-foreground/70'
                    : daemonStatus.http_ok
                      ? 'text-emerald-600'
                      : 'text-rose-500',
                ]"
              >
                {{ daemonStatus?.http_ok ? '正常' : daemonStatus ? '异常' : '检查中…' }}
              </span>
            </div>
            <div class="flex items-center justify-between">
              <span class="text-muted-foreground">启动时间</span>
              <code class="font-mono text-[11px]">{{ daemonStartedLabel }}</code>
            </div>
          </div>
        </div>
        <div
          v-if="!daemonRunning && daemonStatus"
          class="flex items-center gap-1.5 rounded-md border border-rose-500/40 bg-rose-500/5 px-3 py-2 text-[12px] text-rose-600"
        >
          <AlertTriangle class="size-3.5 shrink-0" />
          <span>
            后台服务未就绪，所有数据与功能可能暂时不可用。系统会自动尝试拉起，或点击右下按钮手动重启。
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
          {{ daemonLoading ? '处理中…' : '重启 Daemon' }}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          class="gap-1.5"
          @click="onOpenDaemonLog"
        >
          <FileText class="size-3.5" />
          查看日志
        </Button>
      </CardFooter>
    </Card>

    <Card>
      <CardHeader>
        <CardDescription>命令行</CardDescription>
        <CardTitle class="text-base">CLI 工具</CardTitle>
        <CardAction>
          <Badge :variant="cliInstalled ? 'default' : 'outline'" class="gap-1">
            <Terminal class="size-3" />
            {{ cliInstalled ? '已安装' : '未安装' }}
          </Badge>
        </CardAction>
      </CardHeader>
      <CardContent class="space-y-3">
        <div class="flex items-center justify-between text-sm">
          <span>安装路径</span>
          <code class="font-mono text-xs text-muted-foreground">{{ cliPath }}</code>
        </div>
        <div
          v-if="cliInstalled && cli?.path_contains_target_dir"
          class="flex items-center gap-1.5 rounded-md border border-emerald-500/30 bg-emerald-500/5 px-3 py-2 text-[12px] text-emerald-600"
        >
          <CheckCircle2 class="size-3.5 shrink-0" />
          <span>CLI 已安装且在 PATH 中可用</span>
        </div>
        <div
          v-else-if="cliInstalled && cli && !cli.path_contains_target_dir"
          class="space-y-1 rounded-md border border-amber-500/40 bg-amber-500/5 p-3 text-[12px] text-amber-700"
        >
          <div class="flex items-center gap-1.5">
            <AlertTriangle class="size-3.5 shrink-0" />
            <span>已安装，但 {{ cliPath }} 不在 PATH 中</span>
          </div>
          <code class="block break-all font-mono text-[11px]">{{ cli.path_export_hint }}</code>
        </div>
        <div
          v-else
          class="flex items-center gap-1.5 rounded-md border border-zinc-500/30 bg-muted/30 px-3 py-2 text-[12px] text-muted-foreground"
        >
          <XCircle class="size-3.5 shrink-0" />
          <span>CLI 尚未安装</span>
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
          {{ cliBusy ? '处理中…' : '卸载 CLI' }}
        </Button>
        <Button v-else size="sm" class="gap-1.5" :disabled="cliBusy" @click="installCli">
          <Download class="size-3.5" />
          {{ cliBusy ? '处理中…' : '安装到 PATH' }}
        </Button>
      </CardFooter>
    </Card>

    <Card>
      <CardHeader>
        <CardDescription>诊断</CardDescription>
        <CardTitle class="text-base">Doctor 系统检查</CardTitle>
      </CardHeader>
      <CardContent class="space-y-3">
        <div class="rounded-md border bg-muted/30 p-4 text-[12px]">
          <div class="grid grid-cols-2 gap-3">
            <div class="flex items-center justify-between">
              <span class="text-muted-foreground">数据目录</span>
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
              <span class="text-muted-foreground">数据库</span>
              <span
                :class="[
                  !doctor
                    ? 'text-muted-foreground/70'
                    : doctor.report.db_exists
                      ? 'text-emerald-600'
                      : 'text-rose-500',
                ]"
              >
                {{ doctor ? (doctor.report.db_exists ? schemaLabel : '未找到') : schemaLabel }}
              </span>
            </div>
            <div class="flex items-center justify-between">
              <span class="text-muted-foreground">FTS5 索引</span>
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
              <span class="text-muted-foreground">Cursor 数据</span>
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
          {{ doctorRunning ? '运行中…' : '运行 Doctor' }}
        </Button>
      </CardFooter>
    </Card>

    <Card>
      <CardHeader>
        <CardDescription>关于</CardDescription>
        <CardTitle class="text-base">Memex</CardTitle>
        <CardAction>
          <Badge variant="secondary" class="font-mono text-[11px]">v{{ appVersion }}</Badge>
        </CardAction>
      </CardHeader>
      <CardContent class="space-y-3">
        <div class="flex items-center justify-between text-sm">
          <span>当前版本</span>
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
              {{ updateChecking ? '检查中…' : '检查更新' }}
            </Button>
          </div>
        </div>
        <div
          v-if="updateMessage"
          :class="
            updateMessage.startsWith('检测到')
              ? 'flex items-center gap-1.5 rounded-md border border-sky-500/40 bg-sky-500/5 px-3 py-2 text-[12px] text-sky-600'
              : 'flex items-center gap-1.5 rounded-md border border-emerald-500/30 bg-emerald-500/5 px-3 py-2 text-[12px] text-emerald-600'
          "
        >
          <CheckCircle2 v-if="!updateMessage.startsWith('检测到')" class="size-3.5 shrink-0" />
          <Download v-else class="size-3.5 shrink-0" />
          <span class="flex-1">{{ updateMessage }}</span>
          <Button
            v-if="updateMessage.startsWith('检测到') && update?.html_url"
            size="sm"
            variant="ghost"
            class="h-6 text-xs"
            @click="openReleasePage"
          >
            打开发布页
          </Button>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
