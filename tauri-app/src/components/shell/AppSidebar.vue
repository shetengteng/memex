<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuBadge,
} from '@/components/ui/sidebar'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { Button } from '@/components/ui/button'
import { formatNumber, humanizeBackendError } from '@/lib/utils'
import {
  ChevronUp,
  Library,
  Plug,
  Settings as SettingsIcon,
  Sparkles,
  Sunrise,
  Wand2,
} from 'lucide-vue-next'
import { toast } from 'vue-sonner'
import { totals, daemonStatus, adapters, stats } from '@/stores/memex'
import { useMemex } from '@/composables/useMemex'
import packageJson from '../../../package.json'

interface NavItem {
  title: string
  to: string
  icon: any
  badge?: { display: string; tooltip: string }
  trailing?: 'status-ok'
}

const route = useRoute()
const router = useRouter()
const memex = useMemex()
const batchSummarizing = ref(false)

const librarySessionsBadge = computed(() => ({
  display: formatNumber(totals.sessions),
  tooltip: `${totals.sessions.toLocaleString()} 个会话`,
}))

const navMain = computed<NavItem[]>(() => [
  { title: '今天', to: '/today', icon: Sunrise },
  { title: '资料库', to: '/library', icon: Library, badge: librarySessionsBadge.value },
  { title: '洞察', to: '/insights', icon: Sparkles },
])

const navWorkspace: NavItem[] = [
  { title: '连接', to: '/connect', icon: Plug, trailing: 'status-ok' },
  { title: '设置', to: '/settings', icon: SettingsIcon },
]

const isActive = (to: string) => route.path === to || route.path.startsWith(to + '/')

const activeAdapters = computed(() => adapters.filter((a) => a.status === 'active').length)

// 摘要进度：summaries / sessions_eligible_for_summary
const summaryPct = computed(() => {
  const eligible = stats.value?.sessions_eligible_for_summary ?? 0
  if (!eligible) return 100
  const done = stats.value?.summaries ?? 0
  return Math.round((done / eligible) * 100)
})

const sessionsPending = computed(() => {
  const eligible = stats.value?.sessions_eligible_for_summary ?? 0
  const done = stats.value?.summaries ?? 0
  return Math.max(0, eligible - done)
})

async function runBatchSummarize() {
  if (batchSummarizing.value) return
  batchSummarizing.value = true
  try {
    const n = await memex.batchSummarize()
    if (n > 0) toast.success(`已触发 ${n} 个会话的摘要任务`)
    else toast.info('暂无待处理的会话')
  } catch (e) {
    const fe = humanizeBackendError(e)
    toast.error('生成摘要失败', {
      description: fe.friendly,
      action: fe.action
        ? { label: fe.action.label, onClick: () => router.push(fe.action!.route) }
        : undefined,
      duration: 8000,
    })
  } finally {
    batchSummarizing.value = false
  }
}

const appVersion = `v${packageJson.version}`
</script>

