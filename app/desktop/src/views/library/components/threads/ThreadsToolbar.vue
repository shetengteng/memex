<script setup lang="ts">
/**
 * 线索视图顶部工具栏：搜索 + 自动聚类开关 + 全量聚类按钮 + 筛选 chips。
 *
 * 拆为子组件后此文件无业务逻辑——状态全部 v-model / props 从父传入，回调走 emit。
 */
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Separator } from '@/components/ui/separator'
import { Switch } from '@/components/ui/switch'
import { Loader2, RefreshCw, Wand2 } from 'lucide-vue-next'
import type { FilterKey } from '../../composables/useThreadsView'
import { useI18n } from '@/i18n'

const { t } = useI18n()

defineProps<{
  llmQuery: string
  llmSearching: boolean
  autoCluster: boolean
  regenerating: boolean
  filter: FilterKey
  filterCounts: { all: number; multi_project: number; recent_7d: number }
}>()

const emit = defineEmits<{
  'update:llmQuery': [string]
  'update:filter': [FilterKey]
  search: []
  setAutoCluster: [boolean]
  regenerate: []
}>()
</script>

<template>
  <section class="border-b border-border/60 px-6 py-3">
    <!-- Row 1: 搜索 + 控制 -->
    <div class="flex items-center gap-3">
      <form class="flex flex-1 items-center gap-2" @submit.prevent="emit('search')">
        <div class="relative flex-1">
          <Wand2
            class="pointer-events-none absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground"
          />
          <Input
            id="threads-search-input"
            :model-value="llmQuery"
            class="h-9 pl-9 text-[13px]"
            :placeholder="t('library.threads.search_placeholder')"
            :disabled="llmSearching"
            @update:model-value="(v) => emit('update:llmQuery', String(v))"
          />
        </div>
        <Button
          type="submit"
          size="sm"
          :disabled="llmSearching || !llmQuery.trim()"
          class="h-9"
        >
          <Loader2 v-if="llmSearching" class="size-3.5 animate-spin" />
          <Wand2 v-else class="size-3.5" />
          <span class="ml-1.5">{{ t('library.threads.action.search') }}</span>
        </Button>
      </form>

      <Separator orientation="vertical" class="!h-5 !self-center" />

      <div class="flex shrink-0 items-center gap-3 text-[12px]">
        <label class="flex cursor-pointer items-center gap-2 text-muted-foreground">
          <span>{{ t('library.threads.action.auto_cluster') }}</span>
          <Switch
            :model-value="autoCluster"
            size="sm"
            @update:model-value="(v: boolean) => emit('setAutoCluster', v)"
          />
        </label>
        <Button
          type="button"
          variant="outline"
          size="sm"
          :disabled="regenerating"
          class="h-9"
          @click="emit('regenerate')"
        >
          <Loader2 v-if="regenerating" class="size-3.5 animate-spin" />
          <RefreshCw v-else class="size-3.5" />
          <span class="ml-1.5">{{ t('library.threads.action.regenerate') }}</span>
        </Button>
      </div>
    </div>

    <!-- Row 2: 筛选 chips -->
    <div class="mt-2 flex flex-wrap items-center gap-2 text-[12px]">
      <Badge
        :variant="filter === 'all' ? 'default' : 'outline'"
        class="cursor-pointer rounded-full px-3 py-1 text-[12px]"
        @click="emit('update:filter', 'all')"
      >
        {{ t('library.threads.filter.all') }}
        <span class="ml-1 tabular-nums opacity-70">{{ filterCounts.all }}</span>
      </Badge>
      <Badge
        :variant="filter === 'multi_project' ? 'default' : 'outline'"
        class="cursor-pointer rounded-full px-3 py-1 text-[12px]"
        @click="emit('update:filter', 'multi_project')"
      >
        {{ t('library.threads.filter.multi_project') }}
        <span class="ml-1 tabular-nums opacity-70">{{ filterCounts.multi_project }}</span>
      </Badge>
      <Badge
        :variant="filter === 'recent_7d' ? 'default' : 'outline'"
        class="cursor-pointer rounded-full px-3 py-1 text-[12px]"
        @click="emit('update:filter', 'recent_7d')"
      >
        {{ t('library.threads.filter.recent_7d') }}
        <span class="ml-1 tabular-nums opacity-70">{{ filterCounts.recent_7d }}</span>
      </Badge>
    </div>
  </section>
</template>
