<script setup lang="ts">
import { computed } from 'vue'
import MarkdownIt from 'markdown-it'
// @ts-expect-error markdown-it-task-lists 没有官方 d.ts，作为 plugin 调用
import taskLists from 'markdown-it-task-lists'

const props = defineProps<{ source: string }>()

// Help 页面 markdown 来源全部是仓库内静态 .md（views/help/contents/*.md），
// 不是用户输入；html:true 安全打开（否则 .md 里的 `<kbd>⌘⇧M</kbd>` / `<mark>`
// 这类内联 HTML 会被丢弃，UI 上看到的就是不连贯的"快捷键 M"）。
const md = new MarkdownIt({
  html: true,
  linkify: true,
  breaks: false,
  typographer: false,
}).use(taskLists, { enabled: false, label: true })

const rendered = computed(() => md.render(props.source))
</script>

<!--
  排版完全依赖 @tailwindcss/typography 的 prose 体系（已在 style.css 全局
  启用）。基类 + size + dark mode 处理：
  - prose-sm：13.5px 起步，适合 Help Drawer 的紧凑布局
  - dark:prose-invert：自动反色（标题、正文、代码块）
  - max-w-none：撑满父容器宽度，不被 prose 默认 65ch 卡住
  - prose-pre / prose-code / prose-headings 等再做 shadcn flavor 微调（圆角、
    border、color 切换到 hsl(var(--*))）

  之所以选 prose 而不是继续手写 200 行 :deep() 选择器，是为了：
  1. 维护成本：升级 prose 即拿到 GFM / 表格 / 任务列表等开箱即用的所有改进
  2. 一致性：和外部生态（shadcn 文档站本身也是 prose 派系）视觉对齐
  3. 减少 sandboxing 风险：Vue scoped style + :deep() 选择器在生产 build 容易
     被 PostCSS 处理意外丢弃，而 prose 是直出工具类不依赖 scoped 透传
-->
<template>
  <article
    v-html="rendered"
    class="
      prose prose-sm max-w-none dark:prose-invert
      prose-headings:scroll-mt-24
      prose-headings:font-semibold
      prose-headings:tracking-tight
      prose-h1:text-2xl prose-h1:font-bold prose-h1:tracking-tight
      prose-h2:text-lg prose-h2:mt-8 prose-h2:mb-3 prose-h2:pb-2 prose-h2:border-b prose-h2:border-border
      prose-h3:text-base prose-h3:mt-6 prose-h3:mb-2
      prose-p:leading-relaxed
      prose-a:text-foreground prose-a:font-medium hover:prose-a:underline
      prose-strong:text-foreground prose-strong:font-semibold
      prose-code:rounded prose-code:bg-muted prose-code:px-1.5 prose-code:py-0.5 prose-code:font-medium prose-code:text-foreground prose-code:before:content-none prose-code:after:content-none prose-code:text-[0.85em]
      prose-pre:bg-muted prose-pre:border prose-pre:border-border prose-pre:rounded-lg prose-pre:text-foreground
      prose-pre:shadow-sm
      prose-blockquote:border-l-primary prose-blockquote:bg-muted/40 prose-blockquote:rounded-r-lg prose-blockquote:px-4 prose-blockquote:py-2 prose-blockquote:not-italic prose-blockquote:font-normal
      prose-blockquote:before:hidden prose-blockquote:after:hidden
      prose-table:border prose-table:border-border prose-table:rounded-lg prose-table:overflow-hidden
      prose-th:bg-muted prose-th:text-foreground prose-th:font-semibold
      prose-th:py-2 prose-th:px-3
      prose-td:py-2 prose-td:px-3 prose-td:border-border
      prose-tr:border-border
      prose-li:my-1
      prose-img:rounded-lg prose-img:border prose-img:border-border
      prose-hr:border-border
      help-md
    "
  />
</template>

<style scoped>
/* prose 没覆盖到的 GFM 元素：kbd / mark / 任务列表 checkbox。 */
.help-md :deep(kbd) {
  font-family: 'JetBrains Mono', ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 0.8em;
  font-weight: 600;
  background: hsl(var(--background));
  color: hsl(var(--foreground));
  border: 1px solid hsl(var(--border));
  border-bottom-width: 2px;
  border-radius: 5px;
  padding: 1px 6px;
  margin: 0 2px;
  vertical-align: baseline;
  white-space: nowrap;
  box-shadow: 0 1px 0 hsl(var(--border));
}

.help-md :deep(mark) {
  background: hsl(48 96% 53% / 0.35);
  color: inherit;
  padding: 1px 4px;
  border-radius: 3px;
}

.help-md :deep(.task-list-item) {
  list-style: none;
  margin-left: -1.4em;
  padding-left: 0;
}

.help-md :deep(.task-list-item input[type='checkbox']) {
  margin-right: 6px;
  vertical-align: middle;
  accent-color: hsl(var(--primary, var(--foreground)));
}

/* prose 默认 a:hover 加 underline，前面 hover:prose-a:underline 已声明；
   但 prose-pre 内的 a 不会有 hover——pre 内通常没链接，无需特殊处理。 */
</style>
