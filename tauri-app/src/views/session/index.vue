<script setup lang="ts">
import { ref, inject, onMounted } from 'vue'
import { ArrowLeft, Copy, Check, User, Bot } from 'lucide-vue-next'
import type { SessionDetail } from '@/types'
import { useMemex } from '@/composables/useMemex'
import { adapterAbbr, adapterColor, adapterBg, timeAgo, formatTime } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import MarkdownContent from '@/components/MarkdownContent.vue'

const props = defineProps<{ sessionId: string }>()
const back = inject<() => void>('back')!
const { getSession } = useMemex()

const detail = ref<SessionDetail | null>(null)
const loading = ref(true)
const copied = ref(false)

onMounted(async () => {
  try { detail.value = await getSession(props.sessionId) } catch { /* ignore */ }
  loading.value = false
})

async function copyContent() {
  if (!detail.value) return
  const text = detail.value.messages
    .map((m) => `## ${m.role} ${m.timestamp ? `· ${m.timestamp}` : ''}\n\n${m.content}`)
    .join('\n\n---\n\n')
  await navigator.clipboard.writeText(text)
  copied.value = true
  setTimeout(() => (copied.value = false), 2000)
}
</script>

<template>
  <div class="flex h-full flex-col">
    <!-- Header -->
    <div class="flex items-center gap-1.5 border-b border-border px-3 py-2">
      <Button variant="ghost" size="icon" class="h-6 w-6 shrink-0" @click="back">
        <ArrowLeft class="h-3.5 w-3.5" />
      </Button>
      <template v-if="detail">
        <span class="mono grid h-[18px] w-[22px] place-items-center rounded text-[8px] font-semibold" :class="[adapterBg(detail.source), adapterColor(detail.source)]">
          {{ adapterAbbr(detail.source) }}
        </span>
        <span class="truncate text-xs font-semibold">{{ detail.project_path?.split('/').pop() ?? detail.id.slice(0, 12) }}</span>
        <span class="mono shrink-0 text-[10px] text-muted-foreground">{{ detail.message_count }} msg · {{ timeAgo(detail.updated_at) }}</span>
      </template>
      <span class="flex-1" />
      <Button v-if="detail" variant="outline" size="xs" class="mono h-5 gap-1 px-1.5 text-[10px]" @click="copyContent">
        <component :is="copied ? Check : Copy" class="h-2.5 w-2.5" />
        {{ copied ? '✓' : 'Copy' }}
      </Button>
    </div>

    <!-- Messages -->
    <div class="flex-1 select-text overflow-y-auto">
      <div v-if="loading" class="py-10 text-center text-xs text-muted-foreground">加载中...</div>
      <div v-else-if="!detail" class="py-10 text-center text-xs text-muted-foreground">Session not found.</div>
      <template v-else>
        <div
          v-for="msg in detail.messages"
          :key="msg.id"
          class="border-b border-border/30 px-3.5 py-2.5"
        >
          <div class="mb-1.5 flex items-center gap-1.5">
            <span
              class="flex h-5 w-5 items-center justify-center rounded-full text-white"
              :class="msg.role === 'user' ? 'bg-primary' : 'bg-success'"
            >
              <User v-if="msg.role === 'user'" class="h-3 w-3" />
              <Bot v-else class="h-3 w-3" />
            </span>
            <span class="text-xs font-semibold" :class="msg.role === 'user' ? 'text-primary' : 'text-success'">
              {{ msg.role === 'user' ? 'User' : 'Assistant' }}
            </span>
            <span v-if="msg.timestamp" class="mono text-[10px] text-muted-foreground">
              {{ formatTime(msg.timestamp) }}
            </span>
          </div>
          <div class="pl-[26px]">
            <MarkdownContent :content="msg.content" :max-len="3000" />
          </div>
        </div>
      </template>
    </div>
  </div>
</template>
