<script setup lang="ts">
/**
 * 完全没有线索时显示的搜索引擎风格 hero。
 * 5 个建议词点击就触发关键词检索，省去用户构思 query 的成本。
 */
import { computed } from 'vue'
import { Badge } from '@/components/ui/badge'
import { Sparkles } from 'lucide-vue-next'
import { useI18n } from '@/i18n'

const { t } = useI18n()

const SUGGESTIONS = computed(() => [
  t('library.threads.suggestions.0'),
  t('library.threads.suggestions.1'),
  t('library.threads.suggestions.2'),
  t('library.threads.suggestions.3'),
  t('library.threads.suggestions.4'),
])

defineEmits<{ apply: [string] }>()
</script>

<template>
  <div class="flex h-full min-h-[60vh] items-center justify-center px-6 py-12">
    <div class="w-full max-w-lg text-center">
      <div class="mx-auto flex size-14 items-center justify-center rounded-full bg-primary/10">
        <Sparkles class="size-6 text-primary" />
      </div>
      <h3 class="mt-4 text-[16px] font-semibold">{{ t('library.threads.empty.title') }}</h3>
      <p class="mx-auto mt-2 max-w-md text-[12.5px] text-muted-foreground">
        {{ t('library.threads.empty.body') }}
      </p>
      <div class="mt-6 flex flex-wrap items-center justify-center gap-2">
        <Badge
          v-for="s in SUGGESTIONS"
          :key="s"
          variant="outline"
          class="cursor-pointer rounded-full px-3 py-1 text-[11.5px] hover:border-primary hover:text-foreground"
          @click="$emit('apply', s)"
        >
          {{ s }}
        </Badge>
      </div>
      <p class="mt-6 text-[11px] text-muted-foreground/70">
        {{ t('library.threads.empty.cluster_hint') }}<span class="mx-1 font-medium text-foreground">{{ t('library.threads.empty.cluster_link') }}</span>{{ t('library.threads.empty.cluster_suffix') }}
      </p>
    </div>
  </div>
</template>
