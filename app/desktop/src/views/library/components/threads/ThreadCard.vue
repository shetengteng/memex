<script setup lang="ts">
/**
 * 线索网格里的一张卡，shadcn-vue Card 组合。
 *
 * Card / CardContent 用 `flex-1` 拉伸 + CardFooter `mt-auto` 沉底，保证同行
 * 卡片高度对齐、底部统计信息基线整齐。
 */
import { computed } from 'vue'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardFooter, CardHeader } from '@/components/ui/card'
import { Clock, FolderGit2, Trash2 } from 'lucide-vue-next'
import type { ThreadRow } from '@/types'
import { adapterLabel, dateRangeFmt, lastProjectName, timeFmt } from '../../composables/threadsFormat'
import { useI18n } from '@/i18n'

defineProps<{ thread: ThreadRow }>()
const emit = defineEmits<{
  open: [ThreadRow]
  delete: [ThreadRow, MouseEvent]
}>()

const { t, locale } = useI18n()
// 给 formatter 用的 BCP-47 locale。i18n 的 'zh' / 'en' 不是 valid BCP-47，
// 这里映射成 'zh-CN' / 'en-US'。
const dateLocale = computed(() => (locale.value === 'en' ? 'en-US' : 'zh-CN'))
const daysSuffix = computed(() => (locale.value === 'en' ? 'days' : '天'))
</script>

<template>
  <Card
    size="sm"
    class="group/card relative h-full cursor-pointer transition-all hover:border-foreground/40 hover:shadow-sm"
    @click="emit('open', thread)"
  >
    <CardHeader class="px-4">
      <div class="flex items-start justify-between gap-2">
        <h3 class="line-clamp-2 text-[14px] font-semibold leading-snug">
          {{ thread.name }}
        </h3>
        <button
          type="button"
          :aria-label="t('library.threads.card.delete_aria')"
          class="flex size-6 shrink-0 items-center justify-center rounded text-muted-foreground opacity-0 transition-opacity hover:bg-destructive/10 hover:text-destructive group-hover/card:opacity-100 focus:opacity-100 focus:outline-none focus:ring-1 focus:ring-ring"
          @click="(e: MouseEvent) => emit('delete', thread, e)"
        >
          <Trash2 class="size-3.5" />
        </button>
      </div>
    </CardHeader>

    <CardContent class="flex-1 space-y-3 px-4">
      <p
        v-if="thread.summary"
        class="line-clamp-2 text-[12px] leading-relaxed text-muted-foreground"
      >
        {{ thread.summary }}
      </p>
      <p v-else class="text-[12px] italic text-muted-foreground/70">
        {{ t('library.threads.card.no_summary') }}
      </p>

      <div
        v-if="thread.firstSessionAt && thread.lastSessionAt"
        class="flex items-center gap-2 text-[10.5px] text-muted-foreground"
      >
        <Clock class="size-3 shrink-0" />
        <span class="tabular-nums">{{ dateRangeFmt(thread.firstSessionAt, thread.lastSessionAt, dateLocale, daysSuffix) }}</span>
      </div>

      <div
        v-if="(thread.projects?.length ?? 0) || (thread.adapters?.length ?? 0)"
        class="flex flex-wrap items-center gap-1.5"
      >
        <Badge
          v-for="p in (thread.projects ?? []).slice(0, 2)"
          :key="`p-${p}`"
          variant="secondary"
          class="gap-1 px-2 text-[10.5px]"
        >
          <FolderGit2 class="size-2.5" />
          {{ lastProjectName(p) }}
        </Badge>
        <Badge
          v-if="(thread.projects?.length ?? 0) > 2"
          variant="secondary"
          class="px-2 text-[10.5px]"
        >
          +{{ (thread.projects?.length ?? 0) - 2 }}
        </Badge>
        <Badge
          v-for="a in (thread.adapters ?? [])"
          :key="`a-${a}`"
          variant="outline"
          class="px-2 text-[10.5px]"
        >
          {{ adapterLabel(a) }}
        </Badge>
      </div>
    </CardContent>

    <CardFooter class="mt-auto flex items-center justify-between px-4 text-[10.5px] text-muted-foreground/80">
      <span class="tabular-nums">{{ t('library.threads.card.session_count', { n: thread.sessionCount }) }}</span>
      <span class="tabular-nums">{{ timeFmt(thread.updatedAt, dateLocale) }}</span>
    </CardFooter>
  </Card>
</template>
