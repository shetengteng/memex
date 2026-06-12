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

const refreshing = ref(false)

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
  const loadingId = toast.loading('刷新中…')
  try {
    await Promise.all([refreshSessions(), refreshProjects(), refreshBreakdown()])
    window.dispatchEvent(new CustomEvent('today-refresh'))
    toast.dismiss(loadingId)
    toast.success('已刷新')
  } catch (e) {
    toast.dismiss(loadingId)
    toastBackendError('刷新失败', e)
  } finally {
    refreshing.value = false
  }
}

const greeting = computed(() => {
  const h = new Date().getHours()
  if (h < 6) return '深夜好'
  if (h < 12) return '早上好'
  if (h < 14) return '中午好'
  if (h < 18) return '下午好'
  return '晚上好'
})

const todayStr = computed(() => {
  const d = new Date()
  const w = ['日', '一', '二', '三', '四', '五', '六'][d.getDay()]
  return `今天是 ${d.getFullYear()} 年 ${d.getMonth() + 1} 月 ${d.getDate()} 日 周${w}`
})

// 后端 DaemonStatus 暂未暴露 last_ingest_at —— 用 store 里最新一条 session 的 startedAt 兜底，
// 因为采集就是把会话写入 DB，sessions[0] 即"最近一次成功采集"的时间戳。
// 全空时根据 daemon 状态给出友好兜底，避免界面上孤零零的空白。
const lastIngestText = computed(() => {
  const raw = sessions[0]?.startedAt?.trim() || ''
  if (raw) return formatRelative(raw)
  if (daemon.value && daemon.value.running === false) return '后台未运行'
  if (daemon.value) return '尚未采集'
  return '等待后台连接…'
})

function formatRelative(iso: string): string {
  const t = new Date(iso).getTime()
  if (!Number.isFinite(t)) return iso
  const diff = (Date.now() - t) / 1000
  if (diff < 0) return '刚刚'
  if (diff < 60) return `${Math.floor(diff)} 秒前`
  if (diff < 3600) return `${Math.floor(diff / 60)} 分钟前`
  if (diff < 86400) return `${Math.floor(diff / 3600)} 小时前`
  return `${Math.floor(diff / 86400)} 天前`
}
</script>

<template>
  <div class="@container/main flex flex-1 flex-col min-h-0 overflow-y-auto">
    <div class="mx-auto w-full max-w-6xl space-y-6 px-6 py-6">
      <section class="flex items-end justify-between">
        <div>
          <h1 class="text-2xl font-bold tracking-tight">{{ greeting }}，{{ userName }}</h1>
          <p class="mt-1 text-[13px] text-muted-foreground">
            {{ todayStr }} · 上次采集
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
          {{ refreshing ? '刷新中…' : '刷新' }}
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
