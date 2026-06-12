<script setup lang="ts">
/**
 * MCP 单次调用详情 Dialog。
 *
 * 触发：用户在「实时事件流」点击某一行 → 父组件传入选中的 `McpCallEntry`。
 * 关闭：传 `null` 给 `entry`，Dialog 自动隐藏。
 *
 * v3 起后端 schema 加了 `arguments_json` / `result_json` 两列，本组件因此从
 * "只展示 metadata" 升级为 "metadata + 完整调用上下文"。Payload 通过
 * `prettifyPayload` 做：1) `{"text":...}` 壳拆包；2) JSON 缩进美化；3) 截断
 * marker 检测。Parse 失败时退回 plain text，不会让对话框崩。
 */
import { computed, ref, watch } from 'vue'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { CheckCircle2, Copy, XCircle } from 'lucide-vue-next'
import type { McpCallEntry } from '@/types'
import { formatFullTime, formatRelative, prettifyPayload } from './mcp-format'

const props = defineProps<{
  entry: McpCallEntry | null
}>()

const emit = defineEmits<{ close: [] }>()

const occurredAt = computed(() => {
  if (props.entry == null) return null
  const d = new Date(props.entry.occurred_at)
  if (Number.isNaN(d.getTime())) return null
  return d
})

const fullTime = computed(() =>
  props.entry == null ? '—' : formatFullTime(props.entry.occurred_at),
)

const relativeTime = computed(() => {
  const d = occurredAt.value
  if (d == null) return null
  return formatRelative(d)
})

const latencyLabel = computed(() => {
  const v = props.entry?.latency_ms ?? 0
  if (v < 10) return `${v} ms`
  if (v < 1_000) return `${Math.round(v)} ms`
  return `${(v / 1_000).toFixed(2)} s`
})

const argsPretty = computed(() => prettifyPayload(props.entry?.arguments_json))
const resultPretty = computed(() => prettifyPayload(props.entry?.result_json))

// 复制按钮 2 秒态反馈。两个 section 独立计数避免一起亮。
const copiedKey = ref<'args' | 'result' | null>(null)
watch(
  () => props.entry,
  () => {
    copiedKey.value = null
  },
)

async function copyToClipboard(key: 'args' | 'result', text: string) {
  if (text === '') return
  try {
    await navigator.clipboard.writeText(text)
    copiedKey.value = key
    setTimeout(() => {
      if (copiedKey.value === key) copiedKey.value = null
    }, 2_000)
  } catch {
    // 剪贴板权限被拒：静默吞。用户可手动选中复制，行为退化但不阻塞。
  }
}
</script>

