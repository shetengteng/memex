<script setup lang="ts">
import { Badge } from '@/components/ui/badge'
import IdeChip from '@/components/shell/IdeChip.vue'
import {
  Check,
  ChevronRight,
  Clock,
  Loader2,
  Lock,
  LockOpen,
  MessageCircle,
  RefreshCw,
} from 'lucide-vue-next'
import type { Session } from '@/stores/memex'
import { useI18n } from '@/i18n'

defineProps<{
  session: Session
  groupKey: string
  active: boolean
  /// 父组件追踪的"该 session 正在生成摘要"标志，用于禁用 badge 点击 + 显示 spinner。
  summarizing?: boolean
}>()
defineEmits<{
  open: [Session]
  'toggle-private': [Session]
  /// 用户点击列表行的"未摘要"或"已摘要"badge 时触发。
  /// 父组件统一调 retry_summary IPC——它对"无现有 L2 行"也安全（best-effort delete + 生成）。
  summarize: [Session]
}>()

const { t, locale } = useI18n()

const groupFmt = (iso: string, group: string) => {
  const d = new Date(iso)
  const hmLocale = locale.value === 'en' ? 'en-US' : 'zh-CN'
  const hm = d.toLocaleTimeString(hmLocale, {
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  })
  if (group === 'today' || group === 'yesterday') return hm
  if (group === 'week') {
    const day = t(`library.list.weekday.${d.getDay()}`)
    return `${day} ${hm}`
  }
  return `${d.getMonth() + 1}/${d.getDate()} ${hm}`
}
</script>

