<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Sheet,
  SheetContent,
  SheetTitle,
  SheetDescription,
} from '@/components/ui/sheet'
import IdeChip from '@/components/shell/IdeChip.vue'
import MarkdownContent from '@/components/MarkdownContent.vue'
import { Bot, User as UserIcon } from 'lucide-vue-next'
import type { Session } from '@/stores/memex'
import type { SessionDetail } from '@/types'
import { useMemex } from '@/composables/useMemex'
import { useI18n } from '@/i18n'

const props = defineProps<{ session: Session | null; open: boolean }>()
defineEmits<{ 'update:open': [boolean] }>()

const memex = useMemex()
const { t } = useI18n()
const detail = ref<SessionDetail | null>(null)
const detailLoading = ref(false)
const visibleCount = ref(50)

watch(
  () => [props.open, props.session?.id] as const,
  async ([isOpen, id]) => {
    if (!isOpen || !id) {
      detail.value = null
      visibleCount.value = 50
      return
    }
    visibleCount.value = 50
    detailLoading.value = true
    try {
      detail.value = await memex.getSession(id)
    } catch (e) {
      console.warn('[LibrarySessionDrawer] getSession failed', e)
      detail.value = null
    } finally {
      detailLoading.value = false
    }
  },
  { immediate: true },
)

const tFmt = (iso: string) => {
  if (!iso) return '—'
  const d = new Date(iso)
  return d.toLocaleString('zh-CN', { dateStyle: 'short', timeStyle: 'short' })
}

const visibleMessages = computed(() =>
  detail.value?.messages?.slice(0, visibleCount.value) ?? [],
)

const remainingCount = computed(() => {
  const total = detail.value?.messages?.length ?? 0
  return Math.max(0, total - visibleMessages.value.length)
})

function loadMore() {
  visibleCount.value += 50
}
</script>

<template>
  <Sheet :open="open" @update:open="(v) => $emit('update:open', v)">
    <SheetContent
      class="flex w-full flex-col overflow-hidden p-0 data-[side=right]:sm:max-w-[640px] data-[side=right]:lg:max-w-[720px]"
    >
      <header v-if="session" class="flex items-start gap-3 border-b px-5 py-4">
        <div class="min-w-0 flex-1">
          <div class="mb-1.5 flex items-center gap-2">
            <IdeChip :adapter="session.adapter" />
            <span class="text-[11px] text-muted-foreground">
              {{ session.project }} · {{ tFmt(session.startedAt) }}
            </span>
          </div>
          <SheetTitle class="text-[16px] font-semibold leading-tight">{{ session.title }}</SheetTitle>
          <SheetDescription class="mt-1 text-[12px]">
            {{ session.messages }} 条消息 · L2 摘要
            {{ session.l2Done ? '已生成' : '待生成' }}
          </SheetDescription>
        </div>
      </header>

      <div v-if="session" class="flex-1 space-y-5 overflow-y-auto px-5 py-4">
        <p v-if="detailLoading" class="text-center text-[12px] text-muted-foreground">加载详情中…</p>

        <section v-if="detail?.summary">
          <div class="mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
            LLM 摘要（L2）
          </div>
          <p class="whitespace-pre-line text-[13px] leading-relaxed">{{ detail.summary }}</p>
        </section>

        <section v-if="detail?.topics?.length">
          <div class="mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
            主题
          </div>
          <div class="flex flex-wrap gap-1.5">
            <Badge v-for="topic in detail.topics" :key="topic" variant="secondary">{{ topic }}</Badge>
          </div>
        </section>

        <section v-if="detail?.decisions?.length">
          <div class="mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
            关键决策
          </div>
          <ul class="space-y-1.5 text-[13px]">
            <li v-for="d in detail.decisions" :key="d" class="flex gap-2">
              <span class="mt-2 size-1 shrink-0 rounded-full bg-primary" />
              <span>{{ d }}</span>
            </li>
          </ul>
        </section>

        <section v-if="visibleMessages.length">
          <div class="mb-3 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
            {{ t('session.messages') }}
          </div>
          <div class="space-y-5">
            <article
              v-for="(m, i) in visibleMessages"
              :key="m.id ?? i"
              class="grid grid-cols-[28px_1fr] gap-3"
            >
              <span
                class="grid size-7 place-items-center rounded-full text-white"
                :class="m.role === 'user' ? 'bg-primary' : 'bg-success'"
              >
                <UserIcon v-if="m.role === 'user'" class="size-3.5" />
                <Bot v-else class="size-3.5" />
              </span>
              <div class="min-w-0">
                <div class="mb-1 flex items-center gap-2 text-xs">
                  <span
                    class="font-semibold"
                    :class="m.role === 'user' ? 'text-primary' : 'text-success'"
                  >
                    {{ m.role === 'user' ? t('session.role.user') : t('session.role.assistant') }}
                  </span>
                  <span v-if="m.timestamp" class="text-muted-foreground">
                    {{ new Date(m.timestamp).toLocaleString() }}
                  </span>
                </div>
                <div class="text-sm leading-relaxed">
                  <MarkdownContent :content="m.content" />
                </div>
              </div>
            </article>
          </div>

          <div v-if="remainingCount > 0" class="mt-6 flex justify-center">
            <Button variant="ghost" size="sm" @click="loadMore">
              {{ t('session.load_more', { count: remainingCount }) }}
            </Button>
          </div>
        </section>

        <p v-if="!detailLoading && !detail" class="text-center text-[12px] italic text-muted-foreground">
          未找到会话详情
        </p>
      </div>
    </SheetContent>
  </Sheet>
</template>
