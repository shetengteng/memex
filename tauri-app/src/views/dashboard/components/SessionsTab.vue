<script setup lang="ts">
import { Loader2 } from 'lucide-vue-next'
import type { SessionRow } from '@/types'
import { timeAgo, adapterColor, adapterBg, adapterLabel } from '@/lib/utils'

defineProps<{
  sessions: SessionRow[]
  loading: boolean
}>()

const emit = defineEmits<{
  openSession: [sessionId: string]
}>()
</script>

<template>
  <h2 class="mb-5 text-xl font-bold tracking-tight">Sessions</h2>
  <div v-if="loading" class="flex items-center justify-center py-10">
    <Loader2 class="h-5 w-5 animate-spin text-muted-foreground" />
  </div>
  <div v-else class="overflow-hidden rounded-lg border border-border">
    <table class="w-full text-sm">
      <thead>
        <tr class="border-b border-border bg-muted/50">
          <th class="px-4 py-2.5 text-left text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Source</th>
          <th class="px-4 py-2.5 text-left text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Project</th>
          <th class="px-4 py-2.5 text-left text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Messages</th>
          <th class="px-4 py-2.5 text-left text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Updated</th>
          <th class="px-4 py-2.5 text-left text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Session ID</th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="s in sessions"
          :key="s.id"
          class="cursor-pointer border-b border-border transition-colors hover:bg-accent"
          @click="emit('openSession', s.id)"
        >
          <td class="px-4 py-2.5">
            <span class="inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-semibold" :class="[adapterBg(s.source), adapterColor(s.source)]">
              {{ adapterLabel(s.source) }}
            </span>
          </td>
          <td class="max-w-[200px] truncate px-4 py-2.5 text-xs">{{ s.project_path?.split('/').pop() ?? '-' }}</td>
          <td class="px-4 py-2.5 text-xs">{{ s.message_count }}</td>
          <td class="px-4 py-2.5 text-xs text-muted-foreground">{{ timeAgo(s.updated_at) }}</td>
          <td class="px-4 py-2.5 font-mono text-[11px] text-muted-foreground">{{ s.id.slice(0, 12) }}…</td>
        </tr>
      </tbody>
    </table>
  </div>
  <div class="mt-3 text-center text-xs text-muted-foreground">Showing {{ sessions.length }} sessions</div>
</template>
