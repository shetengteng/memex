<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
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
import {
  ChevronUp,
  Library,
  Plug,
  Settings as SettingsIcon,
  Sparkles,
  Sunrise,
} from '@lucide/vue'
import { totals, daemonStatus, adapters } from '@/mock/data'

interface NavItem {
  title: string
  to: string
  icon: any
  badge?: string | number
  trailing?: 'status-ok'
}

const route = useRoute()

const navMain: NavItem[] = [
  { title: '今天', to: '/today', icon: Sunrise, badge: 12 },
  { title: '资料库', to: '/library', icon: Library, badge: totals.sessions.toLocaleString() },
  { title: '洞察', to: '/insights', icon: Sparkles },
]

const navWorkspace: NavItem[] = [
  { title: '连接', to: '/connect', icon: Plug, trailing: 'status-ok' },
  { title: '设置', to: '/settings', icon: SettingsIcon },
]

const isActive = (to: string) => route.path === to || route.path.startsWith(to + '/')

const activeAdapters = computed(() => adapters.filter((a) => a.status === 'active').length)
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
              <SidebarMenuBadge v-if="item.badge !== undefined" class="tabular-nums">
                {{ item.badge }}
              </SidebarMenuBadge>
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
                    {{ totals.sessions.toLocaleString() }}
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
                  <div>
                    <div class="text-[14px] font-bold tabular-nums">
                      {{ totals.sessions.toLocaleString() }}
                    </div>
                    <div class="text-[10px] text-muted-foreground">会话</div>
                  </div>
                  <div>
                    <div class="text-[14px] font-bold tabular-nums">
                      {{ totals.messages.toLocaleString() }}
                    </div>
                    <div class="text-[10px] text-muted-foreground">消息</div>
                  </div>
                  <div>
                    <div class="text-[14px] font-bold tabular-nums">{{ activeAdapters }} / {{ adapters.length }}</div>
                    <div class="text-[10px] text-muted-foreground">采集源</div>
                  </div>
                </div>

                <div class="space-y-1.5">
                  <div class="flex items-center justify-between text-[11px]">
                    <span class="text-muted-foreground">摘要进度</span>
                    <span class="font-medium text-emerald-600">98%</span>
                  </div>
                  <div class="h-1 w-full overflow-hidden rounded-full bg-muted">
                    <div class="h-full w-[98%] rounded-full bg-emerald-500" />
                  </div>
                  <div class="flex items-center justify-between text-[10px] text-muted-foreground">
                    <span>{{ daemonStatus.llmModel }}</span>
                    <span>v0.3.4</span>
                  </div>
                </div>
              </div>
            </PopoverContent>
          </Popover>
        </SidebarMenuItem>
      </SidebarMenu>
    </SidebarFooter>
  </Sidebar>
</template>
