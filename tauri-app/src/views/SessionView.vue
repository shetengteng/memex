<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { Clock, MessageSquare, FolderOpen, Copy, Check } from 'lucide-vue-next'
import type { SearchResult } from '@/types'
import { useMemex } from '@/composables/useMemex'
import { adapterLabel, adapterColor } from '@/lib/utils'
import ViewHeader from '@/components/ViewHeader.vue'

const props = defineProps<{ sessionId: string }>()
const { searchMemex } = useMemex()

const chunks = ref<SearchResult[]>([])
const loading = ref(true)
const copied = ref(false)

onMounted(async () => {
  try {
    const all = await searchMemex('*', 100)
    chunks.value = all.filter((c) => c.session_id === props.sessionId)
  } catch (e) {
    console.error('Failed to load session:', e)
  } finally {
    loading.value = false
  }
})

const sessionSource = ref('')
onMounted(() => {
  const parts = props.sessionId.split('_')
  sessionSource.value = parts[0] ?? 'unknown'
})

async function copyContent() {
  const text = chunks.value.map((c) => c.content).join('\n\n---\n\n')
  await navigator.clipboard.writeText(text)
  copied.value = true
  setTimeout(() => (copied.value = false), 2000)
}
</script>

<template>
  <div class="flex h-full flex-col">
    <ViewHeader title="Session Detail" show-back />

    <div class="flex-1 overflow-y-auto">
      <!-- Session Meta -->
      <div class="border-b border-border px-4 py-3">
        <div class="flex items-center gap-2">
          <span
            :class="[adapterColor(sessionSource), 'inline-flex h-6 w-6 items-center justify-center rounded text-[9px] font-semibold text-white']"
          >
            {{ adapterLabel(sessionSource).slice(0, 2) }}
          </span>
          <span class="text-sm font-medium">{{ adapterLabel(sessionSource) }}</span>
        </div>

        <p class="mt-1.5 break-all text-xs text-muted-foreground">
          {{ sessionId }}
        </p>

        <div class="mt-2 flex gap-4 text-xs text-muted-foreground">
          <span class="flex items-center gap-1">
            <MessageSquare class="h-3 w-3" />
            {{ chunks.length }} chunks
          </span>
          <span class="flex items-center gap-1">
            <Clock class="h-3 w-3" />
            {{ sessionSource }}
          </span>
        </div>
      </div>

      <!-- Chunks -->
      <div v-if="loading" class="space-y-3 p-4">
        <div v-for="i in 3" :key="i" class="h-20 animate-pulse rounded-lg bg-muted" />
      </div>

      <div v-else-if="chunks.length === 0" class="px-4 py-12 text-center">
        <FolderOpen class="mx-auto h-8 w-8 text-muted-foreground" />
        <p class="mt-2 text-sm text-muted-foreground">No chunks found for this session.</p>
      </div>

      <div v-else class="space-y-2 p-4">
        <div
          v-for="chunk in chunks"
          :key="chunk.chunk_id"
          class="rounded-lg border border-border p-3"
        >
          <div class="mb-1.5 flex items-center gap-1.5">
            <span class="rounded bg-muted px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground">
              {{ chunk.chunk_type }}
            </span>
          </div>
          <p class="whitespace-pre-wrap text-sm leading-relaxed">{{ chunk.content }}</p>
        </div>
      </div>
    </div>

    <!-- Footer Actions -->
    <div class="flex shrink-0 items-center gap-2 border-t border-border px-3 py-2">
      <button
        class="inline-flex h-7 items-center gap-1.5 rounded-md bg-secondary px-2.5 text-xs font-medium transition-colors hover:bg-accent"
        @click="copyContent"
      >
        <component :is="copied ? Check : Copy" class="h-3 w-3" />
        {{ copied ? 'Copied' : 'Copy All' }}
      </button>
    </div>
  </div>
</template>
