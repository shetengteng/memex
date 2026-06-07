<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
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
import { Switch } from '@/components/ui/switch'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Download, RefreshCw, Trash2, Upload } from 'lucide-vue-next'
import { toast } from 'vue-sonner'
import { useMemex } from '@/composables/useMemex'
import { stats } from '@/stores/memex'

const memex = useMemex()
const dbPath = ref<string>('')
const ready = ref(false)
const backupAuto = ref(true)
const backupRetentionDays = ref<'3' | '7' | '30'>('7')
const rebuilding = ref(false)
const clearing = ref(false)

const sessionsTotal = computed(() => stats.value?.sessions ?? 0)
const messagesTotal = computed(() => stats.value?.messages ?? 0)
const summariesTotal = computed(() => stats.value?.summaries ?? 0)

onMounted(async () => {
  try {
    const report = await memex.runDoctor()
    dbPath.value = report.data_dir ? `${report.data_dir}/memex.db` : ''
  } catch {
    /* ignore */
  }
  try {
    const auto = await memex.getConfig('backup.auto')
    if (auto != null) backupAuto.value = auto === 'true'
    const ret = await memex.getConfig('backup.retention_days')
    if (ret === '3' || ret === '7' || ret === '30') backupRetentionDays.value = ret
  } catch {
    /* ignore */
  }
  ready.value = true
})

watch(backupAuto, (v) => {
  if (ready.value) memex.setConfig('backup.auto', String(v)).catch(() => {})
})
watch(backupRetentionDays, (v) => {
  if (ready.value) memex.setConfig('backup.retention_days', String(v)).catch(() => {})
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
        <CardTitle class="text-base">自动快照</CardTitle>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="flex items-center justify-between">
          <div>
            <Label class="text-sm">每晚自动快照</Label>
            <p class="text-xs text-muted-foreground">
              保存于 ~/Library/Application Support/memex/backups
            </p>
          </div>
          <Switch v-model="backupAuto" />
        </div>
        <div class="flex items-center justify-between">
          <div>
            <Label class="text-sm">保留最近</Label>
            <p class="text-xs text-muted-foreground">更老的快照会自动清理</p>
          </div>
          <Select v-model="backupRetentionDays">
            <SelectTrigger class="w-32"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="3">3 天</SelectItem>
              <SelectItem value="7">7 天</SelectItem>
              <SelectItem value="30">30 天</SelectItem>
            </SelectContent>
          </Select>
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
