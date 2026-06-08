<script setup lang="ts">
// 工具调用块的卡片渲染：标题（工具名）+ 折叠的 args / result。
// 视觉与 Cursor / Claude Code chat 中的工具调用样式一致：左侧轨道、单色 monospace、可展开 JSON。
import { computed, ref } from 'vue'
import { ChevronRight, Hammer } from 'lucide-vue-next'

const props = defineProps<{
  name: string
  args: string | null
  result: string | null
}>()

const argsOpen = ref(false)
const resultOpen = ref(false)

function formatJson(raw: string | null): string {
  if (!raw) return ''
  // Cursor 导出的 result 经常长这样：
  //   {"contents":" <!-- ... --> \n <main ...> \n …"}
  // 里头的 \n 是字面 backslash-n（两字节），并非真实换行。
  // 先尝试 JSON.parse 后 re-stringify 美化；失败就原样吐出来。
  try {
    const parsed = JSON.parse(raw)
    return JSON.stringify(parsed, null, 2)
  } catch {
    return raw
  }
}

// 摘要预览（折叠状态下显示的一小行）
const argsPreview = computed(() => previewOf(props.args))
const resultPreview = computed(() => previewOf(props.result))

function previewOf(raw: string | null): string {
  if (!raw) return ''
  const flat = raw.replace(/\s+/g, ' ').trim()
  return flat.length > 80 ? flat.slice(0, 80) + '…' : flat
}
</script>

<template>
  <div class="my-1.5 overflow-hidden rounded-md border border-border/60 bg-muted/30 text-[12px]">
    <!-- 标题栏 -->
    <div class="flex items-center gap-2 border-b border-border/40 bg-muted/50 px-2.5 py-1.5">
      <Hammer class="size-3 shrink-0 text-muted-foreground" />
      <span class="font-mono text-[11px] font-semibold tracking-tight text-foreground">
        {{ name }}
      </span>
      <span class="ml-auto text-[10px] uppercase tracking-wider text-muted-foreground/70">
        tool call
      </span>
    </div>

    <!-- args 行 -->
    <button
      v-if="args"
      type="button"
      class="group flex w-full items-start gap-2 px-2.5 py-1.5 text-left transition-colors hover:bg-muted/40"
      @click="argsOpen = !argsOpen"
    >
      <ChevronRight
        class="mt-0.5 size-3 shrink-0 text-muted-foreground transition-transform"
        :class="{ 'rotate-90': argsOpen }"
      />
      <span class="shrink-0 font-mono text-[10px] font-semibold uppercase tracking-wider text-muted-foreground/80">
        args
      </span>
      <span
        v-if="!argsOpen"
        class="min-w-0 flex-1 truncate font-mono text-[11px] text-muted-foreground/90"
      >
        {{ argsPreview }}
      </span>
    </button>
    <pre
      v-if="args && argsOpen"
      class="m-0 max-h-64 overflow-auto border-t border-border/40 bg-background/40 px-3 py-2 font-mono text-[11px] leading-relaxed"
    >{{ formatJson(args) }}</pre>

    <!-- result 行 -->
    <button
      v-if="result"
      type="button"
      class="group flex w-full items-start gap-2 border-t border-border/40 px-2.5 py-1.5 text-left transition-colors hover:bg-muted/40"
      @click="resultOpen = !resultOpen"
    >
      <ChevronRight
        class="mt-0.5 size-3 shrink-0 text-muted-foreground transition-transform"
        :class="{ 'rotate-90': resultOpen }"
      />
      <span class="shrink-0 font-mono text-[10px] font-semibold uppercase tracking-wider text-muted-foreground/80">
        result
      </span>
      <span
        v-if="!resultOpen"
        class="min-w-0 flex-1 truncate font-mono text-[11px] text-muted-foreground/90"
      >
        {{ resultPreview }}
      </span>
    </button>
    <pre
      v-if="result && resultOpen"
      class="m-0 max-h-80 overflow-auto border-t border-border/40 bg-background/40 px-3 py-2 font-mono text-[11px] leading-relaxed"
    >{{ formatJson(result) }}</pre>
  </div>
</template>
