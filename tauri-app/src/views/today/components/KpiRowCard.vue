<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { Card } from '@/components/ui/card'
import { useMemex } from '@/composables/useMemex'

interface KpiBucket {
  label: string
  days: number
  sessions: number | null
  messages: number | null
  loading: boolean
}

const memex = useMemex()

const buckets = ref<KpiBucket[]>([
  { label: '今天', days: 1, sessions: null, messages: null, loading: true },
  { label: '本周', days: 7, sessions: null, messages: null, loading: true },
  { label: '本月', days: 30, sessions: null, messages: null, loading: true },
])

async function loadOne(idx: number) {
  const b = buckets.value[idx]
  b.loading = true
  try {
    const report = await memex.getWorkload(b.days)
    b.sessions = report.overall.sessions ?? 0
    b.messages = report.overall.messages ?? 0
  } catch (e) {
    console.warn(`[KpiRowCard] getWorkload(${b.days}) failed`, e)
    b.sessions = 0
    b.messages = 0
  } finally {
    b.loading = false
  }
}

async function loadAll() {
  await Promise.all(buckets.value.map((_, i) => loadOne(i)))
}

function onTodayRefresh() {
  void loadAll()
}

onMounted(() => {
  void loadAll()
  window.addEventListener('today-refresh', onTodayRefresh)
})

onBeforeUnmount(() => {
  window.removeEventListener('today-refresh', onTodayRefresh)
})

function fmt(n: number | null): string {
  if (n === null) return '—'
  return n.toLocaleString()
}
</script>

<template>
  <div class="grid grid-cols-1 gap-4 sm:grid-cols-3">
    <Card v-for="b in buckets" :key="b.label" class="p-4">
      <div class="mb-1 text-[11px] tracking-wider text-muted-foreground">
        {{ b.label }}
      </div>
      <div class="flex items-baseline gap-3">
        <div class="flex items-baseline gap-1">
          <span class="text-2xl font-bold tabular-nums">{{ fmt(b.sessions) }}</span>
          <span class="text-[11px] text-muted-foreground">会话</span>
        </div>
        <div class="flex items-baseline gap-1 text-muted-foreground">
          <span class="text-[14px] tabular-nums">{{ fmt(b.messages) }}</span>
          <span class="text-[11px]">条消息</span>
        </div>
      </div>
    </Card>
  </div>
</template>
