<script setup lang="ts">
import { LayoutDashboard, List, Search } from 'lucide-vue-next'
import { Separator } from '@/components/ui/separator'

export type DashTab = 'overview' | 'sessions' | 'search' | 'session-detail'

defineProps<{ activeTab: DashTab }>()
const emit = defineEmits<{ switchTab: [tab: DashTab] }>()

const navItems: { key: DashTab; icon: typeof LayoutDashboard; label: string }[] = [
  { key: 'overview', icon: LayoutDashboard, label: 'Dashboard' },
  { key: 'sessions', icon: List, label: 'Sessions' },
  { key: 'search', icon: Search, label: 'Search' },
]
</script>

<template>
  <div class="flex w-52 shrink-0 flex-col border-r border-border bg-card">
    <div class="px-4 py-5">
      <h1 class="text-lg font-extrabold tracking-widest"><span class="text-primary">M</span>EMEX</h1>
      <p class="mt-0.5 text-[11px] text-muted-foreground">AI Session Memory</p>
    </div>
    <Separator />
    <nav class="flex-1 space-y-0.5 p-2">
      <button
        v-for="item in navItems"
        :key="item.key"
        class="flex w-full items-center gap-2.5 rounded-md px-3 py-2 text-sm font-medium transition-colors"
        :class="activeTab === item.key || (activeTab === 'session-detail' && item.key === 'sessions')
          ? 'bg-primary/10 text-primary'
          : 'text-muted-foreground hover:bg-accent hover:text-foreground'"
        @click="emit('switchTab', item.key)"
      >
        <component :is="item.icon" class="h-4 w-4" />
        {{ item.label }}
      </button>
    </nav>
    <Separator />
    <div class="px-4 py-3 text-[10px] text-muted-foreground">Memex v0.1.0 · Local Only</div>
  </div>
</template>