<template>
  <Dialog
    :open="entry !== null"
    @update:open="(v: boolean) => { if (!v) emit('close') }"
  >
    <DialogContent class="w-[92vw] !max-w-2xl max-h-[85vh] overflow-y-auto">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <CheckCircle2 v-if="entry?.success" class="size-4 text-emerald-500" />
          <XCircle v-else class="size-4 text-rose-500" />
          <code class="font-mono text-[14px]">{{ entry?.tool_name || '(unknown)' }}</code>
          <Badge
            v-if="entry?.success"
            variant="outline"
            class="border-emerald-500/40 text-emerald-600 dark:text-emerald-400"
          >
            成功
          </Badge>
          <Badge
            v-else
            variant="outline"
            class="border-rose-500/40 text-rose-600 dark:text-rose-400"
          >
            失败
          </Badge>
        </DialogTitle>
        <DialogDescription class="text-[11.5px]">
          来自 SQLite `mcp_call_log` 的单次工具调用记录
        </DialogDescription>
      </DialogHeader>

      <dl class="space-y-2.5 text-[12.5px]">
        <div class="grid grid-cols-[88px_1fr] items-baseline gap-2">
          <dt class="text-[11px] text-muted-foreground">发生时间</dt>
          <dd class="font-mono tabular-nums">
            {{ fullTime }}
            <span v-if="relativeTime" class="ml-2 text-[10.5px] text-muted-foreground">
              ({{ relativeTime }})
            </span>
          </dd>
        </div>
        <div class="grid grid-cols-[88px_1fr] items-baseline gap-2">
          <dt class="text-[11px] text-muted-foreground">延迟</dt>
          <dd class="font-mono tabular-nums">{{ latencyLabel }}</dd>
        </div>
        <div class="grid grid-cols-[88px_1fr] items-baseline gap-2">
          <dt class="text-[11px] text-muted-foreground">调用 ID</dt>
          <dd class="font-mono text-[11.5px] text-muted-foreground">#{{ entry?.id }}</dd>
        </div>
        <div
          v-if="entry?.error_message"
          class="grid grid-cols-[88px_1fr] items-start gap-2"
        >
          <dt class="text-[11px] text-muted-foreground">错误信息</dt>
          <dd>
            <!-- 不设 max-h：由 DialogContent 的 max-h-[85vh] + overflow-y-auto 统一接管滚动，
                 避免对话框内出现嵌套垂直滚动条（外层 + 内层 pre 两个）。仅 break-words +
                 whitespace-pre-wrap 防止单行过宽撑爆。 -->
            <pre
              class="whitespace-pre-wrap break-words rounded bg-muted/40 px-2 py-1.5 font-mono text-[11px] text-rose-600 dark:text-rose-400"
            >{{ entry.error_message }}</pre>
          </dd>
        </div>
      </dl>

      <!-- 调用参数 -->
      <section class="mt-2 space-y-1.5">
        <header class="flex items-center justify-between">
          <h3 class="flex items-center gap-2 text-[12px] font-medium">
            调用参数
            <Badge v-if="argsPretty.truncated" variant="outline" class="border-amber-500/40 text-amber-600 dark:text-amber-400">
              已截断
            </Badge>
          </h3>
          <Button
            v-if="!argsPretty.empty"
            type="button"
            variant="ghost"
            size="sm"
            class="h-6 px-1.5 text-[10.5px]"
            @click="copyToClipboard('args', entry?.arguments_json ?? '')"
          >
            <Copy class="size-3" />
            {{ copiedKey === 'args' ? '已复制' : '复制原始' }}
          </Button>
        </header>
        <pre
          class="whitespace-pre-wrap break-words rounded bg-muted/40 px-2 py-1.5 font-mono text-[11px] leading-[1.55]"
          :class="argsPretty.empty ? 'text-muted-foreground italic' : ''"
        >{{ argsPretty.display }}</pre>
      </section>

      <!-- 返回内容 -->
      <section class="mt-3 space-y-1.5">
        <header class="flex items-center justify-between">
          <h3 class="flex items-center gap-2 text-[12px] font-medium">
            返回内容
            <Badge v-if="resultPretty.truncated" variant="outline" class="border-amber-500/40 text-amber-600 dark:text-amber-400">
              已截断
            </Badge>
          </h3>
          <Button
            v-if="!resultPretty.empty"
            type="button"
            variant="ghost"
            size="sm"
            class="h-6 px-1.5 text-[10.5px]"
            @click="copyToClipboard('result', entry?.result_json ?? '')"
          >
            <Copy class="size-3" />
            {{ copiedKey === 'result' ? '已复制' : '复制原始' }}
          </Button>
        </header>
        <pre
          class="whitespace-pre-wrap break-words rounded bg-muted/40 px-2 py-1.5 font-mono text-[11px] leading-[1.55]"
          :class="[
            resultPretty.empty ? 'text-muted-foreground italic' : '',
            entry?.success === false && !resultPretty.empty ? 'text-rose-600 dark:text-rose-400' : '',
          ]"
        >{{ resultPretty.display }}</pre>
      </section>
    </DialogContent>
  </Dialog>
</template>
