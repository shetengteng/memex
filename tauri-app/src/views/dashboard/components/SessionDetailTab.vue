<script setup lang="ts">
import { ref, computed } from 'vue'
import { ArrowLeft, Loader2, RefreshCw, Copy, Check, Bot, User as UserIcon } from 'lucide-vue-next'
import type { SessionDetail } from '@/types'
import { timeAgo, adapterColor, adapterBg, adapterLabel } from '@/lib/utils'
import { useMemex } from '@/composables/useMemex'
import { useI18n } from '@/i18n'
import MarkdownContent from '@/components/MarkdownContent.vue'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import { Badge } from '@/components/ui/badge'

const { t } = useI18n()

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
const copied = ref(false)
const visibleCount = ref(50)

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
      summaryError.value = t('session.summary.fail_short')
    }
  } catch (e: unknown) {
    summaryError.value = e instanceof Error ? e.message : String(e)
  }
  summarizing.value = false
}

async function copySession() {
  if (!props.session) return
  const lines: string[] = []
  lines.push(`# ${projectName.value}`)
  lines.push(`source: ${adapterLabel(props.session.source)}  ·  id: ${props.session.id}`)
  if (props.session.title) {
    lines.push('')
    lines.push(`${t('session.summary')}: ${props.session.title}`)
  }
  lines.push('')
  for (const m of props.session.messages) {
    const ts = m.timestamp ? ` · ${new Date(m.timestamp).toLocaleString()}` : ''
    const role = m.role === 'user' ? t('session.role.user') : t('session.role.assistant')
    lines.push(`## ${role}${ts}`)
    lines.push('')
    lines.push(m.content)
    lines.push('')
  }
  await navigator.clipboard.writeText(lines.join('\n'))
  copied.value = true
  setTimeout(() => (copied.value = false), 2000)
}

const projectName = computed(() => {
  const p = props.session?.project_path
  if (p) return p.split('/').filter(Boolean).pop() ?? p
  return props.session?.id.slice(0, 16) ?? ''
})

const visibleMessages = computed(() => props.session?.messages.slice(0, visibleCount.value) ?? [])

function loadMore() {
  visibleCount.value += 50
}
</script>

