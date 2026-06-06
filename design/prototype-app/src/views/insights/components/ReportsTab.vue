<script setup lang="ts">
import { computed, ref } from 'vue'
import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  Calendar,
  CalendarDays,
  CalendarRange,
  Download,
  Sparkles,
} from '@lucide/vue'
import { reports, type ReportScope } from '@/mock/data'

const scope = ref<ReportScope>('daily')
const filteredReports = computed(() => reports.filter((r) => r.scope === scope.value))
const selectedReportId = ref(filteredReports.value[0]?.id ?? '')
const selectedReport = computed(
  () => reports.find((r) => r.id === selectedReportId.value) ?? filteredReports.value[0],
)
const onScopeChange = (s: string | number) => {
  scope.value = String(s) as ReportScope
  selectedReportId.value = filteredReports.value[0]?.id ?? ''
}
</script>

<template>
  <div class="mx-auto w-full max-w-6xl px-4 py-4 lg:px-6 lg:py-6">
    <div class="mb-4 flex items-center justify-between">
      <Tabs :model-value="scope" @update:model-value="onScopeChange">
        <TabsList class="h-9">
          <TabsTrigger value="daily" class="gap-1.5">
            <Calendar class="size-3.5" />
            日报
          </TabsTrigger>
          <TabsTrigger value="weekly" class="gap-1.5">
            <CalendarDays class="size-3.5" />
            周报
          </TabsTrigger>
          <TabsTrigger value="monthly" class="gap-1.5">
            <CalendarRange class="size-3.5" />
            月报
          </TabsTrigger>
        </TabsList>
      </Tabs>
      <div class="flex items-center gap-2">
        <Button variant="ghost" size="sm" class="h-8 gap-1.5">
          <Sparkles class="size-3.5" />
          重新生成
        </Button>
        <Button variant="outline" size="sm" class="h-8 gap-1.5">
          <Download class="size-3.5" />
          导出
        </Button>
      </div>
    </div>

    <div class="grid grid-cols-1 gap-6 lg:grid-cols-[240px_1fr]">
      <Card class="overflow-hidden">
        <div
          class="border-b px-3 py-2.5 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground"
        >
          最近 {{ filteredReports.length }} 份
        </div>
        <ScrollArea class="max-h-[480px]">
          <ul>
            <li v-for="r in filteredReports" :key="r.id">
              <button
                :class="[
                  'flex w-full items-baseline justify-between px-3 py-2 text-left transition-colors hover:bg-accent',
                  selectedReportId === r.id ? 'bg-accent font-semibold' : '',
                ]"
                @click="selectedReportId = r.id"
              >
                <span class="text-[13px] tabular-nums">{{ r.date.slice(5) }}</span>
                <span class="text-[11px] text-muted-foreground tabular-nums">
                  {{ r.sessions }} 个
                </span>
              </button>
            </li>
          </ul>
        </ScrollArea>
      </Card>

      <Card class="p-6">
        <header class="mb-4">
          <div class="mb-1 flex items-baseline gap-3">
            <h2 class="text-lg font-semibold">{{ selectedReport?.period }}</h2>
            <Badge variant="secondary">{{ selectedReport?.sessions }} 个会话</Badge>
            <Badge variant="outline">qwen2.5</Badge>
          </div>
          <p class="text-[12px] text-muted-foreground">
            生成于 09:23 · 348 条消息 · 跨 4 个项目 / 3 个工具
          </p>
        </header>

        <pre
          class="whitespace-pre-wrap font-sans text-[14px] leading-relaxed"
        >{{ selectedReport?.body }}</pre>

        <Separator class="my-5" />

        <div class="mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
          主题
        </div>
        <div class="flex flex-wrap gap-1.5">
          <Badge v-for="t in selectedReport?.topics" :key="t" variant="secondary">{{ t }}</Badge>
        </div>
      </Card>
    </div>
  </div>
</template>
