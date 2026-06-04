<script setup lang="ts">
import { ref, inject, onMounted } from 'vue'
import { ArrowLeft, Copy, Check, User, Bot } from 'lucide-vue-next'
import type { SessionDetail } from '@/types'
import { useMemex } from '@/composables/useMemex'
import { useI18n } from '@/i18n'
import { timeAgo, formatTime } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import MarkdownContent from '@/components/MarkdownContent.vue'
import IdeIcon from '@/components/IdeIcon.vue'

const props = defineProps<{ sessionId: string }>()
const back = inject<() => void>('back')!
const { t } = useI18n()
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
    <div class="flex items-center gap-2 border-b border-border px-3 py-2.5">
      <Button variant="ghost" size="sm" class="gap-1.5 px-2" @click="back">
        <ArrowLeft class="h-4 w-4" />
        {{ t('common.back') }}
      </Button>
      <template v-if="detail">
        <span class="grid h-7 w-7 shrink-0 place-items-center">
          <IdeIcon :source="detail.source" class="h-5 w-5" />
        </span>
        <span class="truncate text-sm font-semibold">{{ detail.project_path?.split('/').pop() ?? detail.id.slice(0, 12) }}</span>
        <span class="shrink-0 text-xs text-muted-foreground">{{ detail.message_count }} · {{ timeAgo(detail.updated_at) }}</span>
      </template>
      <span class="flex-1" />
      <Button v-if="detail" variant="ghost" size="icon" class="h-8 w-8 shrink-0" @click="copyContent" :title="copied ? t('common.copied') : t('common.copy')">
        <component :is="copied ? Check : Copy" class="h-4 w-4" />
      </Button>
    </div>

    <!-- Messages -->
    <div class="flex-1 select-text overflow-y-auto">
      <div v-if="loading" class="py-10 text-center text-sm text-muted-foreground">{{ t('common.loading') }}</div>
      <div v-else-if="!detail" class="py-10 text-center text-sm text-muted-foreground">{{ t('session.not_found') }}</div>
      <template v-else>
        <div
          v-for="msg in detail.messages"
          :key="msg.id"
          class="border-b border-border/30 px-3.5 py-3"
        >
          <div class="mb-2 flex items-center gap-2">
            <span
              class="flex h-6 w-6 items-center justify-center rounded-full text-white"
              :class="msg.role === 'user' ? 'bg-primary' : 'bg-success'"
            >
              <User v-if="msg.role === 'user'" class="h-3.5 w-3.5" />
              <Bot v-else class="h-3.5 w-3.5" />
            </span>
            <span class="text-sm font-semibold" :class="msg.role === 'user' ? 'text-primary' : 'text-success'">
              {{ msg.role === 'user' ? t('session.role.user') : t('session.role.assistant') }}
            </span>
            <span v-if="msg.timestamp" class="text-xs text-muted-foreground">
              {{ formatTime(msg.timestamp) }}
            </span>
          </div>
          <div class="pl-8 text-sm leading-relaxed">
            <MarkdownContent :content="msg.content" :max-len="3000" />
          </div>
        </div>
      </template>
    </div>
  </div>
</template>
