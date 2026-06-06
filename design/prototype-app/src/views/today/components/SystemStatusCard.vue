<script setup lang="ts">
import { useRouter } from 'vue-router'
import { Card } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import { ArrowRight, ChevronDown, Settings2 } from '@lucide/vue'
import { daemonStatus } from '@/mock/data'

const router = useRouter()

const sysStats = [
  { label: '后台进程', value: '运行中 (pid 4823)', dot: 'ok' },
  { label: '采集器', value: `${daemonStatus.adapterActive} / ${daemonStatus.adapterTotal} 个活跃` },
  {
    label: 'LLM 服务',
    value: `${daemonStatus.llmProvider} : ${daemonStatus.llmModel}`,
    accent: true,
  },
  { label: 'FTS5 索引', value: '健康', dot: 'ok' },
  { label: '存储路径', value: '~/.memex/', mono: true },
  { label: '已用空间', value: '824 MB' },
]
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
          <span class="status-dot status-dot-ok" />
          <span class="text-[12px] text-muted-foreground">
            后台运行中 · {{ daemonStatus.adapterActive }}/{{ daemonStatus.adapterTotal }} 个适配器 · LLM
            {{ daemonStatus.llmProvider }}:{{ daemonStatus.llmModel }}
          </span>
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
