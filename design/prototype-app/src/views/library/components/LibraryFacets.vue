<script setup lang="ts">
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'
import { Label } from '@/components/ui/label'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import IdeDot from '@/components/shell/IdeDot.vue'
import { X } from '@lucide/vue'
import { adapters, projects, type Adapter } from '@/mock/data'

const adapterCounts: Record<Adapter, number> = {
  claude_code: 3210,
  cursor: 2148,
  codex: 321,
  opencode: 842,
  aider: 0,
  continue: 0,
  cline: 0,
}

defineProps<{
  fAdapters: Adapter[]
  fProjects: string[]
  fTime: string
  fSummary: string
  activeFilterCount: number
  hasActiveFilters: boolean
}>()

const emit = defineEmits<{
  toggleAdapter: [Adapter]
  toggleProject: [string]
  'update:fTime': [string]
  'update:fSummary': [string]
  clear: []
}>()

const formatCount = (n: number) => {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1).replace(/\.0$/, '')}M`
  if (n >= 1000) return `${(n / 1000).toFixed(1).replace(/\.0$/, '')}K`
  return String(n)
}
</script>

<template>
  <div class="flex h-full flex-col">
    <div class="flex-1 overflow-y-auto p-4">
      <div class="space-y-7">
        <!-- adapter -->
        <div>
          <div class="mb-2 flex items-center justify-between">
            <span class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
              工具
            </span>
            <button class="text-[11px] text-muted-foreground hover:text-foreground">全选</button>
          </div>
          <div class="space-y-1.5">
            <Label
              v-for="a in adapters.slice(0, 4)"
              :key="a.id"
              class="cursor-pointer text-[13px] font-normal"
            >
              <Checkbox
                :model-value="fAdapters.includes(a.id)"
                @update:model-value="emit('toggleAdapter', a.id)"
              />
              <IdeDot :adapter="a.id" />
              <span class="flex-1">{{ a.label }}</span>
              <Tooltip :delay-duration="120">
                <TooltipTrigger as-child>
                  <span class="cursor-default text-[11px] tabular-nums text-muted-foreground">
                    {{ formatCount(adapterCounts[a.id]) }}
                  </span>
                </TooltipTrigger>
                <TooltipContent side="right" :side-offset="6" class="px-2 py-1 text-[11px]">
                  <span class="tabular-nums">{{ adapterCounts[a.id].toLocaleString() }}</span>
                  <span class="ml-1 text-muted-foreground">个会话</span>
                </TooltipContent>
              </Tooltip>
            </Label>
          </div>
        </div>

        <!-- project -->
        <div>
          <div class="mb-2 flex items-center justify-between">
            <span class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
              项目
            </span>
            <button class="text-[11px] text-muted-foreground hover:text-foreground">全选</button>
          </div>
          <div class="space-y-1.5">
            <Label
              v-for="p in projects"
              :key="p.id"
              class="cursor-pointer text-[13px] font-normal"
            >
              <Checkbox
                :model-value="fProjects.includes(p.name)"
                @update:model-value="emit('toggleProject', p.name)"
              />
              <span class="flex-1 truncate">{{ p.name }}</span>
              <Tooltip :delay-duration="120">
                <TooltipTrigger as-child>
                  <span class="cursor-default text-[11px] tabular-nums text-muted-foreground">
                    {{ formatCount(p.sessions) }}
                  </span>
                </TooltipTrigger>
                <TooltipContent side="right" :side-offset="6" class="px-2 py-1 text-[11px]">
                  <span class="tabular-nums">{{ p.sessions.toLocaleString() }}</span>
                  <span class="ml-1 text-muted-foreground">个会话</span>
                </TooltipContent>
              </Tooltip>
            </Label>
            <button class="text-[11px] text-muted-foreground hover:text-foreground">+ 12 更多…</button>
          </div>
        </div>

        <!-- time -->
        <div>
          <div class="mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
            时间
          </div>
          <RadioGroup :model-value="fTime" @update:model-value="(v) => emit('update:fTime', v)" class="gap-1.5">
            <Label class="cursor-pointer text-[13px] font-normal">
              <RadioGroupItem value="today" />今天
            </Label>
            <Label class="cursor-pointer text-[13px] font-normal">
              <RadioGroupItem value="7d" />近 7 天
            </Label>
            <Label class="cursor-pointer text-[13px] font-normal">
              <RadioGroupItem value="30d" />近 30 天
            </Label>
            <Label class="cursor-pointer text-[13px] font-normal">
              <RadioGroupItem value="90d" />近 90 天
            </Label>
            <Label class="cursor-pointer text-[13px] font-normal">
              <RadioGroupItem value="all" />全部
            </Label>
          </RadioGroup>
        </div>

        <!-- summary state -->
        <div>
          <div class="mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
            摘要状态
          </div>
          <RadioGroup
            :model-value="fSummary"
            @update:model-value="(v) => emit('update:fSummary', v)"
            class="gap-1.5"
          >
            <Label class="cursor-pointer text-[13px] font-normal">
              <RadioGroupItem value="all" />全部
            </Label>
            <Label class="cursor-pointer text-[13px] font-normal">
              <RadioGroupItem value="done" />仅已摘要
            </Label>
            <Label class="cursor-pointer text-[13px] font-normal">
              <RadioGroupItem value="pending" />仅待摘要
            </Label>
            <Label class="cursor-pointer text-[13px] font-normal">
              <RadioGroupItem value="invalid" />无效会话
            </Label>
          </RadioGroup>
        </div>
      </div>
    </div>

    <div class="shrink-0 border-t bg-card p-3">
      <Button
        variant="outline"
        size="sm"
        class="w-full gap-1.5 border-destructive/30 text-[12px] text-destructive hover:bg-destructive/10 hover:text-destructive"
        :disabled="!hasActiveFilters"
        @click="emit('clear')"
      >
        <X class="size-3.5" />
        清除全部筛选
        <Badge v-if="activeFilterCount" variant="secondary" class="ml-auto h-4 px-1.5 text-[10px]">
          {{ activeFilterCount }}
        </Badge>
      </Button>
    </div>
  </div>
</template>