<template>
  <Sidebar variant="inset" collapsible="icon">
    <SidebarHeader>
      <SidebarMenu>
        <SidebarMenuItem>
          <SidebarMenuButton size="lg" as-child class="data-[slot=sidebar-menu-button]:p-1.5">
            <RouterLink to="/today" class="flex items-center gap-2.5">
              <div
                class="flex aspect-square size-8 items-center justify-center rounded-lg text-sm font-bold text-white"
                style="background: linear-gradient(135deg, #18181b, #4f46e5);"
              >
                M
              </div>
              <div class="grid flex-1 text-left leading-tight">
                <span class="truncate text-[15px] font-bold tracking-tight">Memex</span>
                <span class="truncate text-[11px] text-muted-foreground">AI 记忆中枢</span>
              </div>
            </RouterLink>
          </SidebarMenuButton>
        </SidebarMenuItem>
      </SidebarMenu>
    </SidebarHeader>

    <SidebarContent>
      <SidebarGroup>
        <SidebarGroupContent>
          <SidebarMenu>
            <SidebarMenuItem v-for="item in navMain" :key="item.to">
              <SidebarMenuButton as-child :is-active="isActive(item.to)" :tooltip="item.title">
                <RouterLink :to="item.to">
                  <component :is="item.icon" />
                  <span>{{ item.title }}</span>
                </RouterLink>
              </SidebarMenuButton>
              <Tooltip v-if="item.badge !== undefined" :delay-duration="80">
                <TooltipTrigger as-child>
                  <SidebarMenuBadge class="cursor-default tabular-nums">
                    {{ item.badge.display }}
                  </SidebarMenuBadge>
                </TooltipTrigger>
                <TooltipContent side="right" :side-offset="6">
                  {{ item.badge.tooltip }}
                </TooltipContent>
              </Tooltip>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarGroupContent>
      </SidebarGroup>

      <SidebarGroup>
        <SidebarGroupLabel>工作区</SidebarGroupLabel>
        <SidebarGroupContent>
          <SidebarMenu>
            <SidebarMenuItem v-for="item in navWorkspace" :key="item.to">
              <SidebarMenuButton as-child :is-active="isActive(item.to)" :tooltip="item.title">
                <RouterLink :to="item.to">
                  <component :is="item.icon" />
                  <span>{{ item.title }}</span>
                </RouterLink>
              </SidebarMenuButton>
              <SidebarMenuBadge v-if="item.trailing === 'status-ok'">
                <span class="status-dot status-dot-ok" />
              </SidebarMenuBadge>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarGroupContent>
      </SidebarGroup>

    </SidebarContent>

    <SidebarFooter>
      <SidebarMenu>
        <SidebarMenuItem>
          <Popover>
            <PopoverTrigger as-child>
              <SidebarMenuButton
                size="sm"
                class="data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground"
                :tooltip="`记忆中 · ${totals.sessions.toLocaleString()} 个会话`"
              >
                <span class="relative flex size-2 shrink-0 items-center justify-center">
                  <span class="absolute inline-flex size-full animate-ping rounded-full bg-emerald-500/60" />
                  <span class="relative inline-flex size-1.5 rounded-full bg-emerald-500" />
                </span>
                <span class="flex-1 truncate text-[12px]">
                  记忆中 ·
                  <span class="font-semibold tabular-nums text-foreground">
                    {{ formatNumber(totals.sessions) }}
                  </span>
                </span>
                <ChevronUp class="ml-auto size-3.5 opacity-50" />
              </SidebarMenuButton>
            </PopoverTrigger>
            <PopoverContent
              side="top"
              align="start"
              :side-offset="8"
              class="w-[260px] p-3"
            >
              <div class="space-y-3">
                <div class="flex items-center justify-between">
                  <div class="flex items-center gap-2">
                    <span class="status-dot status-dot-ok" />
                    <span class="text-[12px] font-medium">采集中</span>
                  </div>
                  <span class="text-[10px] text-muted-foreground">实时同步</span>
                </div>

                <div class="grid grid-cols-3 gap-2 rounded-md border bg-muted/30 p-2 text-center">
                  <Tooltip :delay-duration="80">
                    <TooltipTrigger as-child>
                      <div class="cursor-default">
                        <div class="text-[14px] font-bold tabular-nums">
                          {{ formatNumber(totals.sessions) }}
                        </div>
                        <div class="text-[10px] text-muted-foreground">会话</div>
                      </div>
                    </TooltipTrigger>
                    <TooltipContent>
                      <span class="tabular-nums">{{ totals.sessions.toLocaleString() }}</span>
                    </TooltipContent>
                  </Tooltip>
                  <Tooltip :delay-duration="80">
                    <TooltipTrigger as-child>
                      <div class="cursor-default">
                        <div class="text-[14px] font-bold tabular-nums">
                          {{ formatNumber(totals.messages) }}
                        </div>
                        <div class="text-[10px] text-muted-foreground">消息</div>
                      </div>
                    </TooltipTrigger>
                    <TooltipContent>
                      <span class="tabular-nums">{{ totals.messages.toLocaleString() }}</span>
                    </TooltipContent>
                  </Tooltip>
                  <div>
                    <div class="text-[14px] font-bold tabular-nums">{{ activeAdapters }} / {{ adapters.length }}</div>
                    <div class="text-[10px] text-muted-foreground">采集源</div>
                  </div>
                </div>

                <div class="space-y-1.5">
                  <div class="flex items-center justify-between text-[11px]">
                    <span class="text-muted-foreground">摘要进度</span>
                    <span class="font-medium text-emerald-600">{{ summaryPct }}%</span>
                  </div>
                  <div class="h-1 w-full overflow-hidden rounded-full bg-muted">
                    <div
                      class="h-full rounded-full bg-emerald-500"
                      :style="{ width: summaryPct + '%' }"
                    />
                  </div>
                  <div class="flex items-center justify-between text-[10px] text-muted-foreground">
                    <span>{{ daemonStatus.llmModel }}</span>
                    <span>{{ appVersion }}</span>
                  </div>
                </div>

                <Button
                  v-if="sessionsPending > 0"
                  variant="outline"
                  size="sm"
                  class="w-full gap-1 text-[11px]"
                  :disabled="batchSummarizing"
                  @click="runBatchSummarize"
                >
                  <Wand2 class="size-3" />
                  {{ batchSummarizing ? '触发中…' : `触发 ${sessionsPending} 个待摘要` }}
                </Button>
              </div>
            </PopoverContent>
          </Popover>
        </SidebarMenuItem>
      </SidebarMenu>
    </SidebarFooter>
  </Sidebar>
</template>
