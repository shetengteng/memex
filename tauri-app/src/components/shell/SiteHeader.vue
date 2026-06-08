<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from '@/components/ui/breadcrumb'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import { SidebarTrigger } from '@/components/ui/sidebar'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { Bell, Search } from 'lucide-vue-next'
import { toast } from 'vue-sonner'
import { totals } from '@/stores/memex'
import { formatNumber } from '@/lib/utils'
import { useCommandPalette } from '@/composables/useCommandPalette'

const route = useRoute()
const { open: openPalette } = useCommandPalette()

const crumbs = computed<string[]>(() => (route.meta?.breadcrumb as string[]) ?? [])
const isToday = computed(() => route.path === '/today')
const isLibrary = computed(() => route.path === '/library')
const isInsights = computed(() => route.path === '/insights')
const isConnect = computed(() => route.path === '/connect')

function onBellClick() {
  // 通知系统尚未接入；保留按钮位置 + 给个明确 hint，让用户知道这里规划了功能。
  toast.message('通知中心正在路上', {
    description: '后续会接入摘要/反思生成完成、采集失败等系统通知。',
  })
}
</script>

<template>
  <header
    class="grid h-(--header-height) shrink-0 grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-center gap-3 border-b px-4 transition-[width,height] ease-linear group-has-data-[collapsible=icon]/sidebar-wrapper:h-12 lg:px-6"
  >
    <div class="flex min-w-0 items-center gap-3">
      <SidebarTrigger class="-ml-1" />
      <Separator
        orientation="vertical"
        class="data-[orientation=vertical]:h-5 data-[orientation=vertical]:self-center"
      />
      <Breadcrumb class="shrink-0">
        <BreadcrumbList class="flex-nowrap whitespace-nowrap">
          <template v-for="(c, i) in crumbs" :key="i">
            <BreadcrumbItem class="whitespace-nowrap">
              <BreadcrumbPage v-if="i === crumbs.length - 1" class="whitespace-nowrap">{{ c }}</BreadcrumbPage>
              <span v-else class="whitespace-nowrap text-muted-foreground/70">{{ c }}</span>
            </BreadcrumbItem>
            <BreadcrumbSeparator v-if="i < crumbs.length - 1" />
          </template>
        </BreadcrumbList>
      </Breadcrumb>

      <div
        v-if="isLibrary"
        class="hidden min-w-0 items-center gap-1.5 truncate whitespace-nowrap text-[12px] text-muted-foreground md:flex"
      >
        <span class="text-muted-foreground/40">·</span>
        <Tooltip :delay-duration="80">
          <TooltipTrigger as-child>
            <span class="cursor-default tabular-nums underline-offset-2 hover:underline">
              <span class="font-medium text-foreground">{{ formatNumber(totals.sessions) }}</span>
              <span class="ml-0.5">个会话</span>
            </span>
          </TooltipTrigger>
          <TooltipContent side="bottom" :side-offset="4">
            <span class="tabular-nums">{{ totals.sessions.toLocaleString() }} 个会话</span>
          </TooltipContent>
        </Tooltip>
        <span class="text-muted-foreground/40">·</span>
        <Tooltip :delay-duration="80">
          <TooltipTrigger as-child>
            <span class="cursor-default tabular-nums underline-offset-2 hover:underline">
              <span class="font-medium text-foreground">{{ formatNumber(totals.messages) }}</span>
              <span class="ml-0.5">条消息</span>
            </span>
          </TooltipTrigger>
          <TooltipContent side="bottom" :side-offset="4">
            <span class="tabular-nums">{{ totals.messages.toLocaleString() }} 条消息</span>
          </TooltipContent>
        </Tooltip>
      </div>

      <div
        v-else-if="isInsights"
        class="hidden min-w-0 items-center gap-1.5 truncate whitespace-nowrap text-[12px] text-muted-foreground md:flex"
      >
        <span class="text-muted-foreground/40">·</span>
        <span class="truncate">AI 帮你回顾、提炼、找规律</span>
      </div>

      <div
        v-else-if="isConnect"
        class="hidden min-w-0 items-center gap-1.5 truncate whitespace-nowrap text-[12px] text-muted-foreground md:flex"
      >
        <span class="text-muted-foreground/40">·</span>
        <span class="truncate">让你的 AI 编辑器记住一切</span>
      </div>
    </div>

    <div id="memex-header-center" class="flex items-center justify-center" />

    <div class="flex min-w-0 items-center justify-end gap-2">
      <!-- 页面级 actions slot（settings 的 tabs 等用 Teleport 注入此处） -->
      <div id="memex-header-actions" class="flex min-w-0 items-center gap-2" />

      <!-- 全局搜索 + 通知按钮：仅在 Today 页展示。其他页面（Library / Insights / Connect / Settings）保持 header 干净。 -->
      <template v-if="isToday">
        <Button
          variant="outline"
          size="sm"
          class="hidden h-8 min-w-[220px] justify-between gap-2 px-3 text-[12px] text-muted-foreground hover:text-foreground md:flex"
          @click="openPalette"
        >
          <span class="flex items-center gap-2">
            <Search class="size-3.5" />
            搜索会话、项目、命令…
          </span>
          <kbd class="rounded border bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">⌘K</kbd>
        </Button>

        <Tooltip :delay-duration="200">
          <TooltipTrigger as-child>
            <Button
              variant="ghost"
              size="icon"
              class="size-8 text-muted-foreground hover:text-foreground"
              aria-label="通知中心"
              @click="onBellClick"
            >
              <Bell class="size-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="bottom">通知中心（开发中）</TooltipContent>
        </Tooltip>
      </template>
    </div>
  </header>
</template>
