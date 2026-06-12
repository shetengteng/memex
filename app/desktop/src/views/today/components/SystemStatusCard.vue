<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { Card } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import { ArrowRight, ChevronDown, Settings2 } from 'lucide-vue-next'
import { daemon, daemonStatus, stats } from '@/stores/memex'

const router = useRouter()

const procValue = computed(() => {
  if (!daemon.value) return '查询中…'
  if (!daemon.value.running) return '未运行'
  return daemon.value.pid ? `运行中 (pid ${daemon.value.pid})` : '运行中'
})

const procDot = computed(() => (daemon.value?.running ? 'ok' : 'warn'))

const ftsDot = computed(() => (stats.value?.db_exists ? 'ok' : 'warn'))
const ftsValue = computed(() => (stats.value?.db_exists ? '健康' : '未初始化'))

const sysStats = computed(() => [
  { label: '后台进程', value: procValue.value, dot: procDot.value },
  { label: '采集器', value: `${daemonStatus.adapterActive} / ${daemonStatus.adapterTotal} 个活跃` },
  {
    label: 'LLM 服务',
    value: `${daemonStatus.llmProvider} : ${daemonStatus.llmModel}`,
    accent: true,
  },
  { label: 'FTS5 索引', value: ftsValue.value, dot: ftsDot.value },
  { label: '存储路径', value: '~/.memex/', mono: true },
  { label: '会话数', value: `${stats.value?.sessions ?? 0}` },
])

const headerText = computed(() => {
  const running = daemon.value?.running ? '后台运行中' : '后台未运行'
  return `${running} · ${daemonStatus.adapterActive}/${daemonStatus.adapterTotal} 个适配器 · LLM ${daemonStatus.llmProvider}:${daemonStatus.llmModel}`
})
</script>

<template>
  <Card class="overflow-hidden">
    <Collapsible>
      <CollapsibleTrigger
        class="group flex w-full items-center justify-between px-5 py-3 text-left hover:bg-accent/40"
      >
        <div class="flex items-center gap-2">
          <Settings2 class="size-3.5 text-muted-foreground" />
          <span class="text-[14px] font-semibold">系统状态</span>
          <span :class="['status-dot', daemon?.running ? 'status-dot-ok' : 'status-dot-warn']" />
          <span class="text-[12px] text-muted-foreground">{{ headerText }}</span>
        </div>
        <ChevronDown
          class="size-3.5 text-muted-foreground transition-transform group-data-[state=open]:rotate-180"
        />
      </CollapsibleTrigger>
      <CollapsibleContent>
        <div class="space-y-3 px-5 pb-4">
          <Separator />
          <div class="grid grid-cols-2 gap-x-6 gap-y-1.5 pt-1 text-[12px]">
            <div v-for="r in sysStats" :key="r.label" class="flex justify-between">
              <span class="text-muted-foreground">{{ r.label }}</span>
              <span
                :class="[
                  'flex items-center gap-1',
                  r.mono && 'font-mono text-[11px]',
                  r.accent && 'text-[var(--adapter-claude)]',
                ]"
              >
                <span v-if="r.dot === 'ok'" class="status-dot status-dot-ok" />
                {{ r.value }}
              </span>
            </div>
          </div>
          <div class="flex justify-end pt-1">
            <Button
              variant="outline"
              size="sm"
              class="h-7 gap-1 text-xs"
              @click="router.push('/connect')"
            >
              打开"连接"页
              <ArrowRight class="size-3" />
            </Button>
          </div>
        </div>
      </CollapsibleContent>
    </Collapsible>
  </Card>
</template>