<template>
  <Button variant="ghost" size="sm" class="mb-4 gap-1.5 -ml-2 text-muted-foreground" @click="emit('back')">
    <ArrowLeft class="h-4 w-4" />
    {{ t('session.back') }}
  </Button>

  <div v-if="loading" class="flex items-center justify-center py-16">
    <Loader2 class="h-5 w-5 animate-spin text-muted-foreground" />
  </div>

  <template v-else-if="session">
    <!-- 标题区 -->
    <header class="mb-5">
      <div class="flex items-start justify-between gap-3">
        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-2 text-xs text-muted-foreground">
            <Badge variant="outline" :class="[adapterBg(session.source), adapterColor(session.source)]" class="border-transparent">
              {{ adapterLabel(session.source) }}
            </Badge>
            <span class="truncate font-mono text-[11px]">{{ session.id }}</span>
          </div>
          <h2 class="mt-1.5 truncate text-xl font-bold tracking-tight">{{ projectName }}</h2>
        </div>
        <div class="flex shrink-0 items-center gap-1.5">
          <Button variant="ghost" size="sm" class="h-8 gap-1.5" :disabled="copied" @click="copySession">
            <component :is="copied ? Check : Copy" class="h-3.5 w-3.5" />
            {{ copied ? t('common.copied') : t('common.copy') }}
          </Button>
        </div>
      </div>

      <!-- KPI 行 -->
      <dl class="mt-4 grid grid-cols-[auto_1fr] gap-x-6 gap-y-1.5 text-sm">
        <dt class="text-muted-foreground">{{ t('session.kpi.messages') }}</dt>
        <dd class="tabular-nums">{{ t('session.kpi.messages_unit', { count: session.message_count }) }}</dd>
        <dt class="text-muted-foreground">{{ t('session.kpi.updated') }}</dt>
        <dd>{{ timeAgo(session.updated_at) }}</dd>
        <template v-if="session.project_path">
          <dt class="text-muted-foreground">{{ t('session.kpi.path') }}</dt>
          <dd class="truncate font-mono text-xs">{{ session.project_path }}</dd>
        </template>
      </dl>
    </header>

    <!-- 摘要卡 -->
    <section class="mb-6 rounded-lg border border-border bg-card/50 p-4">
      <div class="mb-2 flex items-baseline justify-between gap-3">
        <h3 class="text-sm font-semibold">{{ t('session.summary') }}</h3>
        <Button variant="outline" size="sm" class="h-7 gap-1.5 text-xs" :disabled="summarizing" @click="handleRetrySummary">
          <RefreshCw class="h-3 w-3" :class="{ 'animate-spin': summarizing }" />
          {{ summarizing ? t('session.summary.generating') : session.title ? t('session.summary.regenerate') : t('session.summary.generate') }}
        </Button>
      </div>
      <div v-if="session.title" class="text-sm">
        <p class="font-medium">{{ session.title }}</p>
        <p v-if="session.summary" class="mt-2 text-sm leading-relaxed text-muted-foreground">{{ session.summary }}</p>
        <div v-if="session.topics?.length" class="mt-3 flex flex-wrap gap-1.5">
          <Badge v-for="topic in session.topics" :key="topic" variant="secondary">{{ topic }}</Badge>
        </div>
        <div v-if="session.decisions?.length" class="mt-3">
          <div class="mb-1 text-xs font-medium text-muted-foreground">{{ t('session.decisions') }}</div>
          <ul class="space-y-1 text-sm">
            <li v-for="(d, i) in session.decisions" :key="i" class="flex gap-2">
              <span class="mt-1.5 h-1 w-1 shrink-0 rounded-full bg-primary" />
              <span>{{ d }}</span>
            </li>
          </ul>
        </div>
      </div>
      <p v-else class="text-sm text-muted-foreground">{{ t('session.summary.empty') }}</p>
      <p v-if="summaryError" class="mt-2 text-xs text-destructive">{{ summaryError }}</p>
      <p v-else-if="summarySuccess" class="mt-2 text-xs text-success">{{ t('session.summary.success') }}</p>
      <p v-else-if="summarizing" class="mt-2 text-xs text-muted-foreground">{{ t('session.summary.calling') }}</p>
    </section>

    <Separator class="mb-4" />

    <!-- 消息时间线 -->
    <section>
      <h3 class="mb-3 text-sm font-semibold">{{ t('session.messages') }}</h3>
      <div class="space-y-5">
        <article v-for="(m, i) in visibleMessages" :key="i" class="grid grid-cols-[28px_1fr] gap-3">
          <span
            class="grid h-7 w-7 place-items-center rounded-full text-white"
            :class="m.role === 'user' ? 'bg-primary' : 'bg-success'"
          >
            <UserIcon v-if="m.role === 'user'" class="h-3.5 w-3.5" />
            <Bot v-else class="h-3.5 w-3.5" />
          </span>
          <div class="min-w-0">
            <div class="mb-1 flex items-center gap-2 text-xs">
              <span class="font-semibold" :class="m.role === 'user' ? 'text-primary' : 'text-success'">
                {{ m.role === 'user' ? t('session.role.user') : t('session.role.assistant') }}
              </span>
              <span v-if="m.timestamp" class="text-muted-foreground">{{ new Date(m.timestamp).toLocaleString() }}</span>
            </div>
            <div class="text-sm leading-relaxed">
              <MarkdownContent :content="m.content" :max-len="3000" />
            </div>
          </div>
        </article>
      </div>

      <div v-if="session.messages.length > visibleMessages.length" class="mt-6 flex flex-col items-center gap-2">
        <Button variant="outline" size="sm" @click="loadMore">
          {{ t('session.load_more', { count: session.messages.length - visibleMessages.length }) }}
        </Button>
      </div>
    </section>
  </template>

  <div v-else class="py-16 text-center text-sm text-muted-foreground">{{ t('session.not_found') }}</div>
</template>
