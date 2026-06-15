<script setup lang="ts">
import { computed } from 'vue'
import MarkdownIt from 'markdown-it'
import DOMPurify, { type Config as DOMPurifyConfig } from 'dompurify'
// @ts-expect-error markdown-it-task-lists 没有官方 d.ts，作为 plugin 调用
import taskLists from 'markdown-it-task-lists'

const props = defineProps<{ content: string; maxLen?: number }>()

// 内容来源包含 AI 会话消息 / 本地 LLM 反思，理论上可能携带恶意 `<script>`（恶意 prompt
// 攻击的 AI 输出、或用户粘贴的代码）。Tauri webview 直接执行 JS 会被滥用 IPC，
// 所以走"html:true 让 markdown-it 输出富文本 + DOMPurify 白名单清洗"两步——
// 既能渲染 `<kbd>⌘K</kbd>` 这种正常的内联 HTML，又会把 `<script>/<iframe>/onerror=`
// 这类危险标签 / 属性剥掉。不要把 html 改回 false：那样 .md 里的 kbd/sub/mark
// 又会被静默丢弃。
const md = new MarkdownIt({
  html: true,
  linkify: true,
  breaks: true,
  typographer: false,
}).use(taskLists, { enabled: false, label: true })

// markdown-it-task-lists 会给 li/input 输出 class，DOMPurify 默认允许 class，
// 不需要 ADD_ATTR；只需把 input 加进白名单（DOMPurify 默认禁掉 input 防钓鱼）。
const SANITIZE_CONFIG: DOMPurifyConfig = {
  ADD_TAGS: ['input'],
  ADD_ATTR: ['type', 'checked', 'disabled'],
}

const rendered = computed(() => {
  let text = props.content
  if (props.maxLen && text.length > props.maxLen) {
    text = text.slice(0, props.maxLen) + '\n\n…'
  }
  const raw = md.render(text)
  return DOMPurify.sanitize(raw, SANITIZE_CONFIG)
})
</script>

<template>
  <div class="markdown-body" v-html="rendered" />
</template>

<style scoped>
.markdown-body {
  font-size: 12px;
  line-height: 1.65;
  word-break: break-word;
}
.markdown-body :deep(h1),
.markdown-body :deep(h2),
.markdown-body :deep(h3),
.markdown-body :deep(h4) {
  font-weight: 600;
  margin: 0.6em 0 0.3em;
  line-height: 1.3;
}
.markdown-body :deep(h1) { font-size: 1.15em; }
.markdown-body :deep(h2) { font-size: 1.05em; }
.markdown-body :deep(h3) { font-size: 1em; }
.markdown-body :deep(p) {
  margin: 0.55em 0;
}
.markdown-body :deep(ul),
.markdown-body :deep(ol) {
  margin: 0.55em 0;
  padding-left: 1.5em;
}
.markdown-body :deep(li) {
  margin: 0.25em 0;
}
.markdown-body :deep(code) {
  font-family: "JetBrains Mono", ui-monospace, SFMono-Regular, monospace;
  font-size: 0.9em;
  background: var(--color-muted, #f1f5f9);
  padding: 0.15em 0.35em;
  border-radius: 3px;
}
.markdown-body :deep(pre) {
  margin: 0.75em 0;
  padding: 0.9em 1em;
  background: var(--color-muted, #f1f5f9);
  border-radius: 6px;
  overflow-x: auto;
  font-size: 0.85em;
  line-height: 1.5;
}
.markdown-body :deep(pre code) {
  background: none;
  padding: 0;
}
.markdown-body :deep(blockquote) {
  margin: 0.4em 0;
  padding: 0.2em 0.8em;
  border-left: 3px solid var(--color-border, #e2e8f0);
  color: var(--color-muted-foreground, #64748b);
}
.markdown-body :deep(hr) {
  border: none;
  border-top: 1px dashed currentColor;
  opacity: 0.25;
  margin: 0.8em 0;
}
.markdown-body :deep(a) {
  color: var(--color-primary, #4f46e5);
  text-decoration: underline;
  text-decoration-color: transparent;
  transition: text-decoration-color 0.15s;
}
.markdown-body :deep(a:hover) {
  text-decoration-color: currentColor;
}
.markdown-body :deep(table) {
  border-collapse: collapse;
  margin: 0.5em 0;
  font-size: 0.9em;
  width: 100%;
}
.markdown-body :deep(th),
.markdown-body :deep(td) {
  border: 1px solid var(--color-border, #e2e8f0);
  padding: 0.3em 0.6em;
  text-align: left;
}
.markdown-body :deep(th) {
  background: var(--color-muted, #f1f5f9);
  font-weight: 600;
}
.markdown-body :deep(img) {
  max-width: 100%;
  border-radius: 4px;
}
.markdown-body :deep(kbd) {
  font-family: "JetBrains Mono", ui-monospace, SFMono-Regular, monospace;
  font-size: 0.85em;
  background: var(--color-muted, #f1f5f9);
  border: 1px solid var(--color-border, #e2e8f0);
  border-bottom-width: 2px;
  border-radius: 4px;
  padding: 0.05em 0.4em;
  margin: 0 0.1em;
  vertical-align: baseline;
  white-space: nowrap;
}
.markdown-body :deep(mark) {
  background: rgba(250, 204, 21, 0.35);
  color: inherit;
  padding: 0 0.15em;
  border-radius: 2px;
}
.markdown-body :deep(.task-list-item) {
  list-style: none;
  margin-left: -1.4em;
}
.markdown-body :deep(.task-list-item input[type="checkbox"]) {
  margin-right: 0.4em;
  vertical-align: middle;
}
</style>
