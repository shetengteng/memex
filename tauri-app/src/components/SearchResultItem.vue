<script setup lang="ts">
import { inject } from 'vue'
import { FileText } from 'lucide-vue-next'
import type { SearchResult, ViewName } from '@/types'

defineProps<{ result: SearchResult; index: number }>()

const navigate = inject<(view: ViewName, id?: string) => void>('navigate')!
</script>

<template>
  <button
    class="flex w-full gap-3 rounded-lg px-3 py-2.5 text-left transition-colors hover:bg-accent"
    @click="navigate('session', result.session_id)"
  >
    <span class="mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded text-xs font-medium text-muted-foreground">
      {{ index + 1 }}
    </span>

    <div class="min-w-0 flex-1">
      <div class="flex items-center gap-1.5">
        <FileText class="h-3.5 w-3.5 text-muted-foreground" />
        <span class="text-xs font-medium text-muted-foreground">{{ result.chunk_type }}</span>
        <span class="text-xs text-muted-foreground">·</span>
        <span class="truncate text-xs text-muted-foreground">
          {{ result.session_id.slice(0, 12) }}
        </span>
      </div>
      <p
        class="mt-1 line-clamp-2 text-sm leading-snug"
        v-html="result.snippet"
      />
    </div>
  </button>
</template>
