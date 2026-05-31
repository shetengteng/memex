<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { Clock, MessageSquare, FolderOpen, Copy, Check, User, Bot } from 'lucide-vue-next'
import type { SessionDetail } from '@/types'
import { useMemex } from '@/composables/useMemex'
import { adapterLabel, adapterColor, timeAgo } from '@/lib/utils'
import ViewHeader from '@/components/ViewHeader.vue'

const props = defineProps<{ sessionId: string }>()
const { getSession } = useMemex()

const detail = ref<SessionDetail | null>(null)
const loading = ref(true)
const copied = ref(false)

onMounted(async () => {
  try {
    detail.value = await getSession(props.sessionId)
  } catch (e) {
    console.error('Failed to load session:', e)
  } finally {
    loading.value = false
  }
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

function roleIcon(role: string) {
  return role === 'user' ? User : Bot
}
</script>

<template>
  <div class="flex h-full flex-col">
    <ViewHeader title="Session Detail" show-back />

    <div v-if="loading" class="flex-1 space-y-3 p-4">
      <div v-for="i in 3" :key="i" class="h-20 animate-pulse rounded-lg bg-muted" />
    </div>

    <div v-else-if="!detail" class="flex flex-1 flex-col items-center justify-center px-4">
      <FolderOpen class="h-10 w-10 text-muted-foreground" />
      <p class="mt-3 text-sm text-muted-foreground">Session not found.</p>
    </div>

    <template v-else>
      <div class="flex-1 overflow-y-auto">
        <!-- Session Meta -->
        <div class="border-b border-border px-4 py-3">
          <div class="flex items-center gap-2">
            <span
              :class="[adapterColor(detail.source), 'inline-flex h-6 w-6 items-center justify-center rounded text-[9px] font-semibold text-white']"
            >
              {{ adapterLabel(detail.source).slice(0, 2) }}
            </span>
            <span class="text-sm font-medium">{{ adapterLabel(detail.source) }}</span>
            <span v-if="detail.project_path" class="truncate text-xs text-muted-foreground">
              · {{ detail.project_path.split('/').pop() }}
            </span>
          </div>

          <div class="mt-2 flex flex-wrap gap-3 text-xs text-muted-foreground">
            <span class="flex items-center gap-1">
              <MessageSquare class="h-3 w-3" />
              {{ detail.message_count }} messages
            </span>
            <span class="flex items-center gap-1">
              <Clock class="h-3 w-3" />
              {{ timeAgo(detail.updated_at) }}
            </span>
          </div>
        </div>

        <!-- Messages -->
        <div class="space-y-1 p-3">
          <div
            v-for="msg in detail.messages"
            :key="msg.id"
            class="rounded-lg border border-border p-3"
          >
            <div class="mb-1.5 flex items-center gap-1.5">
              <component :is="roleIcon(msg.role)" class="h-3.5 w-3.5 text-muted-foreground" />
              <span class="text-xs font-medium capitalize">{{ msg.role }}</span>
              <span v-if="msg.timestamp" class="text-[10px] text-muted-foreground">
                {{ msg.timestamp }}
              </span>
            </div>
            <p class="whitespace-pre-wrap text-sm leading-relaxed">
              {{ msg.content.length > 2000 ? msg.content.slice(0, 2000) + '…' : msg.content }}
            </p>
          </div>
        </div>
      </div>

      <!-- Footer -->
      <div class="flex shrink-0 items-center gap-2 border-t border-border px-3 py-2">
        <button
          class="inline-flex h-7 items-center gap-1.5 rounded-md bg-secondary px-2.5 text-xs font-medium transition-colors hover:bg-accent"
          @click="copyContent"
        >
          <component :is="copied ? Check : Copy" class="h-3 w-3" />
          {{ copied ? 'Copied' : 'Copy All' }}
        </button>
        <span class="flex-1 text-right text-[10px] text-muted-foreground">
          {{ detail.messages.length }} messages
        </span>
      </div>
    </template>
  </div>
</template>
