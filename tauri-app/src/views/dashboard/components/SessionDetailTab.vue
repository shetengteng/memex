<script setup lang="ts">
import { ArrowLeft, Loader2 } from 'lucide-vue-next'
import type { SessionDetail } from '@/types'
import { timeAgo, adapterColor, adapterBg, adapterLabel } from '@/lib/utils'
import MarkdownContent from '@/components/MarkdownContent.vue'

defineProps<{
  session: SessionDetail | null
  loading: boolean
}>()

const emit = defineEmits<{
  back: []
}>()
</script>

<template>
  <button class="mb-4 flex items-center gap-1 text-xs font-medium text-primary hover:underline" @click="emit('back')">
    <ArrowLeft class="h-3.5 w-3.5" /> Back to sessions
  </button>
  <div v-if="loading" class="flex items-center justify-center py-10">
    <Loader2 class="h-5 w-5 animate-spin text-muted-foreground" />
  </div>
  <template v-else-if="session">
    <div class="mb-5">
      <h2 class="text-lg font-bold">
        <span class="mr-2 inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-semibold" :class="[adapterBg(session.source), adapterColor(session.source)]">
          {{ adapterLabel(session.source) }}
        </span>
        {{ session.project_path?.split('/').pop() ?? session.id.slice(0, 16) }}
      </h2>
      <div class="mt-2 flex flex-wrap gap-4 text-xs text-muted-foreground">
        <span>ID: <code class="rounded bg-muted px-1 py-0.5 font-mono text-[10px]">{{ session.id }}</code></span>
        <span>{{ session.message_count }} messages</span>
        <span>Updated: {{ timeAgo(session.updated_at) }}</span>
      </div>
    </div>
    <div class="space-y-2">
      <div
        v-for="(m, i) in session.messages.slice(0, 100)"
        :key="i"
        class="rounded-lg p-3 text-xs leading-relaxed"
        :class="m.role === 'user' ? 'border-l-2 border-green-500 bg-green-500/5' : 'border-l-2 border-primary bg-primary/5'"
      >
        <div class="mb-1 flex items-center gap-2 text-[11px]">
          <span class="font-semibold" :class="m.role === 'user' ? 'text-green-500' : 'text-primary'">{{ m.role }}</span>
          <span v-if="m.timestamp" class="text-muted-foreground">{{ new Date(m.timestamp).toLocaleTimeString() }}</span>
        </div>
        <MarkdownContent :content="m.content.substring(0, 1200)" />
      </div>
      <div v-if="session.messages.length > 100" class="py-3 text-center text-xs text-muted-foreground">
        … and {{ session.messages.length - 100 }} more messages
      </div>
    </div>
  </template>
  <div v-else class="py-10 text-center text-sm text-muted-foreground">Session not found</div>
</template>
