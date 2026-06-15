<script setup lang="ts">
import { computed } from 'vue'
import MarkdownIt from 'markdown-it'

const props = defineProps<{ source: string }>()

// Help 页面的 markdown 渲染密度比 Library Drawer 高一档（标题更大、表格更宽），
// 所以单独配一个 instance，不复用 src/components/MarkdownContent.vue。
const md = new MarkdownIt({
  html: false,
  linkify: true,
  breaks: false,
  typographer: false,
})

const rendered = computed(() => md.render(props.source))
</script>

<template>
  <article class="help-md" v-html="rendered" />
</template>

<style scoped>
.help-md {
  color: hsl(var(--foreground));
  font-size: 13.5px;
  line-height: 1.65;
  word-break: break-word;
}

.help-md :deep(h1) {
  font-size: 22px;
  font-weight: 700;
  letter-spacing: -0.01em;
  margin: 0 0 8px;
}

.help-md :deep(h2) {
  font-size: 16px;
  font-weight: 600;
  margin: 24px 0 8px;
}

.help-md :deep(h2:first-of-type) {
  margin-top: 12px;
}

.help-md :deep(h3) {
  font-size: 14px;
  font-weight: 600;
  margin: 18px 0 6px;
}

.help-md :deep(p) {
  font-size: 13.5px;
  line-height: 1.7;
  color: hsl(var(--muted-foreground));
  margin: 8px 0;
}

.help-md :deep(p strong),
.help-md :deep(li strong) {
  color: hsl(var(--foreground));
}

.help-md :deep(ul),
.help-md :deep(ol) {
  font-size: 13.5px;
  line-height: 1.7;
  color: hsl(var(--muted-foreground));
  padding-left: 22px;
  margin: 8px 0;
}

.help-md :deep(ul) {
  list-style: disc;
}

.help-md :deep(ol) {
  list-style: decimal;
}

.help-md :deep(li) {
  margin: 3px 0;
}

.help-md :deep(li > ul),
.help-md :deep(li > ol) {
  margin: 3px 0;
}

.help-md :deep(code) {
  font-family: 'JetBrains Mono', ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 12px;
  background: hsl(var(--muted) / 0.7);
  border-radius: 4px;
  padding: 1px 5px;
}

.help-md :deep(pre) {
  font-family: 'JetBrains Mono', ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 12px;
  line-height: 1.55;
  background: hsl(var(--muted) / 0.55);
  border: 1px solid hsl(var(--border));
  border-radius: 8px;
  padding: 12px 14px;
  margin: 10px 0;
  overflow-x: auto;
}

.help-md :deep(pre code) {
  background: transparent;
  padding: 0;
  border-radius: 0;
}

.help-md :deep(blockquote) {
  border-left: 3px solid hsl(var(--border));
  padding: 4px 0 4px 14px;
  margin: 12px 0;
  font-size: 13px;
  color: hsl(var(--muted-foreground));
}

.help-md :deep(a) {
  color: hsl(var(--foreground));
  text-decoration: underline;
  text-underline-offset: 2px;
  text-decoration-color: hsl(var(--border));
  transition: text-decoration-color 0.15s;
}

.help-md :deep(a:hover) {
  text-decoration-color: hsl(var(--foreground));
}

.help-md :deep(hr) {
  border: none;
  border-top: 1px solid hsl(var(--border));
  margin: 22px 0;
}

.help-md :deep(table) {
  width: 100%;
  border-collapse: collapse;
  font-size: 12.5px;
  margin: 10px 0;
}

.help-md :deep(th),
.help-md :deep(td) {
  border-bottom: 1px solid hsl(var(--border));
  padding: 6px 10px;
  text-align: left;
  vertical-align: top;
}

.help-md :deep(th) {
  font-weight: 600;
  background: hsl(var(--muted) / 0.5);
}

.help-md :deep(img) {
  max-width: 100%;
  border-radius: 6px;
}
</style>
