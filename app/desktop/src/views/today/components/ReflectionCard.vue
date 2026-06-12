<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { Card } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { ArrowRight, Lightbulb, Plus } from 'lucide-vue-next'
import { useMemex } from '@/composables/useMemex'
import { useI18n } from '@/i18n'
import type { ReflectEntry } from '@/types'

const router = useRouter()
const memex = useMemex()
const { t, locale } = useI18n()
const entries = ref<ReflectEntry[]>([])

interface ReflectionItemView {
  title: string
  chip?: string
  body: string
  dashed: boolean
  icon: typeof ArrowRight
  scopeKey?: string
}

onMounted(async () => {
  try {
    entries.value = await memex.reflectList()
  } catch (e) {
    console.warn('[ReflectionCard] reflectList failed', e)
  }
})

const items = computed<ReflectionItemView[]>(() => {
  const xs = entries.value.slice(0, 2).map<ReflectionItemView>((e) => ({
    title: e.title ?? e.scope_key,
    chip: t('today.reflect.summary_count', { count: e.digest_count }),
    body: e.scope_key,
    dashed: false,
    icon: ArrowRight,
    scopeKey: e.scope_key,
  }))
  if (xs.length < 2) {
    xs.push({
      title: t('today.reflect.new_title'),
      body: t('today.reflect.new_body'),
      dashed: true,
      icon: Plus,
    })
  }
  return xs
})

const latestText = computed(() => {
  const top = entries.value[0]
  if (!top) return t('today.reflect.empty')
  const tag = locale.value === 'en' ? 'en-US' : 'zh-CN'
  return t('today.reflect.latest', { time: new Date(top.created_at).toLocaleString(tag) })
})

function openItem(item: ReflectionItemView) {
  if (item.scopeKey) router.push(`/insights?reflect=${encodeURIComponent(item.scopeKey)}`)
  else router.push('/insights')
}
</script>

<template>
  <Card class="flex flex-col p-5">
    <div class="mb-3 flex items-center justify-between">
      <div class="flex items-center gap-2">
        <Lightbulb class="size-4" :style="{ color: 'var(--warning)' }" />
        <h3 class="text-[14px] font-semibold">{{ t('today.reflect.heading') }}</h3>
      </div>
      <Badge class="border-amber-500/30 bg-amber-500/10 text-amber-700">{{ t('today.reflect.count_badge', { count: items.length }) }}</Badge>
    </div>

    <div class="mb-4 space-y-2">
      <button
        v-for="(item, idx) in items"
        :key="idx"
        :class="[
          'group w-full rounded-lg border p-3 text-left transition-colors hover:bg-accent',
          item.dashed && 'border-dashed',
        ]"
        @click="openItem(item)"
      >
        <div class="mb-1 flex items-center justify-between">
          <span class="text-[13px] font-semibold">{{ item.title }}</span>
          <Badge v-if="item.chip" variant="outline">{{ item.chip }}</Badge>
          <component v-else :is="item.icon" class="size-3.5 text-muted-foreground" />
        </div>
        <p class="truncate text-[12px] text-muted-foreground">{{ item.body }}</p>
      </button>
    </div>

    <div class="mt-auto flex items-center justify-between border-t pt-3">
      <span class="text-[11px] text-muted-foreground">{{ latestText }}</span>
      <Button variant="ghost" size="sm" class="h-7 gap-1 text-xs" @click="router.push('/insights')">
        {{ t('today.reflect.all_button') }}
        <ArrowRight class="size-3" />
      </Button>
    </div>
  </Card>
</template>
