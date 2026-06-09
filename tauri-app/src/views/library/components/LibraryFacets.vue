<script setup lang="ts">
import { computed, ref } from 'vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'
import { Label } from '@/components/ui/label'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import IdeDot from '@/components/shell/IdeDot.vue'
import { Search, X } from 'lucide-vue-next'
import { adapters, breakdownByAdapter, projects } from '@/stores/memex'
import { formatNumber } from '@/lib/utils'
import type { SummaryFilter, TimeFilter } from '../composables/sessionFilters'

const adapterCount = (id: string): number => breakdownByAdapter[id] ?? 0

const TIME_FILTERS = ['today', '7d', '30d', '90d', 'all'] as const
const SUMMARY_FILTERS = ['all', 'done', 'pending'] as const

const props = defineProps<{
  fAdapters: string[]
  fProjects: string[]
  fTime: TimeFilter
  fSummary: SummaryFilter
  activeFilterCount: number
  hasActiveFilters: boolean
}>()

const emit = defineEmits<{
  toggleAdapter: [string]
  toggleProject: [string]
  'update:fAdapters': [string[]]
  'update:fProjects': [string[]]
  'update:fTime': [TimeFilter]
  'update:fSummary': [SummaryFilter]
  clear: []
}>()

function emitTimeFilter(v: unknown): void {
  const s = String(v ?? '')
  if ((TIME_FILTERS as readonly string[]).includes(s)) {
    emit('update:fTime', s as TimeFilter)
  }
}

function emitSummaryFilter(v: unknown): void {
  const s = String(v ?? '')
  if ((SUMMARY_FILTERS as readonly string[]).includes(s)) {
    emit('update:fSummary', s as SummaryFilter)
  }
}

const PROJECT_DEFAULT_LIMIT = 8
const PROJECT_PAGE_STEP = 10
const projectQuery = ref('')
const projectsLimit = ref(PROJECT_DEFAULT_LIMIT)

const sortedProjects = computed(() =>
  [...projects].sort((a, b) => b.sessions - a.sessions),
)
const filteredProjects = computed(() => {
  const q = projectQuery.value.trim().toLowerCase()
  if (!q) return sortedProjects.value
  return sortedProjects.value.filter((p) => p.name.toLowerCase().includes(q))
})
// 搜索时 → 全部命中；未搜索 → 当前 limit
const visibleProjects = computed(() => {
  if (projectQuery.value.trim()) return filteredProjects.value
  return filteredProjects.value.slice(0, projectsLimit.value)
})
const hiddenProjectCount = computed(() => {
  if (projectQuery.value.trim()) return 0
  return Math.max(0, filteredProjects.value.length - projectsLimit.value)
})
const nextProjectStep = computed(() =>
  Math.min(PROJECT_PAGE_STEP, hiddenProjectCount.value),
)
function expandProjects() {
  projectsLimit.value += PROJECT_PAGE_STEP
}
function collapseProjects() {
  projectsLimit.value = PROJECT_DEFAULT_LIMIT
}

// 全选/全清逻辑：均针对当前可见集合
const allAdaptersSelected = computed(() =>
  adapters.length > 0 && adapters.every((a) => props.fAdapters.includes(a.id)),
)
function toggleSelectAllAdapters() {
  if (allAdaptersSelected.value) {
    emit('update:fAdapters', [])
  } else {
    emit(
      'update:fAdapters',
      adapters.map((a) => a.id),
    )
  }
}

// 项目"全选"指：全选当前 visible 集合（搜索时仅筛选结果，未搜索时仅默认 N 条 + 已展开后全部）
const allVisibleProjectsSelected = computed(() => {
  if (visibleProjects.value.length === 0) return false
  return visibleProjects.value.every((p) => props.fProjects.includes(p.name))
})
function toggleSelectAllProjects() {
  if (allVisibleProjectsSelected.value) {
    // 取消可见集合中的所有选中
    const visibleNames = new Set(visibleProjects.value.map((p) => p.name))
    emit(
      'update:fProjects',
      props.fProjects.filter((n) => !visibleNames.has(n)),
    )
  } else {
    // 把可见集合的项目全部加入 selected（保留原有的不在 visible 中的选中）
    const merged = new Set(props.fProjects)
    for (const p of visibleProjects.value) merged.add(p.name)
    emit('update:fProjects', [...merged])
  }
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
            <button
              class="text-[11px] text-muted-foreground hover:text-foreground"
              @click="toggleSelectAllAdapters"
            >
              {{ allAdaptersSelected ? '全清' : '全选' }}
            </button>
          </div>
          <div class="space-y-1.5">
            <Label
              v-for="a in adapters"
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
                    {{ formatNumber(adapterCount(a.id)) }}
                  </span>
                </TooltipTrigger>
                <TooltipContent side="right" :side-offset="6" class="px-2 py-1 text-[11px]">
                  <span class="tabular-nums">{{ adapterCount(a.id).toLocaleString() }}</span>
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
              <span v-if="projects.length > 0" class="ml-1 normal-case tracking-normal text-muted-foreground/60">
                ({{ projects.length }})
              </span>
            </span>
            <button
              class="text-[11px] text-muted-foreground hover:text-foreground"
              :disabled="visibleProjects.length === 0"
              @click="toggleSelectAllProjects"
            >
              {{ allVisibleProjectsSelected ? '全清' : '全选' }}
            </button>
          </div>

          <div class="relative mb-2">
            <Search class="pointer-events-none absolute left-2 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground/60" />
            <Input
              v-model="projectQuery"
              type="search"
              placeholder="搜索项目名…"
              class="h-7 pl-7 pr-7 text-[12px]"
            />
            <button
              v-if="projectQuery"
              class="absolute right-1.5 top-1/2 -translate-y-1/2 rounded p-0.5 text-muted-foreground hover:bg-muted hover:text-foreground"
              @click="projectQuery = ''"
            >
              <X class="size-3" />
            </button>
          </div>

          <div class="space-y-1.5">
            <Label
              v-for="p in visibleProjects"
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
                    {{ formatNumber(p.sessions) }}
                  </span>
                </TooltipTrigger>
                <TooltipContent side="right" :side-offset="6" class="px-2 py-1 text-[11px]">
                  <span class="tabular-nums">{{ p.sessions.toLocaleString() }}</span>
                  <span class="ml-1 text-muted-foreground">个会话</span>
                </TooltipContent>
              </Tooltip>
            </Label>

            <p
              v-if="filteredProjects.length === 0"
              class="py-2 text-center text-[11px] text-muted-foreground"
            >
              没有匹配的项目
            </p>

            <div class="flex items-center gap-3 text-[11px]">
              <button
                v-if="hiddenProjectCount > 0"
                class="text-muted-foreground hover:text-foreground"
                @click="expandProjects"
              >
                + 展开 {{ nextProjectStep }}（剩 {{ hiddenProjectCount }}）
              </button>
              <button
                v-if="!projectQuery && projectsLimit > PROJECT_DEFAULT_LIMIT"
                class="text-muted-foreground hover:text-foreground"
                @click="collapseProjects"
              >
                收起
              </button>
            </div>
          </div>
        </div>

        <!-- time -->
        <div>
          <div class="mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
            时间
          </div>
          <RadioGroup :model-value="fTime" @update:model-value="emitTimeFilter" class="gap-1.5">
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
            @update:model-value="emitSummaryFilter"
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
