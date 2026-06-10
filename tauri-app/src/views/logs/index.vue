<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { toast } from 'vue-sonner'
import { ArrowLeft, RefreshCw, Filter as FilterIcon, AlertTriangle } from 'lucide-vue-next'

const router = useRouter()
function goBackToSystemTab() {
  router.push({ path: '/settings', query: { tab: 'system' } })
}

interface DaemonLogFile {
  name: string
  path: string
  size: number
  modified_secs: number
}
interface DaemonLogRead {
  file: string
  lines: string[]
  total_lines_returned: number
  truncated: boolean
}

const files = ref<DaemonLogFile[]>([])
const activeFile = ref<string>('')
const tailLines = ref<number>(500)
const filterText = ref<string>('')
const autoRefresh = ref<boolean>(true)
const loading = ref<boolean>(false)
const lastError = ref<string>('')

const raw = ref<DaemonLogRead | null>(null)

const displayLines = computed<string[]>(() => {
  if (!raw.value) return []
  const q = filterText.value.trim().toLowerCase()
  if (!q) return raw.value.lines
  return raw.value.lines.filter((line) => line.toLowerCase().includes(q))
})

const fileOptions = computed(() =>
  files.value.map((f) => ({
    value: f.name,
    label: `${f.name}  ·  ${formatBytes(f.size)}`,
  })),
)

const tailOptions = [
  { value: 100, label: '最近 100 行' },
  { value: 500, label: '最近 500 行' },
  { value: 1000, label: '最近 1000 行' },
  { value: 5000, label: '最近 5000 行' },
]

function formatBytes(b: number): string {
  if (b < 1024) return `${b} B`
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`
  return `${(b / (1024 * 1024)).toFixed(1)} MB`
}

async function loadFiles() {
  try {
    files.value = await invoke<DaemonLogFile[]>('list_daemon_log_files')
    if (!files.value.length) {
      lastError.value = '日志目录为空。daemon 可能尚未启动或刚启动还没写入日志。'
      raw.value = null
      return
    }
    if (!activeFile.value || !files.value.find((f) => f.name === activeFile.value)) {
      activeFile.value = files.value[0].name
    }
  } catch (e) {
    lastError.value = `列出日志失败：${String(e)}`
    files.value = []
  }
}

async function loadContent() {
  if (!activeFile.value) return
  loading.value = true
  try {
    raw.value = await invoke<DaemonLogRead>('read_daemon_log', {
      fileName: activeFile.value,
      lines: tailLines.value,
    })
    lastError.value = ''
  } catch (e) {
    lastError.value = `读取日志失败：${String(e)}`
    raw.value = null
  } finally {
    loading.value = false
  }
}

async function refresh() {
  await loadFiles()
  await loadContent()
}

async function copyAll() {
  if (!raw.value) return
  try {
    await navigator.clipboard.writeText(raw.value.lines.join('\n'))
    toast.success('已复制日志到剪贴板')
  } catch (e) {
    toast.error(`复制失败：${String(e)}`)
  }
}

let timer: number | null = null
function startTimer() {
  stopTimer()
  if (!autoRefresh.value) return
  timer = window.setInterval(() => {
    void loadContent()
  }, 5000)
}
function stopTimer() {
  if (timer !== null) {
    window.clearInterval(timer)
    timer = null
  }
}

watch(autoRefresh, (v) => {
  if (v) startTimer()
  else stopTimer()
})
watch(activeFile, () => {
  void loadContent()
})
watch(tailLines, () => {
  void loadContent()
})

onMounted(async () => {
  await refresh()
  if (autoRefresh.value) startTimer()
})

onBeforeUnmount(() => {
  stopTimer()
})
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <div class="flex flex-wrap items-center gap-2 border-b px-4 py-3">
      <Button
        size="sm"
        variant="ghost"
        class="-ml-2 h-8 gap-1.5 text-[12px]"
        @click="goBackToSystemTab"
      >
        <ArrowLeft class="size-3.5" />
        返回系统
      </Button>
      <span class="h-4 w-px bg-border" aria-hidden="true" />
      <span class="text-[12px] font-medium text-muted-foreground">日志文件</span>
      <Select v-model="activeFile" :disabled="!files.length">
        <SelectTrigger class="h-8 w-[280px] text-[12px]">
          <SelectValue placeholder="选择日志文件" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem v-for="opt in fileOptions" :key="opt.value" :value="opt.value">
            {{ opt.label }}
          </SelectItem>
        </SelectContent>
      </Select>

      <Select v-model.number="tailLines">
        <SelectTrigger class="h-8 w-[140px] text-[12px]">
          <SelectValue placeholder="行数" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem v-for="opt in tailOptions" :key="opt.value" :value="opt.value">
            {{ opt.label }}
          </SelectItem>
        </SelectContent>
      </Select>

      <div class="relative">
        <FilterIcon class="absolute left-2 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
        <Input
          v-model="filterText"
          placeholder="过滤关键字"
          class="h-8 w-[220px] pl-7 text-[12px]"
        />
      </div>

      <div class="flex items-center gap-1.5 text-[12px] text-muted-foreground">
        <Switch v-model="autoRefresh" />
        <span>自动刷新</span>
      </div>

      <div class="ml-auto flex items-center gap-2">
        <Button size="sm" variant="ghost" class="gap-1.5" :disabled="!raw" @click="copyAll">
          复制全部
        </Button>
        <Button size="sm" variant="outline" class="gap-1.5" :disabled="loading" @click="refresh">
          <RefreshCw :class="['size-3.5', loading && 'animate-spin']" />
          刷新
        </Button>
      </div>
    </div>

    <div
      v-if="lastError"
      class="mx-4 mt-3 flex items-start gap-2 rounded-md border border-amber-500/40 bg-amber-500/10 p-3 text-[12px] text-amber-700"
    >
      <AlertTriangle class="mt-0.5 size-4 shrink-0" />
      <span>{{ lastError }}</span>
    </div>

    <div
      v-if="raw?.truncated"
      class="mx-4 mt-3 rounded-md border bg-muted/30 px-3 py-2 text-[11px] text-muted-foreground"
    >
      日志文件较大，只显示文件末尾约 8 MB 数据的最近 {{ tailLines }} 行；更早内容请用文件管理器打开 ~/.memex/logs/ 查看。
    </div>

    <div class="min-h-0 flex-1 overflow-auto bg-muted/20 p-4 font-mono text-[11px] leading-5 text-foreground/90">
      <pre v-if="displayLines.length" class="whitespace-pre-wrap break-all">{{ displayLines.join('\n') }}</pre>
      <div v-else-if="!loading && !lastError" class="text-muted-foreground">
        {{ filterText ? '没有匹配过滤条件的行。' : '暂无日志。' }}
      </div>
    </div>

    <div class="flex items-center justify-between border-t px-4 py-2 text-[11px] text-muted-foreground">
      <span>
        {{ raw ? `${raw.file} · 显示 ${displayLines.length}/${raw.total_lines_returned} 行` : '—' }}
      </span>
      <span>每 5 秒自动刷新</span>
    </div>
  </div>
</template>
