<script setup lang="ts">
import { computed } from 'vue'
import MarkdownIt from 'markdown-it'

const props = defineProps<{ content: string; maxLen?: number }>()

const md = new MarkdownIt({
  html: false,
  linkify: true,
  breaks: true,
  typographer: false,
})

const rendered = computed(() => {
  let text = props.content
  if (props.maxLen && text.length > props.maxLen) {
    text = text.slice(0, props.maxLen) + '\n\n…'
  }
  return md.render(text)
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
</style>
