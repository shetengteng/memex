<script setup lang="ts">
import { inject } from 'vue'
import type { SearchResult, ViewName } from '@/types'
import { adapterLabel, adapterColor, timeAgo } from '@/lib/utils'

defineProps<{ result: SearchResult; index: number }>()

const navigate = inject<(view: ViewName, id?: string) => void>('navigate')!
</script>

<template>
  <button
    class="flex w-full gap-3 rounded-lg px-3 py-2.5 text-left transition-colors hover:bg-accent"
    @click="navigate('session', result.session_id)"
  >
    <span
      v-if="result.adapter"
      :class="[adapterColor(result.adapter), 'mt-0.5 inline-flex h-5 w-5 shrink-0 items-center justify-center rounded text-[8px] font-semibold text-white']"
    >
      {{ adapterLabel(result.adapter).slice(0, 2) }}
    </span>
    <span v-else class="mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center text-xs text-muted-foreground">
      {{ index + 1 }}
    </span>

    <div class="min-w-0 flex-1">
      <div class="flex items-center gap-1.5 text-xs text-muted-foreground">
        <span class="rounded bg-muted px-1 py-0.5 text-[10px] font-medium">{{ result.chunk_type }}</span>
        <span v-if="result.timestamp">· {{ timeAgo(result.timestamp) }}</span>
      </div>
      <p
        class="mt-1 line-clamp-2 text-sm leading-snug"
        v-html="result.snippet"
      />
    </div>
  </button>
</template>
