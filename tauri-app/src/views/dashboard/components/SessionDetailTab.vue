<script setup lang="ts">
import { ref } from 'vue'
import { ArrowLeft, Loader2, RefreshCw } from 'lucide-vue-next'
import type { SessionDetail } from '@/types'
import { timeAgo, adapterColor, adapterBg, adapterLabel } from '@/lib/utils'
import { useMemex } from '@/composables/useMemex'
import MarkdownContent from '@/components/MarkdownContent.vue'
import { Button } from '@/components/ui/button'

const props = defineProps<{
  session: SessionDetail | null
  loading: boolean
}>()

const emit = defineEmits<{
  back: []
  refreshSession: [sessionId: string]
}>()

const { retrySummary } = useMemex()
const summarizing = ref(false)
const summaryError = ref('')
const summarySuccess = ref(false)

async function handleRetrySummary() {
  if (!props.session) return
  summarizing.value = true
  summaryError.value = ''
  summarySuccess.value = false
  try {
    const ok = await retrySummary(props.session.id)
    if (ok) {
      summarySuccess.value = true
      emit('refreshSession', props.session.id)
    } else {
      summaryError.value = 'Session needs at least 2 messages'
    }
  } catch (e: unknown) {
    summaryError.value = e instanceof Error ? e.message : String(e)
  }
  summarizing.value = false
}
</script>

<template>
  <Button variant="ghost" size="sm" class="mb-4" @click="emit('back')">
    <ArrowLeft class="mr-1 h-3.5 w-3.5" /> Back to sessions
  </Button>
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
      <div class="mt-3 flex items-center gap-2">
        <span v-if="session.title" class="text-sm text-muted-foreground">
          Summary: <span class="font-medium text-foreground">{{ session.title }}</span>
        </span>
        <span v-else class="text-sm text-muted-foreground italic">No summary generated</span>
        <Button variant="outline" size="sm" class="ml-auto h-7 text-xs" :disabled="summarizing" @click="handleRetrySummary">
          <RefreshCw class="mr-1 h-3 w-3" :class="{ 'animate-spin': summarizing }" />
          {{ summarizing ? 'Generating...' : session.title ? 'Regenerate' : 'Generate' }} Summary
        </Button>
      </div>
      <p v-if="summarizing" class="mt-1 text-xs text-muted-foreground">Calling LLM to generate summary, this may take a few seconds...</p>
      <p v-else-if="summarySuccess" class="mt-1 text-xs text-green-500">Summary generated successfully!</p>
      <p v-if="summaryError" class="mt-1 text-xs text-destructive">{{ summaryError }}</p>
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
