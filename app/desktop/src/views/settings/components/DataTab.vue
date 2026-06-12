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
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import {
  AlertTriangle,
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
import { useI18n } from '@/i18n'

const memex = useMemex()
const { t } = useI18n()
const dbPath = ref<string>('')
const dataDir = ref<string>('')
const lastBackupPath = ref<string>('')
const rebuilding = ref(false)
const clearing = ref(false)
const backingUp = ref(false)
const openingFolder = ref(false)
const exporting = ref(false)
const importing = ref(false)

// 改用 shadcn Dialog 替代 window.confirm / window.prompt：
// macOS WKWebView 默认不会弹原生 confirm / prompt 框 —— confirm() 直接返回 true，
// prompt() 直接返回 null。clearAll() 用 prompt('DELETE') 因此永远走不到 IPC，
// 表现为"点击清空全部完全没反应"。统一改成 shadcn Dialog 后既可见、又跨平台一致。
const rebuildDialogOpen = ref(false)
const clearDialogOpen = ref(false)
const clearConfirmText = ref('')
const importDialogOpen = ref(false)
const pendingImportSource = ref<string | null>(null)

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
    toast.success(t('settings.data.toast.backup_done', { files: r.files, mb }), {
      description: r.path,
      duration: 10_000,
      action: {
        label: t('settings.data.toast.reveal'),
        onClick: () => {
          void revealItemInDir(r.path).catch((e) =>
            toastBackendError(t('settings.data.toast.reveal_failed'), e),
          )
        },
      },
    })
  } catch (e) {
    toastBackendError(t('settings.data.toast.backup_failed'), e)
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
    toastBackendError(t('settings.data.toast.open_backup_failed'), e)
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

function openRebuildDialog() {
  rebuildDialogOpen.value = true
}

async function confirmRebuildIndex() {
  rebuildDialogOpen.value = false
  rebuilding.value = true
  try {
    await memex.systemResetIndex()
    toast.success(t('settings.data.toast.rebuilt'))
  } catch (e) {
    toastBackendError(t('settings.data.toast.rebuild_failed'), e)
  } finally {
    rebuilding.value = false
  }
}

const clearConfirmValid = computed(() => clearConfirmText.value.trim() === 'DELETE')

function openClearDialog() {
  clearConfirmText.value = ''
  clearDialogOpen.value = true
}

async function confirmClearAll() {
  if (!clearConfirmValid.value) return
  clearDialogOpen.value = false
  clearConfirmText.value = ''
  clearing.value = true
  try {
    await memex.systemResetAll()
    toast.success(t('settings.data.toast.cleared'))
  } catch (e) {
    toastBackendError(t('settings.data.toast.clear_failed'), e)
  } finally {
    clearing.value = false
  }
}

async function exportDb() {
  if (exporting.value) return
  const defaultName = `memex-${new Date().toISOString().slice(0, 10).replace(/-/g, '')}.tar.gz`
  const target = await saveDialog({
    title: t('settings.data.dialog.export_title'),
    defaultPath: defaultName,
    filters: [{ name: t('settings.data.dialog.export_archive'), extensions: ['tar.gz', 'tgz'] }],
  })
  if (!target) return

  exporting.value = true
  const loadingId = toast.loading(t('settings.data.toast.exporting'))
  try {
    const r = await invoke<BackupResult>('export_db', { targetPath: target })
    const mb = (r.size_bytes / 1024 / 1024).toFixed(1)
    toast.dismiss(loadingId)
    toast.success(t('settings.data.toast.export_done', { files: r.files, mb }), {
      description: r.path,
      duration: 10_000,
      action: {
        label: t('settings.data.toast.reveal'),
        onClick: () => {
          void revealItemInDir(r.path).catch((e) =>
            toastBackendError(t('settings.data.toast.reveal_failed'), e),
          )
        },
      },
    })
  } catch (e) {
    toast.dismiss(loadingId)
    toastBackendError(t('settings.data.toast.export_failed'), e)
  } finally {
    exporting.value = false
  }
}

async function importDb() {
  if (importing.value) return

  // 调换顺序：先让用户选文件，再走 dialog 二次确认。这样文件选错可以直接取消，
  // 不用经历"先弹个 confirm 才打开文件选择"两步打断流。
  const source = await openDialog({
    title: t('settings.data.dialog.import_title'),
    multiple: false,
    directory: false,
    filters: [{ name: t('settings.data.dialog.export_archive'), extensions: ['tar.gz', 'tgz'] }],
  })
  if (!source || typeof source !== 'string') return

  pendingImportSource.value = source
  importDialogOpen.value = true
}

async function confirmImportDb() {
  const source = pendingImportSource.value
  if (!source) {
    importDialogOpen.value = false
    return
  }
  importDialogOpen.value = false
  importing.value = true
  const loadingId = toast.loading(t('settings.data.toast.importing'))
  try {
    const r = await invoke<ImportResult>('import_db', { sourcePath: source })
    toast.dismiss(loadingId)
    toast.success(t('settings.data.toast.import_done', { files: r.files }), {
      description: t('settings.data.toast.import_done_desc', { path: r.before_path }),
      duration: 12_000,
    })
    if (r.before_path) lastBackupPath.value = r.before_path
  } catch (e) {
    toast.dismiss(loadingId)
    toastBackendError(t('settings.data.toast.import_failed'), e)
  } finally {
    importing.value = false
    pendingImportSource.value = null
  }
}
</script>

<template>
  <div class="space-y-4">
    <Card>
      <CardHeader>
        <CardDescription>{{ t('settings.data.section') }}</CardDescription>
        <CardTitle class="text-base">{{ t('settings.data.title') }}</CardTitle>
        <CardAction>
          <Badge variant="outline">{{ t('settings.data.local_badge') }}</Badge>
        </CardAction>
      </CardHeader>
      <CardContent class="space-y-3 text-sm">
        <div class="flex items-center justify-between gap-3">
          <span class="shrink-0">{{ t('settings.data.db_path') }}</span>
          <code class="truncate text-xs text-muted-foreground" :title="dbPath">{{ dbPath || '—' }}</code>
        </div>
        <div class="flex items-center justify-between">
          <span>{{ t('settings.data.sessions_count') }}</span>
          <span class="font-medium tabular-nums">{{ sessionsTotal.toLocaleString() }}</span>
        </div>
        <div class="flex items-center justify-between">
          <span>{{ t('settings.data.messages_count') }}</span>
          <span class="font-medium tabular-nums">{{ messagesTotal.toLocaleString() }}</span>
        </div>
        <div class="flex items-center justify-between">
          <span>{{ t('settings.data.summaries_count') }}</span>
          <span class="font-medium tabular-nums">{{ summariesTotal.toLocaleString() }}</span>
        </div>

        <Separator />

        <!-- 一行小字把 "备份/导出/导入" 同源 .tar.gz codec 说清楚 -->
        <div class="flex items-start gap-1.5 rounded-md bg-muted/40 px-2.5 py-2 text-[11.5px] text-muted-foreground">
          <Info class="mt-0.5 size-3.5 shrink-0" />
          <span>{{ t('settings.data.archive_hint') }}</span>
        </div>

        <div class="flex items-center justify-between gap-3">
          <div class="min-w-0 flex-1">
            <Label class="text-sm">{{ t('settings.data.backup_dir') }}</Label>
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
            {{ t('settings.data.open_folder') }}
          </Button>
        </div>
      </CardContent>
      <CardFooter class="flex-wrap gap-2">
        <Button
          size="sm"
          variant="outline"
          class="gap-1.5"
          :disabled="backingUp"
          :title="t('settings.data.backup_tooltip')"
          @click="onBackupNow"
        >
          <FolderArchive :class="['size-3.5', backingUp && 'animate-pulse']" />
          {{ backingUp ? t('settings.data.backing_up') : t('settings.data.backup_now') }}
        </Button>
        <Button
          size="sm"
          variant="outline"
          :disabled="exporting"
          :title="t('settings.data.export_tooltip')"
          @click="exportDb"
        >
          <Download :class="['mr-1.5 size-3.5', exporting && 'animate-pulse']" />
          {{ exporting ? t('settings.data.exporting') : t('settings.data.export_now') }}
        </Button>
        <Button
          size="sm"
          variant="outline"
          :disabled="importing"
          :title="t('settings.data.import_tooltip')"
          @click="importDb"
        >
          <Upload :class="['mr-1.5 size-3.5', importing && 'animate-pulse']" />
          {{ importing ? t('settings.data.importing') : t('settings.data.import_now') }}
        </Button>
      </CardFooter>
    </Card>

    <Card>
      <CardHeader>
        <CardDescription class="text-destructive">{{ t('settings.data.danger_section') }}</CardDescription>
        <CardTitle class="text-base">{{ t('settings.data.danger_title') }}</CardTitle>
      </CardHeader>
      <CardContent class="space-y-3 text-sm">
        <div class="flex items-center justify-between gap-3">
          <div class="flex-1">
            <Label class="text-sm">{{ t('settings.data.rebuild.label') }}</Label>
            <p class="text-xs text-muted-foreground">
              {{ t('settings.data.rebuild.sub') }}
            </p>
          </div>
          <Button
            size="sm"
            variant="outline"
            class="gap-1.5 border-amber-500/50 text-amber-700 hover:bg-amber-500/10"
            :disabled="rebuilding"
            @click="openRebuildDialog"
          >
            <RefreshCw :class="['size-3.5', rebuilding && 'animate-spin']" />
            {{ rebuilding ? t('settings.data.rebuild.running') : t('settings.data.rebuild.btn') }}
          </Button>
        </div>
        <Separator />
        <div class="flex items-center justify-between gap-3">
          <div class="flex-1">
            <Label class="text-sm text-destructive">{{ t('settings.data.clear.label') }}</Label>
            <p class="text-xs text-muted-foreground">
              {{ t('settings.data.clear.sub') }}
            </p>
          </div>
          <Button size="sm" variant="destructive" class="gap-1.5" :disabled="clearing" @click="openClearDialog">
            <Trash2 class="size-3.5" />
            {{ clearing ? t('settings.data.clear.clearing') : t('settings.data.clear.btn') }}
          </Button>
        </div>
      </CardContent>
    </Card>

    <!--
      下面三个 Dialog 替代了原来的 window.confirm / window.prompt。
      Tauri 2 macOS 端 WKWebView 默认不弹原生对话框，否则点击会"看似无反应"。
    -->

    <Dialog v-model:open="rebuildDialogOpen">
      <DialogContent class="sm:max-w-[440px]">
        <DialogHeader>
          <DialogTitle class="flex items-center gap-2">
            <RefreshCw class="size-4 text-amber-500" />
            {{ t('settings.data.dialog.rebuild_title') }}
          </DialogTitle>
          <DialogDescription>
            {{ t('settings.data.dialog.rebuild_desc') }}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" size="sm" @click="rebuildDialogOpen = false">
            {{ t('settings.data.dialog.cancel') }}
          </Button>
          <Button size="sm" class="gap-1.5" @click="confirmRebuildIndex">
            <RefreshCw class="size-3.5" />
            {{ t('settings.data.dialog.rebuild_start') }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <Dialog v-model:open="clearDialogOpen">
      <DialogContent class="sm:max-w-[480px]">
        <DialogHeader>
          <DialogTitle class="flex items-center gap-2 text-destructive">
            <AlertTriangle class="size-4" />
            {{ t('settings.data.dialog.clear_title') }}
          </DialogTitle>
          <DialogDescription>
            {{ t('settings.data.dialog.clear_desc') }}
          </DialogDescription>
        </DialogHeader>
        <div class="space-y-2">
          <Label for="clear-confirm-input" class="text-xs text-muted-foreground">
            {{ t('settings.data.dialog.clear_hint') }}
          </Label>
          <Input
            id="clear-confirm-input"
            v-model="clearConfirmText"
            placeholder="DELETE"
            autocomplete="off"
            @keydown.enter.prevent="confirmClearAll"
          />
        </div>
        <DialogFooter>
          <Button variant="outline" size="sm" @click="clearDialogOpen = false">
            {{ t('settings.data.dialog.cancel') }}
          </Button>
          <Button
            variant="destructive"
            size="sm"
            class="gap-1.5"
            :disabled="!clearConfirmValid"
            @click="confirmClearAll"
          >
            <Trash2 class="size-3.5" />
            {{ t('settings.data.clear.btn') }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <Dialog v-model:open="importDialogOpen">
      <DialogContent class="sm:max-w-[480px]">
        <DialogHeader>
          <DialogTitle class="flex items-center gap-2">
            <Upload class="size-4 text-amber-500" />
            {{ t('settings.data.dialog.import_title2') }}
          </DialogTitle>
          <DialogDescription>
            {{ t('settings.data.dialog.import_desc') }}
          </DialogDescription>
        </DialogHeader>
        <div v-if="pendingImportSource" class="rounded-md bg-muted/40 px-3 py-2 text-[12px] text-muted-foreground">
          <span class="text-muted-foreground/70">{{ t('settings.data.dialog.archive_label') }}</span>
          <code class="break-all font-mono">{{ pendingImportSource }}</code>
        </div>
        <DialogFooter>
          <Button variant="outline" size="sm" @click="importDialogOpen = false">
            {{ t('settings.data.dialog.cancel') }}
          </Button>
          <Button size="sm" class="gap-1.5" @click="confirmImportDb">
            <Upload class="size-3.5" />
            {{ t('settings.data.dialog.import_start') }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
