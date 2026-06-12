<script setup lang="ts">
import { computed, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { RefreshCw } from 'lucide-vue-next'
import { toast } from 'vue-sonner'
import {
  daemon,
  refreshBreakdown,
  refreshProjects,
  refreshSessions,
  sessions,
  userName,
} from '@/stores/memex'
import ActivityCard from './components/ActivityCard.vue'
import KpiRowCard from './components/KpiRowCard.vue'
import WeeklySummaryCard from './components/WeeklySummaryCard.vue'
import ReflectionCard from './components/ReflectionCard.vue'
import SmartResumeCard from './components/SmartResumeCard.vue'
import SystemStatusCard from './components/SystemStatusCard.vue'
import { toastBackendError } from '@/lib/toast-error'
import { useI18n } from '@/i18n'

const refreshing = ref(false)
const { t } = useI18n()

/**
 * Today 页右上「刷新」按钮：用户主动触发数据重拉。
 * 拉的是 store 维度（sessions / projects / breakdown），让 ActivityCard /
 * WeeklySummaryCard 等子组件的派生 computed 立刻刷新。各子组件自己 mount 时拉的
 * 私有数据（如 ActivityCard.workload）由 'today-refresh' 事件触发它们重拉。
 */
async function onRefresh() {
  if (refreshing.value) return
  refreshing.value = true
  // 立即弹一个 loading toast 让用户知道点击有反应；完成后 dismiss + 改成 success/error。
  // 之前只有 toast.success（end），如果数据刷新很快用户感觉不到任何反馈。
  const loadingId = toast.loading(t('today.toast.refreshing'))
  try {
    await Promise.all([refreshSessions(), refreshProjects(), refreshBreakdown()])
    window.dispatchEvent(new CustomEvent('today-refresh'))
    toast.dismiss(loadingId)
    toast.success(t('today.toast.refreshed'))
  } catch (e) {
    toast.dismiss(loadingId)
    toastBackendError(t('today.toast.refresh_failed'), e)
  } finally {
    refreshing.value = false
  }
}

const greeting = computed(() => {
  const h = new Date().getHours()
  if (h < 6) return t('today.greet.midnight')
  if (h < 12) return t('today.greet.morning')
  if (h < 14) return t('today.greet.noon')
  if (h < 18) return t('today.greet.afternoon')
  return t('today.greet.evening')
})

const todayStr = computed(() => {
  const d = new Date()
  const w = t(`today.weekday.${d.getDay()}`)
  return t('today.date_fmt', {
    year: d.getFullYear(),
    month: d.getMonth() + 1,
    day: d.getDate(),
    weekday: w,
  })
})

// 后端 DaemonStatus 暂未暴露 last_ingest_at —— 用 store 里最新一条 session 的 startedAt 兜底，
// 因为采集就是把会话写入 DB，sessions[0] 即"最近一次成功采集"的时间戳。
// 全空时根据 daemon 状态给出友好兜底，避免界面上孤零零的空白。
const lastIngestText = computed(() => {
  const raw = sessions[0]?.startedAt?.trim() || ''
  if (raw) return formatRelative(raw)
  if (daemon.value && daemon.value.running === false) return t('today.last_ingest.daemon_off')
  if (daemon.value) return t('today.last_ingest.none')
  return t('today.last_ingest.waiting')
})

function formatRelative(iso: string): string {
  const ts = new Date(iso).getTime()
  if (!Number.isFinite(ts)) return iso
  const diff = (Date.now() - ts) / 1000
  if (diff < 0) return t('today.rel.just_now')
  if (diff < 60) return t('today.rel.seconds', { n: Math.floor(diff) })
  if (diff < 3600) return t('today.rel.minutes', { n: Math.floor(diff / 60) })
  if (diff < 86400) return t('today.rel.hours', { n: Math.floor(diff / 3600) })
  return t('today.rel.days', { n: Math.floor(diff / 86400) })
}
</script>

<template>
  <div class="@container/main flex flex-1 flex-col min-h-0 overflow-y-auto">
    <div class="mx-auto w-full max-w-6xl space-y-6 px-6 py-6">
      <section class="flex items-end justify-between">
        <div>
          <h1 class="text-2xl font-bold tracking-tight">{{ greeting }}, {{ userName }}</h1>
          <p class="mt-1 text-[13px] text-muted-foreground">
            {{ todayStr }} · {{ t('today.last_ingest.label') }}
            <span class="font-medium text-foreground">{{ lastIngestText }}</span>
          </p>
        </div>
        <Button
          variant="ghost"
          size="sm"
          class="gap-1.5"
          :disabled="refreshing"
          @click="onRefresh"
        >
          <RefreshCw :class="['size-3.5', refreshing && 'animate-spin']" />
          {{ refreshing ? t('today.refresh.refreshing') : t('today.refresh.label') }}
        </Button>
      </section>

      <KpiRowCard />

      <ActivityCard />

      <section class="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <WeeklySummaryCard />
        <ReflectionCard />
      </section>

      <SmartResumeCard />

      <SystemStatusCard />
    </div>
  </div>
</template>
