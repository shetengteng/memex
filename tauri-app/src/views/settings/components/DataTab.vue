<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
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
  RefreshCw,
  Trash2,
  Upload,
} from 'lucide-vue-next'
import { toast } from 'vue-sonner'
import { useMemex } from '@/composables/useMemex'
import { stats } from '@/stores/memex'

const memex = useMemex()
const dbPath = ref<string>('')
const dataDir = ref<string>('')
const lastBackupPath = ref<string>('')
const rebuilding = ref(false)
const clearing = ref(false)
const backingUp = ref(false)
const openingFolder = ref(false)

interface BackupResult {
  path: string
  files: number
  size_bytes: number
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
            toast.error(`无法打开文件位置：${String(e)}`),
          )
        },
      },
    })
  } catch (e) {
    toast.error(`备份失败：${String(e)}`)
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
    toast.error(`无法打开备份目录：${String(e)}`)
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
    toast.error(`重建失败：${String(e)}`)
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
    toast.error(`清空失败：${String(e)}`)
  } finally {
    clearing.value = false
  }
}

function exportDb() {
  toast.message('导出功能即将提供')
}
function importDb() {
  toast.message('导入功能即将提供')
}
</script>

<template>
  <div class="space-y-4">
    <Card>
      <CardHeader>
        <CardDescription>存储</CardDescription>
        <CardTitle class="text-base">本地 SQLite 数据库</CardTitle>
        <CardAction>
          <Badge variant="outline">本地</Badge>
        </CardAction>
      </CardHeader>
      <CardContent class="space-y-3 text-sm">
        <div class="flex items-center justify-between">
          <span>数据库路径</span>
          <code class="text-xs text-muted-foreground">{{ dbPath || '—' }}</code>
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
      </CardContent>
      <CardFooter class="gap-2">
        <Button size="sm" variant="outline" @click="exportDb">
          <Download class="mr-1.5 size-3.5" />
          导出数据库
        </Button>
        <Button size="sm" variant="outline" @click="importDb">
          <Upload class="mr-1.5 size-3.5" />
          导入
        </Button>
      </CardFooter>
    </Card>

    <Card>
      <CardHeader>
        <CardDescription>备份</CardDescription>
        <CardTitle class="text-base">手动快照</CardTitle>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="flex items-center justify-between gap-3">
          <div class="min-w-0 flex-1">
            <Label class="text-sm">立即备份</Label>
            <p class="truncate text-xs text-muted-foreground">
              将 memex.db / config.toml / sessions/ 打包到 .tar.gz
            </p>
          </div>
          <Button
            variant="outline"
            size="sm"
            class="shrink-0 gap-1.5"
            :disabled="backingUp"
            @click="onBackupNow"
          >
            <FolderArchive :class="['size-3.5', backingUp && 'animate-pulse']" />
            {{ backingUp ? '备份中…' : '立即备份' }}
          </Button>
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
            variant="outline"
            size="sm"
            class="shrink-0 gap-1.5"
            :disabled="!backupDir || openingFolder"
            @click="onOpenBackupFolder"
          >
            <FolderOpen class="size-3.5" />
            打开目录
          </Button>
        </div>
      </CardContent>
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
