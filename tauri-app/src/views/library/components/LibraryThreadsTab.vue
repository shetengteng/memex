<script setup lang="ts">
/**
 * L5「线索（Threads）」视图入口 —— 卡片网格 + 详情侧栏。
 *
 * 本文件是 orchestrator：状态全部委托给 `useThreadsView` composable，UI 拆为
 * `ThreadsToolbar` / `ThreadCard` / `ThreadDetailSheet` / `ThreadDeleteDialog`
 * / `ThreadsEmptyHero` 五个子组件，遵循单组件 < 300 行规范。
 *
 * 设计依据：design/20260608-01-Memex-线索Threads重新设计-原型.html
 *   - 80% 浏览场景 → 卡片网格擅长（A 方案）
 *   - 20% 深挖单主题 → 右侧 Sheet 详情擅长（D 方案）
 *
 * 严格使用 shadcn-vue 组件：Card / Badge / Button / Input / Switch / Sheet /
 * Separator / Dialog；外层滚动用 `flex-1 min-h-0 overflow-y-auto` 避免 ScrollArea
 * 在 flex 链中失效。
 */
import { Plus } from 'lucide-vue-next'
import ThreadsToolbar from './threads/ThreadsToolbar.vue'
import ThreadCard from './threads/ThreadCard.vue'
import ThreadDetailSheet from './threads/ThreadDetailSheet.vue'
import ThreadDeleteDialog from './threads/ThreadDeleteDialog.vue'
import ThreadsEmptyHero from './threads/ThreadsEmptyHero.vue'
import { useThreadsView } from '../composables/useThreadsView'
import { rowToSession, type Session } from '@/stores/memex'
import type { SessionRow, ThreadRow } from '@/types'

const emit = defineEmits<{ open: [Session] }>()

const view = useThreadsView()

function onOpenSession(row: SessionRow) {
  // 不关 Sheet：Drawer 是另一个 Dialog 实例，会自然 portal 到 body 顶层叠加在
  // Sheet 之上。关掉 Drawer 后用户仍能继续在 Sheet 里点别的会话，不丢失上下文。
  emit('open', rowToSession(row))
}

function onDeleteFromCard(t: ThreadRow, e: MouseEvent) {
  view.requestDelete(t, e)
}

function onDeleteFromSheet(t: ThreadRow) {
  view.requestDelete(t)
}
</script>

<template>
  <div class="flex flex-1 min-h-0 flex-col overflow-hidden">
    <ThreadsToolbar
      :llm-query="view.llmQuery.value"
      :llm-searching="view.llmSearching.value"
      :auto-cluster="view.autoCluster.value"
      :regenerating="view.regenerating.value"
      :filter="view.filter.value"
      :filter-counts="view.filterCounts.value"
      @update:llm-query="(v: string) => (view.llmQuery.value = v)"
      @update:filter="(v) => (view.filter.value = v)"
      @search="view.onSearch"
      @set-auto-cluster="view.setAutoCluster"
      @regenerate="view.onRegenerate"
    />

    <!-- 卡片网格 / 空状态 -->
    <div class="flex-1 min-h-0 overflow-y-auto">
      <ThreadsEmptyHero
        v-if="view.threadsRef.length === 0"
        @apply="view.applySuggestion"
      />

      <div
        v-else-if="view.filteredThreads.value.length === 0"
        class="flex h-60 items-center justify-center px-6 text-center"
      >
        <p class="text-[12.5px] text-muted-foreground">
          当前筛选下没有线索。试试
          <button
            class="text-foreground underline-offset-2 hover:underline"
            @click="view.filter.value = 'all'"
          >
            查看全部
          </button>
          。
        </p>
      </div>

      <ul
        v-else
        class="grid grid-cols-1 gap-3 px-6 py-5 md:grid-cols-2 xl:grid-cols-3"
      >
        <ThreadCard
          v-for="t in view.filteredThreads.value"
          :key="t.id"
          :thread="t"
          @open="view.openThread"
          @delete="onDeleteFromCard"
        />

        <button
          type="button"
          class="flex min-h-[160px] flex-col items-center justify-center rounded-xl border border-dashed border-border bg-card/40 text-muted-foreground transition-all hover:border-foreground/40 hover:text-foreground"
          @click="view.focusSearch"
        >
          <Plus class="size-6" />
          <span class="mt-1.5 text-[12px]">检索一个新主题</span>
        </button>
      </ul>
    </div>

    <ThreadDetailSheet
      :open="view.sheetOpen.value"
      :thread="view.selectedThread.value"
      :sessions="view.detailSessions.value"
      :loading="view.detailLoading.value"
      @update:open="(v) => (view.sheetOpen.value = v)"
      @delete="onDeleteFromSheet"
      @open-session="onOpenSession"
    />

    <ThreadDeleteDialog
      :target="view.deleteTarget.value"
      :deleting="view.deleting.value"
      @confirm="view.confirmDelete"
      @cancel="view.cancelDelete"
    />
  </div>
</template>
