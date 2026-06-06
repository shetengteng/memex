<script setup lang="ts">
import { computed } from 'vue'
import { Button } from '@/components/ui/button'
import { RefreshCw } from '@lucide/vue'
import { daemonStatus } from '@/mock/data'
import ActivityCard from './components/ActivityCard.vue'
import WeeklySummaryCard from './components/WeeklySummaryCard.vue'
import ReflectionCard from './components/ReflectionCard.vue'
import SmartResumeCard from './components/SmartResumeCard.vue'
import SystemStatusCard from './components/SystemStatusCard.vue'

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
</script>

<template>
  <div class="@container/main flex flex-1 flex-col min-h-0 overflow-y-auto">
    <div class="mx-auto w-full max-w-6xl space-y-6 px-6 py-6">
      <section class="flex items-end justify-between">
        <div>
          <h1 class="text-2xl font-bold tracking-tight">{{ greeting }}，Terrell</h1>
          <p class="mt-1 text-[13px] text-muted-foreground">
            {{ todayStr }} · 上次采集
            <span class="font-medium text-foreground">{{ daemonStatus.lastIngest }}</span>
          </p>
        </div>
        <Button variant="ghost" size="sm" class="gap-1.5">
          <RefreshCw class="size-3.5" />
          刷新
        </Button>
      </section>

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
