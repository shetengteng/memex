<script setup lang="ts">
import { computed, ref } from 'vue'
import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { Loader2, Radio, RefreshCw } from '@lucide/vue'
import IdeDot from '@/components/shell/IdeDot.vue'
import { adapters } from '@/mock/data'

const activeCount = computed(() => adapters.filter((a) => a.status === 'active').length)
const totalAdapters = adapters.length

const rescanning = ref<Record<string, boolean>>({})
function rescanAdapter(id: string) {
  rescanning.value[id] = true
  setTimeout(() => {
    rescanning.value[id] = false
  }, 1200)
}
</script>

<template>
  <section>
    <div class="mb-3 flex items-end justify-between">
      <div>
        <div class="flex items-center gap-2">
          <Radio class="size-3.5" :style="{ color: 'var(--success)' }" />
          <h2 class="text-[15px] font-semibold">采集源</h2>
          <Badge class="border-emerald-500/30 bg-emerald-500/10 text-emerald-700">
            {{ activeCount }} / {{ totalAdapters }} 个启用
          </Badge>
        </div>
        <p class="mt-0.5 text-[11px] text-muted-foreground">
          监听 IDE 会话目录，2 秒内自动入库
        </p>
      </div>
      <Button variant="outline" size="sm" class="h-8 gap-1.5">
        <RefreshCw class="size-3.5" />
        立即扫描
      </Button>
    </div>

    <Card class="overflow-hidden p-0">
      <div
        v-for="(a, i) in adapters"
        :key="a.id"
        class="flex items-center gap-3 px-4 py-3"
        :class="i < adapters.length - 1 && 'border-b'"
      >
        <IdeDot :adapter="a.id" size="lg" />
        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-2 text-[13px] font-semibold">
            {{ a.label }}
            <Badge
              v-if="a.status === 'active'"
              class="border-emerald-500/30 bg-emerald-500/10 text-emerald-700"
            >
              ● 已启用
            </Badge>
            <Badge v-else variant="outline" class="text-muted-foreground">○ 未启用</Badge>
          </div>
          <div class="truncate font-mono text-[11px] text-muted-foreground">{{ a.path }}</div>
        </div>
        <div
          class="text-right"
          :class="a.status === 'disabled' && a.sessions === 0 && 'opacity-50'"
        >
          <div class="text-[13px] font-semibold tabular-nums">
            {{ a.sessions === 0 ? '—' : a.sessions.toLocaleString() }}
          </div>
          <div class="text-[10px] text-muted-foreground">个会话</div>
        </div>
        <Switch :model-value="a.status === 'active'" />
        <Tooltip>
          <TooltipTrigger as-child>
            <Button
              variant="ghost"
              size="icon"
              class="size-7"
              :disabled="a.status !== 'active' || rescanning[a.id]"
              @click="rescanAdapter(a.id)"
            >
              <Loader2 v-if="rescanning[a.id]" class="size-3.5 animate-spin" />
              <RefreshCw v-else class="size-3.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="top" class="text-[11px]">重扫该采集源</TooltipContent>
        </Tooltip>
      </div>
    </Card>
  </section>
</template>
