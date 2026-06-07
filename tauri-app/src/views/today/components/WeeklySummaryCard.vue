<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ArrowRight, BrainCircuit } from 'lucide-vue-next'
import { useMemex } from '@/composables/useMemex'
import type { AggregateSummary } from '@/types'

const router = useRouter()
const memex = useMemex()
const latest = ref<AggregateSummary | null>(null)
const loading = ref(true)

onMounted(async () => {
  try {
    const xs = await memex.listReports('weekly', 1)
    latest.value = xs[0] ?? null
  } catch (e) {
    console.warn('[WeeklySummaryCard] listReports failed', e)
  } finally {
    loading.value = false
  }
})

// 拆分摘要 markdown 第一段当 body
const body = computed(() => {
  const md = latest.value?.summary ?? ''
  if (!md) return ''
  // 取前 3 段非空文本
  const segs = md.split(/\n{2,}/).filter((s) => s.trim() && !s.startsWith('#'))
  return segs.slice(0, 2).join('\n\n')
})

const decisions = computed(() => latest.value?.decisions.slice(0, 3) ?? [])
const topicsDisplay = computed(() => {
  const ts = latest.value?.topics ?? []
  const shown = ts.slice(0, 3)
  const rest = ts.length - shown.length
  return rest > 0 ? [...shown, `+${rest}`] : shown
})

// scope_key 形如 "2026-W23"，提取尾部当 badge
const weekBadge = computed(() => {
  const k = latest.value?.scope_key ?? ''
  const m = k.match(/W(\d+)/)
  return m ? `第 ${m[1]} 周` : k
})
</script>

<template>
  <Card class="flex flex-col p-5">
    <div class="mb-3 flex items-center justify-between">
      <div class="flex items-center gap-2">
        <BrainCircuit class="size-4" :style="{ color: 'var(--adapter-claude)' }" />
        <h3 class="text-[14px] font-semibold">本周自动摘要</h3>
        <Badge variant="secondary">L3</Badge>
      </div>
      <span class="text-[11px] text-muted-foreground">{{ weekBadge }}</span>
    </div>

    <p v-if="latest" class="mb-3 text-[13px] text-muted-foreground">
      {{ latest.session_count }} 个会话 · {{ latest.title ?? '本周摘要' }}
    </p>
    <p v-else-if="loading" class="mb-3 text-[13px] text-muted-foreground">加载中…</p>
    <p v-else class="mb-3 text-[13px] text-muted-foreground">本周还没有自动摘要，可去洞察页生成。</p>

    <p v-if="body" class="mb-4 whitespace-pre-line text-[13px] leading-relaxed">{{ body }}</p>

    <div v-if="decisions.length" class="mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
      关键决策
    </div>
    <ul v-if="decisions.length" class="mb-4 space-y-1.5 text-[13px]">
      <li v-for="d in decisions" :key="d" class="flex gap-2">
        <span class="mt-2 size-1 shrink-0 rounded-full bg-primary" />
        <span>{{ d }}</span>
      </li>
    </ul>

    <div class="mt-auto flex items-center justify-between border-t pt-3">
      <div class="flex flex-wrap items-center gap-1.5">
        <Badge v-for="t in topicsDisplay" :key="t" variant="secondary">{{ t }}</Badge>
      </div>
      <Button variant="ghost" size="sm" class="h-7 gap-1 text-xs" @click="router.push('/insights')">
        完整摘要
        <ArrowRight class="size-3" />
      </Button>
    </div>
  </Card>
</template>
