<script setup lang="ts">
import { inject } from 'vue'
import { MessageSquare } from 'lucide-vue-next'
import type { SessionRow, ViewName } from '@/types'
import { timeAgo, adapterLabel, adapterColor } from '@/lib/utils'

defineProps<{ session: SessionRow }>()

const navigate = inject<(view: ViewName, id?: string) => void>('navigate')!
</script>

<template>
  <button
    class="flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-left transition-colors hover:bg-accent"
    @click="navigate('session', session.id)"
  >
    <span
      :class="[adapterColor(session.source), 'inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-[10px] font-semibold text-white']"
    >
      {{ adapterLabel(session.source).slice(0, 2) }}
    </span>

    <div class="min-w-0 flex-1">
      <p class="truncate text-sm font-medium leading-tight">
        {{ session.project_path?.split('/').pop() ?? session.id.slice(0, 16) }}
      </p>
      <p class="mt-0.5 flex items-center gap-1 text-xs text-muted-foreground">
        <MessageSquare class="h-3 w-3" />
        {{ session.message_count }}
        <span class="mx-0.5">·</span>
        {{ adapterLabel(session.source) }}
      </p>
    </div>

    <span class="shrink-0 text-xs text-muted-foreground">
      {{ timeAgo(session.updated_at) }}
    </span>
  </button>
</template>