<template>
  <button
    :data-active="active"
    class="group relative flex w-full items-start border-b border-border/60 py-3.5 pl-5 pr-5 text-left transition-colors last:border-b-0 hover:bg-accent/40 data-[active=true]:bg-accent/40"
    :class="!session.l2Done && session.messages === 0 && 'opacity-60'"
    @click="$emit('open', session)"
  >
    <div class="min-w-0 flex-1">
      <!-- 第 1 行：标题 + IdeChip。 -->
      <div class="mb-1 flex items-center justify-between gap-3">
        <span class="truncate text-[14px] font-semibold tracking-tight">{{ session.title }}</span>
        <IdeChip class="shrink-0" :adapter="session.adapter" />
      </div>

      <!-- 第 2 行：intent。intent 为空时整行 v-if 折叠，不再被锁占位。 -->
      <p
        v-if="session.intent && session.intent.trim()"
        class="mb-2 truncate text-[12.5px] text-muted-foreground/90"
      >
        {{ session.intent }}
      </p>

      <!--
        第 3 行：锁（常驻，行首）+ 消息数 / 时长 / 摘要状态(可点击) / topics / 项目 · 时间
        - 锁挪到第三行开头：与下方 badges 同基线，避免单独占行造成的视觉空白。
        - 未私有 = 灰 + LockOpen；私有 = 红 + Lock。
      -->
      <div class="flex items-center gap-2">
        <div class="flex min-w-0 flex-1 flex-wrap items-center gap-1.5">
          <span
            role="button"
            tabindex="0"
            class="inline-flex size-5 shrink-0 items-center justify-center rounded transition-colors"
            :class="session.isPrivate
              ? 'text-red-600 hover:bg-red-500/10 dark:text-red-400'
              : 'text-muted-foreground/60 hover:bg-accent hover:text-foreground'"
            :title="session.isPrivate ? t('library.list.action.unmark_private') : t('library.list.action.mark_private')"
            :aria-label="session.isPrivate ? t('library.list.action.unmark_private') : t('library.list.action.mark_private')"
            :aria-pressed="session.isPrivate"
            @click.stop="$emit('toggle-private', session)"
            @keydown.enter.stop.prevent="$emit('toggle-private', session)"
            @keydown.space.stop.prevent="$emit('toggle-private', session)"
          >
            <Lock v-if="session.isPrivate" class="size-3" />
            <LockOpen v-else class="size-3" />
          </span>
          <Badge
            variant="secondary"
            class="h-5 gap-1 bg-muted/70 px-1.5 font-normal text-muted-foreground"
          >
            <MessageCircle class="size-2.5" />
            <span class="tabular-nums">{{ session.messages }}</span>
          </Badge>
          <Badge
            variant="secondary"
            class="h-5 gap-1 bg-muted/70 px-1.5 font-normal text-muted-foreground"
          >
            <Clock class="size-2.5" />
            <span class="tabular-nums">{{ session.durationMin }}m</span>
          </Badge>

          <!--
            摘要状态 badge（可点击）：
            - 已摘要：绿色，hover 加深，点击 → 重新生成
            - 未摘要：琥珀色，hover 加深，点击 → 立即生成
            - summarizing：灰色 + spinner，禁用点击
            外层是 <button>，所以这里用 <span role="button"> + @click.stop，
            和锁按钮同样模式。
          -->
          <span
            v-if="summarizing"
            class="inline-flex h-5 cursor-wait items-center gap-1 rounded-md border border-border/60 bg-muted/50 px-1.5 text-[11px] font-normal text-muted-foreground"
            :title="t('library.list.tooltip.summarizing')"
            :aria-label="t('library.list.tooltip.summarizing')"
          >
            <Loader2 class="size-2.5 animate-spin" />
            {{ t('library.list.badge.summarizing') }}
          </span>
          <span
            v-else-if="session.l2Done"
            role="button"
            tabindex="0"
            class="inline-flex h-5 cursor-pointer items-center gap-1 rounded-md border border-emerald-500/30 bg-emerald-500/5 px-1.5 text-[11px] font-normal text-emerald-700 transition-colors hover:border-emerald-500/60 hover:bg-emerald-500/15 dark:text-emerald-400"
            :title="t('library.list.action.regenerate_summary')"
            :aria-label="t('library.list.action.regenerate_summary')"
            @click.stop="$emit('summarize', session)"
            @keydown.enter.stop.prevent="$emit('summarize', session)"
            @keydown.space.stop.prevent="$emit('summarize', session)"
          >
            <Check class="size-2.5 group-hover/badge:hidden" />
            <RefreshCw class="hidden size-2.5 group-hover/badge:inline" />
            {{ t('library.list.badge.l2_done') }}
          </span>
          <span
            v-else
            role="button"
            tabindex="0"
            class="inline-flex h-5 cursor-pointer items-center gap-1 rounded-md border border-amber-500/40 bg-amber-500/5 px-1.5 text-[11px] font-normal text-amber-700 transition-colors hover:border-amber-500/70 hover:bg-amber-500/20 dark:text-amber-500"
            :title="t('library.list.action.summarize_now')"
            :aria-label="t('library.list.action.summarize_now')"
            @click.stop="$emit('summarize', session)"
            @keydown.enter.stop.prevent="$emit('summarize', session)"
            @keydown.space.stop.prevent="$emit('summarize', session)"
          >
            <Clock class="size-2.5" />
            {{ t('library.list.badge.l2_pending') }}
          </span>

          <template v-if="session.topics.length">
            <span class="mx-0.5 size-1 shrink-0 rounded-full bg-border" />
            <Badge
              v-for="topic in session.topics.slice(0, 3)"
              :key="topic"
              variant="outline"
              class="h-5 px-1.5 font-normal text-muted-foreground"
            >
              {{ topic }}
            </Badge>
            <span
              v-if="session.topics.length > 3"
              class="text-[10px] tabular-nums text-muted-foreground"
            >
              +{{ session.topics.length - 3 }}
            </span>
          </template>
        </div>
        <span class="shrink-0 truncate text-[11px] tabular-nums text-muted-foreground/80">
          {{ session.project }} · {{ groupFmt(session.startedAt, groupKey) }}
        </span>
      </div>
    </div>
    <ChevronRight
      class="mt-1.5 ml-2 size-4 shrink-0 text-muted-foreground/50 transition-all group-hover:translate-x-0.5 group-hover:text-muted-foreground group-data-[active=true]:translate-x-0.5 group-data-[active=true]:text-foreground"
    />
  </button>
</template>
