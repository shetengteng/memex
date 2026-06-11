<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { save as saveDialog, open as openDialog } from '@tauri-apps/plugin-dialog'
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
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import {
  Download,
  FolderArchive,
  FolderOpen,
  Info,
  RefreshCw,
  Trash2,
  Upload,
} from 'lucide-vue-next'
import { toast } from 'vue-sonner'
import { useMemex } from '@/composables/useMemex'
import { stats } from '@/stores/memex'
import { toastBackendError } from '@/lib/toast-error'

const memex = useMemex()
const dbPath = ref<string>('')
const dataDir = ref<string>('')
const lastBackupPath = ref<string>('')
const rebuilding = ref(false)
const clearing = ref(false)
const backingUp = ref(false)
const openingFolder = ref(false)
const exporting = ref(false)
const importing = ref(false)

interface BackupResult {
  path: string
  files: number
  size_bytes: number
}

interface ImportResult {
  source: string
  before_path: string
  files: number
}

const backupDir = computed(() =>
  dataDir.value ? `${dataDir.value}/backups` : '',
)

async function onBackupNow() {
  if (backingUp.value) return
  backingUp.value = true
  try {
    const r = await invoke<BackupResult>('backup_now')
    lastBackupPath.value = r.path
    const mb = (r.size_bytes / 1024 / 1024).toFixed(1)
    toast.success(`备份完成（${r.files} 个文件 · ${mb} MB）`, {
      description: r.path,
      duration: 10_000,
      action: {
        label: '在 Finder 显示',
        onClick: () => {
          void revealItemInDir(r.path).catch((e) =>
            toastBackendError('无法打开文件位置', e),
          )
        },
      },
    })
  } catch (e) {
    toastBackendError('备份失败', e)
  } finally {
    backingUp.value = false
  }
}

async function onOpenBackupFolder() {
  if (!backupDir.value || openingFolder.value) return
  openingFolder.value = true
  try {
    // 第一次点击如果文件夹还不存在（用户从未备份过），先建出来，
    // 否则 Finder 会跳到 `~/.memex` 而不是 backups 子目录。
    await invoke('ensure_backup_dir').catch(() => {
      /* ignore: 即使 IPC 不存在也尝试直接打开父目录 */
    })
    await revealItemInDir(backupDir.value)
  } catch (e) {
    toastBackendError('无法打开备份目录', e)
  } finally {
    openingFolder.value = false
  }
}

const sessionsTotal = computed(() => stats.value?.sessions ?? 0)
const messagesTotal = computed(() => stats.value?.messages ?? 0)
const summariesTotal = computed(() => stats.value?.summaries ?? 0)

onMounted(async () => {
  try {
    // 改用轻量 command，不再为了拿一个路径预跑 doctor
    const dir = await invoke<string>('memex_data_dir')
    if (dir) {
      dataDir.value = dir
      dbPath.value = `${dir}/memex.db`
    }
  } catch {
    /* ignore */
  }
})

async function rebuildIndex() {
  if (!confirm('确认重建 FTS 索引？此操作不会删除数据，但需要数分钟。')) return
  rebuilding.value = true
  try {
    await memex.systemResetIndex()
    toast.success('索引已重建')
  } catch (e) {
    toastBackendError('重建失败', e)
  } finally {
    rebuilding.value = false
  }
}

async function clearAll() {
  const txt = prompt('请输入 DELETE 确认清空全部数据（不可恢复）：')
  if (txt !== 'DELETE') return
  clearing.value = true
  try {
    await memex.systemResetAll()
    toast.success('已清空全部数据')
  } catch (e) {
    toastBackendError('清空失败', e)
  } finally {
    clearing.value = false
  }
}

async function exportDb() {
  if (exporting.value) return
  const defaultName = `memex-${new Date().toISOString().slice(0, 10).replace(/-/g, '')}.tar.gz`
  const target = await saveDialog({
    title: '导出 Memex 数据库',
    defaultPath: defaultName,
    filters: [{ name: '归档', extensions: ['tar.gz', 'tgz'] }],
  })
  if (!target) return

  exporting.value = true
  const loadingId = toast.loading('正在导出…')
  try {
    const r = await invoke<BackupResult>('export_db', { targetPath: target })
    const mb = (r.size_bytes / 1024 / 1024).toFixed(1)
    toast.dismiss(loadingId)
    toast.success(`导出完成（${r.files} 个文件 · ${mb} MB）`, {
      description: r.path,
      duration: 10_000,
      action: {
        label: '在 Finder 显示',
        onClick: () => {
          void revealItemInDir(r.path).catch((e) =>
            toastBackendError('无法打开文件位置', e),
          )
        },
      },
    })
  } catch (e) {
    toast.dismiss(loadingId)
    toastBackendError('导出失败', e)
  } finally {
    exporting.value = false
  }
}

async function importDb() {
  if (importing.value) return
  if (
    !confirm(
      '导入会先停止后台服务、用归档替换 ~/.memex/{memex.db, config.toml, sessions/}，' +
        '然后重启服务。原数据会先搬到 ~/.memex/.before-restore-* 作为安全网。\n\n确认继续？',
    )
  )
    return

  const source = await openDialog({
    title: '选择 Memex 归档（.tar.gz）',
    multiple: false,
    directory: false,
    filters: [{ name: '归档', extensions: ['tar.gz', 'tgz'] }],
  })
  if (!source || typeof source !== 'string') return

  importing.value = true
  const loadingId = toast.loading('正在导入…（会先停 daemon 再解包，重启后此处会刷新）')
  try {
    const r = await invoke<ImportResult>('import_db', { sourcePath: source })
    toast.dismiss(loadingId)
    toast.success(`导入完成（${r.files} 个文件）`, {
      description: `原数据已保留在：${r.before_path}`,
      duration: 12_000,
    })
    if (r.before_path) lastBackupPath.value = r.before_path
  } catch (e) {
    toast.dismiss(loadingId)
    toastBackendError('导入失败', e)
  } finally {
    importing.value = false
  }
}
</script>

