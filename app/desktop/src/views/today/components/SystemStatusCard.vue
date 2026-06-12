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
import { useI18n } from '@/i18n'

const router = useRouter()
const { t } = useI18n()

const procValue = computed(() => {
  if (!daemon.value) return t('today.sys.proc_querying')
  if (!daemon.value.running) return t('today.sys.proc_off')
  return daemon.value.pid
    ? t('today.sys.proc_running_pid', { pid: daemon.value.pid })
    : t('today.sys.proc_running')
})

const procDot = computed(() => (daemon.value?.running ? 'ok' : 'warn'))

const ftsDot = computed(() => (stats.value?.db_exists ? 'ok' : 'warn'))
const ftsValue = computed(() =>
  stats.value?.db_exists ? t('today.sys.fts_healthy') : t('today.sys.fts_uninitialized'),
)

const sysStats = computed(() => [
  { label: t('today.sys.label_proc'), value: procValue.value, dot: procDot.value },
  {
    label: t('today.sys.label_collector'),
    value: t('today.sys.adapter_active_fmt', {
      active: daemonStatus.adapterActive,
      total: daemonStatus.adapterTotal,
    }),
  },
  {
    label: t('today.sys.label_llm'),
    value: `${daemonStatus.llmProvider} : ${daemonStatus.llmModel}`,
    accent: true,
  },
  { label: t('today.sys.label_fts'), value: ftsValue.value, dot: ftsDot.value },
  { label: t('today.sys.label_storage'), value: '~/.memex/', mono: true },
  { label: t('today.sys.label_sessions'), value: `${stats.value?.sessions ?? 0}` },
])

const headerText = computed(() => {
  const running = daemon.value?.running ? t('today.sys.running') : t('today.sys.not_running')
  const adapterCount = t('today.sys.adapter_count_fmt', {
    active: daemonStatus.adapterActive,
    total: daemonStatus.adapterTotal,
  })
  return t('today.sys.header_fmt', {
    running,
    adapter_count: adapterCount,
    provider: daemonStatus.llmProvider,
    model: daemonStatus.llmModel,
  })
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
          <span class="text-[14px] font-semibold">{{ t('today.sys.title') }}</span>
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
              {{ t('today.sys.open_connect') }}
              <ArrowRight class="size-3" />
            </Button>
          </div>
        </div>
      </CollapsibleContent>
    </Collapsible>
  </Card>
</template>
