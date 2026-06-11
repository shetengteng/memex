<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog'
import { VisuallyHidden } from 'reka-ui'
import IdeChip from '@/components/shell/IdeChip.vue'
import MessageContent from '@/components/MessageContent.vue'
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
const loadMoreSentinel = ref<HTMLElement | null>(null)
let observer: IntersectionObserver | null = null

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

// 判断 message 的 timestamp 是不是 session-level fallback（后端 COALESCE 后
// 所有消息的 timestamp 与 session.updated_at 完全相等）。这种情况下时间不是
// "每条 message 真实发送时间"，UI 加 "~" 前缀让用户知道这是估算。
const sessionFallbackTs = computed(() => {
  const msgs = detail.value?.messages ?? []
  if (msgs.length < 2) return null
  const first = msgs[0]?.timestamp
  if (!first) return null
  return msgs.every((m) => m.timestamp === first) ? first : null
})

function messageTimeLabel(ts: string): string {
  const formatted = new Date(ts).toLocaleString('zh-CN', {
    dateStyle: 'short',
    timeStyle: 'short',
  })
  return ts === sessionFallbackTs.value ? `~ ${formatted}` : formatted
}

const remainingCount = computed(() => {
  const total = detail.value?.messages?.length ?? 0
  return Math.max(0, total - visibleMessages.value.length)
})

function loadMore() {
  visibleCount.value += 50
}

// 滚动到底部 sentinel 时自动 loadMore，避免用户每次都点按钮
watch(loadMoreSentinel, (el) => {
  observer?.disconnect()
  if (!el) {
    observer = null
    return
  }
  observer = new IntersectionObserver(
    (entries) => {
      const hit = entries.some((e) => e.isIntersecting)
      if (hit && remainingCount.value > 0) loadMore()
    },
    { rootMargin: '120px' },
  )
  observer.observe(el)
})

onBeforeUnmount(() => {
  observer?.disconnect()
  observer = null
})
</script>

<template>
  <Dialog :open="open" @update:open="(v) => $emit('update:open', v)">
    <DialogContent
      class="flex h-[85vh] w-[92vw] !max-w-4xl flex-col gap-0 overflow-hidden p-0"
    >
      <!-- 让 reka-ui 的 a11y 不报缺失 title，正文里有自定义 header 时把官方 title 视觉隐藏 -->
      <VisuallyHidden>
        <DialogTitle>{{ session?.title ?? '会话详情' }}</DialogTitle>
        <DialogDescription>
          {{ session ? `${session.messages} 条消息` : '' }}
        </DialogDescription>
      </VisuallyHidden>

      <header v-if="session" class="flex items-start gap-3 border-b px-5 py-4 pr-12">
        <div class="min-w-0 flex-1">
          <div class="mb-1.5 flex items-center gap-2">
            <IdeChip :adapter="session.adapter" />
            <span class="text-[11px] text-muted-foreground">
              {{ session.project }} · {{ tFmt(session.startedAt) }}
            </span>
          </div>
          <h2 class="text-[16px] font-semibold leading-tight">{{ session.title }}</h2>
          <p class="mt-1 text-[12px] text-muted-foreground">
            {{ session.messages }} 条消息 · 会话摘要{{ session.l2Done ? '已生成' : '待生成' }}
          </p>
        </div>
      </header>

      <div v-if="session" class="flex-1 space-y-5 overflow-y-auto px-7 py-5">
        <p v-if="detailLoading" class="text-center text-[12px] text-muted-foreground">加载详情中…</p>

        <section v-if="detail?.intent">
          <div class="mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
            用户意图
          </div>
          <p class="whitespace-pre-line text-[13px] leading-relaxed text-muted-foreground">
            {{ detail.intent }}
          </p>
        </section>

        <section v-if="detail?.summary">
          <div class="mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
            会话摘要
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
          <!-- 消息之间用 divide-y 加细虚线：原来只有 space-y-5 间距，长会话扫读时不容易找到相邻消息边界 -->
          <div class="divide-y divide-dashed divide-border/60">
            <article
              v-for="(m, i) in visibleMessages"
              :key="m.id ?? i"
              class="grid grid-cols-[28px_1fr] gap-3 py-3 first:pt-0 last:pb-0"
            >
              <!-- 头像：user → primary 紫；assistant → emerald 绿；其他 (system/tool 等) → muted 灰圈 + Bot icon。
                   之前 m.role === 'user' 三元判断把 system 也归到 assistant 的绿色，造成展示上的歧义；
                   显式 switch 让用户一眼能区分 user / assistant / 系统消息。 -->
              <span
                class="grid size-7 shrink-0 place-items-center rounded-full text-white"
                :class="
                  m.role === 'user'
                    ? 'bg-primary'
                    : m.role === 'assistant'
                      ? 'bg-emerald-600'
                      : 'bg-muted-foreground/40'
                "
              >
                <UserIcon v-if="m.role === 'user'" class="size-3.5" />
                <Bot v-else class="size-3.5" />
              </span>
              <div class="min-w-0">
                <div class="mb-1 flex items-center gap-2 text-xs">
                  <span
                    class="font-semibold"
                    :class="
                      m.role === 'user'
                        ? 'text-primary'
                        : m.role === 'assistant'
                          ? 'text-emerald-600 dark:text-emerald-400'
                          : 'text-muted-foreground'
                    "
                  >
                    {{
                      m.role === 'user'
                        ? t('session.role.user')
                        : m.role === 'assistant'
                          ? t('session.role.assistant')
                          : m.role
                    }}
                  </span>
                  <!-- 后端在 message.timestamp 为 NULL 时退化为 session.updated_at
                       （cursor / continue_dev 没有 per-message timestamp），
                       这种情况下整条会话所有消息共享同一时间，前面加 ~ 提示是
                       会话级估算而非真实 message 时间。 -->
                  <span v-if="m.timestamp" class="text-muted-foreground">
                    {{ messageTimeLabel(m.timestamp) }}
                  </span>
                </div>
                <div class="text-sm leading-relaxed">
                  <MessageContent :content="m.content" />
                </div>
              </div>
            </article>
          </div>

          <div
            v-if="remainingCount > 0"
            ref="loadMoreSentinel"
            class="mt-6 flex justify-center"
          >
            <Button variant="ghost" size="sm" @click="loadMore">
              {{ t('session.load_more', { count: remainingCount }) }}
            </Button>
          </div>
        </section>

        <p v-if="!detailLoading && !detail" class="text-center text-[12px] italic text-muted-foreground">
          未找到会话详情
        </p>
      </div>
    </DialogContent>
  </Dialog>
</template>
