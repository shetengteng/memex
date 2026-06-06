<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from '@/components/ui/breadcrumb'
import { Separator } from '@/components/ui/separator'
import { SidebarTrigger } from '@/components/ui/sidebar'
import { totals } from '@/mock/data'

const route = useRoute()

const crumbs = computed<string[]>(() => (route.meta?.breadcrumb as string[]) ?? [])
const isLibrary = computed(() => route.path === '/library')
const isInsights = computed(() => route.path === '/insights')
const isConnect = computed(() => route.path === '/connect')
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
      <Breadcrumb>
        <BreadcrumbList>
          <template v-for="(c, i) in crumbs" :key="i">
            <BreadcrumbItem>
              <BreadcrumbPage v-if="i === crumbs.length - 1">{{ c }}</BreadcrumbPage>
              <BreadcrumbLink v-else href="#">{{ c }}</BreadcrumbLink>
            </BreadcrumbItem>
            <BreadcrumbSeparator v-if="i < crumbs.length - 1" />
          </template>
        </BreadcrumbList>
      </Breadcrumb>

      <div
        v-if="isLibrary"
        class="hidden min-w-0 items-center gap-1.5 truncate text-[12px] text-muted-foreground md:flex"
      >
        <span class="text-muted-foreground/40">·</span>
        <span class="tabular-nums">
          <span class="font-medium text-foreground">{{ totals.sessions.toLocaleString() }}</span>
          <span class="ml-0.5">个会话</span>
        </span>
        <span class="text-muted-foreground/40">·</span>
        <span class="tabular-nums">
          <span class="font-medium text-foreground">{{ totals.messages.toLocaleString() }}</span>
          <span class="ml-0.5">条消息</span>
        </span>
      </div>

      <div
        v-else-if="isInsights"
        class="hidden min-w-0 items-center gap-1.5 truncate text-[12px] text-muted-foreground md:flex"
      >
        <span class="text-muted-foreground/40">·</span>
        <span class="truncate">AI 帮你回顾、提炼、找规律</span>
      </div>

      <div
        v-else-if="isConnect"
        class="hidden min-w-0 items-center gap-1.5 truncate text-[12px] text-muted-foreground md:flex"
      >
        <span class="text-muted-foreground/40">·</span>
        <span class="truncate">让你的 AI 编辑器记住一切</span>
      </div>
    </div>

    <div id="memex-header-center" class="flex items-center justify-center" />

    <div id="memex-header-actions" class="flex min-w-0 items-center justify-end gap-2" />
  </header>
</template>