<template>
  <div class="space-y-4">
    <Card>
      <CardHeader>
        <CardDescription>存储 · 备份 · 导入导出</CardDescription>
        <CardTitle class="text-base">本地 SQLite 数据库</CardTitle>
        <CardAction>
          <Badge variant="outline">本地</Badge>
        </CardAction>
      </CardHeader>
      <CardContent class="space-y-3 text-sm">
        <div class="flex items-center justify-between gap-3">
          <span class="shrink-0">数据库路径</span>
          <code class="truncate text-xs text-muted-foreground" :title="dbPath">{{ dbPath || '—' }}</code>
        </div>
        <div class="flex items-center justify-between">
          <span>会话数</span>
          <span class="font-medium tabular-nums">{{ sessionsTotal.toLocaleString() }}</span>
        </div>
        <div class="flex items-center justify-between">
          <span>消息数</span>
          <span class="font-medium tabular-nums">{{ messagesTotal.toLocaleString() }}</span>
        </div>
        <div class="flex items-center justify-between">
          <span>摘要数</span>
          <span class="font-medium tabular-nums">{{ summariesTotal.toLocaleString() }}</span>
        </div>

        <Separator />

        <!--
          一行小字把"备份 / 导出 / 导入"是同一种 .tar.gz codec 这件事直说，
          避免用户以为"立即备份"和"导出数据库"是两种格式 —— 底层都是
          memex backup / memex restore，跨机器也能搬。
        -->
        <div class="flex items-start gap-1.5 rounded-md bg-muted/40 px-2.5 py-2 text-[11.5px] text-muted-foreground">
          <Info class="mt-0.5 size-3.5 shrink-0" />
          <span>
            「立即备份」与「导出数据库」生成完全相同的
            <code class="font-mono">.tar.gz</code>。备份可拷到另一台机器后点「导入」恢复。
          </span>
        </div>

        <div class="flex items-center justify-between gap-3">
          <div class="min-w-0 flex-1">
            <Label class="text-sm">备份目录</Label>
            <code
              v-if="backupDir"
              class="mt-0.5 block truncate text-xs text-muted-foreground"
              :title="backupDir"
            >{{ backupDir }}</code>
            <p v-else class="text-xs text-muted-foreground">—</p>
          </div>
          <Button
            variant="ghost"
            size="sm"
            class="shrink-0 gap-1.5"
            :disabled="!backupDir || openingFolder"
            @click="onOpenBackupFolder"
          >
            <FolderOpen class="size-3.5" />
            打开
          </Button>
        </div>
      </CardContent>
      <CardFooter class="flex-wrap gap-2">
        <Button
          size="sm"
          variant="outline"
          class="gap-1.5"
          :disabled="backingUp"
          title="备份到 ~/.memex/backups/（自动命名）"
          @click="onBackupNow"
        >
          <FolderArchive :class="['size-3.5', backingUp && 'animate-pulse']" />
          {{ backingUp ? '备份中…' : '立即备份' }}
        </Button>
        <Button
          size="sm"
          variant="outline"
          :disabled="exporting"
          title="导出到任意位置（与备份产物完全等价）"
          @click="exportDb"
        >
          <Download :class="['mr-1.5 size-3.5', exporting && 'animate-pulse']" />
          {{ exporting ? '导出中…' : '导出到…' }}
        </Button>
        <Button
          size="sm"
          variant="outline"
          :disabled="importing"
          title="选择 .tar.gz 恢复（适用于备份或导出文件）"
          @click="importDb"
        >
          <Upload :class="['mr-1.5 size-3.5', importing && 'animate-pulse']" />
          {{ importing ? '导入中…' : '导入归档' }}
        </Button>
      </CardFooter>
    </Card>

    <Card>
      <CardHeader>
        <CardDescription class="text-destructive">危险区</CardDescription>
        <CardTitle class="text-base">重置数据</CardTitle>
      </CardHeader>
      <CardContent class="space-y-3 text-sm">
        <div class="flex items-center justify-between gap-3">
          <div class="flex-1">
            <Label class="text-sm">仅重建索引</Label>
            <p class="text-xs text-muted-foreground">
              清空 FTS5 索引并按现有数据库重建，不会删除会话
            </p>
          </div>
          <Button
            size="sm"
            variant="outline"
            class="gap-1.5 border-amber-500/50 text-amber-700 hover:bg-amber-500/10"
            :disabled="rebuilding"
            @click="rebuildIndex"
          >
            <RefreshCw :class="['size-3.5', rebuilding && 'animate-spin']" />
            {{ rebuilding ? '重建中…' : '重建索引' }}
          </Button>
        </div>
        <Separator />
        <div class="flex items-center justify-between gap-3">
          <div class="flex-1">
            <Label class="text-sm text-destructive">清空全部数据</Label>
            <p class="text-xs text-muted-foreground">
              删除所有会话、摘要和索引，此操作不可恢复
            </p>
          </div>
          <Button size="sm" variant="destructive" class="gap-1.5" :disabled="clearing" @click="clearAll">
            <Trash2 class="size-3.5" />
            {{ clearing ? '清空中…' : '清空全部' }}
          </Button>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
