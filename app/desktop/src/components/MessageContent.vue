<script setup lang="ts">
// 把会话 detail 中的「工具调用块」从一坨转义字符串渲染成折叠卡片。
// 识别 Cursor / Claude Code / OpenCode 常见格式：
//   [tool: name]
//   args: {...JSON...}
//   result: {...JSON...}
import { computed } from 'vue'
import MarkdownContent from './MarkdownContent.vue'
import ToolCallBlock from './ToolCallBlock.vue'

const props = defineProps<{ content: string }>()

interface MarkdownSegment {
  kind: 'markdown'
  text: string
}
interface ToolSegment {
  kind: 'tool'
  name: string
  args: string | null
  result: string | null
  raw: string
}
type Segment = MarkdownSegment | ToolSegment

// 整段匹配：以 [tool: name] 开头，可选 args: ...、result: ... 行
// args/result 一行装下（即便里头是大 JSON，因为换行是 \n 转义），下一段以空行或下一个 [tool: 开头结束。
const TOOL_BLOCK_RE = /\[tool:\s*([^\]]+?)\]\s*\n(args:\s*(.+?))?(\s*\n?result:\s*(.+?))?(?=\n\s*\n|\n\[tool:|$)/gs

const segments = computed<Segment[]>(() => {
  const text = props.content ?? ''
  if (!text.includes('[tool:')) {
    return [{ kind: 'markdown', text }]
  }
  const out: Segment[] = []
  let lastIndex = 0
  for (const m of text.matchAll(TOOL_BLOCK_RE)) {
    const idx = m.index ?? 0
    if (idx > lastIndex) {
      out.push({ kind: 'markdown', text: text.slice(lastIndex, idx) })
    }
    out.push({
      kind: 'tool',
      name: (m[1] ?? '').trim(),
      args: (m[3] ?? '').trim() || null,
      result: (m[5] ?? '').trim() || null,
      raw: m[0],
    })
    lastIndex = idx + m[0].length
  }
  if (lastIndex < text.length) {
    out.push({ kind: 'markdown', text: text.slice(lastIndex) })
  }
  return out
})
</script>

<template>
  <div class="space-y-2">
    <template v-for="(seg, i) in segments" :key="i">
      <ToolCallBlock
        v-if="seg.kind === 'tool'"
        :name="seg.name"
        :args="seg.args"
        :result="seg.result"
      />
      <MarkdownContent v-else :content="seg.text" />
    </template>
  </div>
</template>
