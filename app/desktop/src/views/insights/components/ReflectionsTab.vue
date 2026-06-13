<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import MarkdownContent from '@/components/MarkdownContent.vue'
import { BrainCircuit, ChevronRight } from 'lucide-vue-next'
import { toast } from 'vue-sonner'
import type { ReflectEntry, ReflectDetail } from '@/types'
import { useMemex } from '@/composables/useMemex'
import { humanizeBackendError } from '@/lib/utils'
import { useI18n } from '@/i18n'

const { t, locale } = useI18n()
const router = useRouter()
const memex = useMemex()
const entries = ref<ReflectEntry[]>([])
const period = ref('7d')
const running = ref(false)

const openDetail = ref(false)
const detail = ref<ReflectDetail | null>(null)
const detailLoading = ref(false)

async function loadEntries() {
  try {
    entries.value = await memex.reflectList()
  } catch (e) {
    console.warn('[ReflectionsTab] reflectList failed', e)
  }
}

onMounted(loadEntries)

async function runReflect() {
  if (running.value) return
  running.value = true
  try {
    const r = await memex.reflectRun(period.value)
    toast.success(t('insights.reflect.toast.generated', { key: r.scope_key }))
    await loadEntries()
  } catch (e) {
    const fe = humanizeBackendError(e)
    toast.error(t('insights.reflect.toast.failed'), {
      description: fe.friendly,
      action: fe.action
        ? { label: fe.action.label, onClick: () => router.push(fe.action!.route) }
        : undefined,
      duration: 8000,
    })
  } finally {
    running.value = false
  }
}

async function openEntry(e: ReflectEntry) {
  openDetail.value = true
  detail.value = null
  detailLoading.value = true
  try {
    detail.value = await memex.reflectGet(e.scope_key)
  } catch (err) {
    console.warn('[ReflectionsTab] reflectGet failed', err)
  } finally {
    detailLoading.value = false
  }
}

const fmtTime = (iso: string) =>
  new Date(iso).toLocaleString(locale.value === 'zh' ? 'zh-CN' : 'en-US', {
    dateStyle: 'short',
    timeStyle: 'short',
  })
</script>

<template>
  <div class="mx-auto w-full max-w-5xl space-y-4 px-4 py-4 lg:px-6 lg:py-6">
    <Card class="p-5">
      <div class="mb-3 flex items-center gap-2">
        <BrainCircuit class="size-4 text-primary" />
        <h3 class="text-[14px] font-semibold">{{ t('insights.reflect.title') }}</h3>
      </div>
      <div class="flex items-center gap-2">
        <span class="text-[12px] text-muted-foreground">{{ t('insights.reflect.range_label') }}</span>
        <Select v-model="period">
          <SelectTrigger class="h-8 w-40">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="3d">{{ t('insights.reflect.range.3d') }}</SelectItem>
            <SelectItem value="7d">{{ t('insights.reflect.range.7d') }}</SelectItem>
            <SelectItem value="30d">{{ t('insights.reflect.range.30d') }}</SelectItem>
          </SelectContent>
        </Select>
        <Button size="sm" class="h-8 gap-1.5" :disabled="running" @click="runReflect">
          <BrainCircuit class="size-3.5" />
          {{ running ? t('insights.reflect.action.busy') : t('insights.reflect.action.start') }}
        </Button>
        <span class="ml-2 text-[11px] italic text-muted-foreground">{{ t('insights.reflect.hint') }}</span>
      </div>
    </Card>

    <div class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
      {{ t('insights.reflect.history') }}
    </div>
    <Card class="overflow-hidden">
      <ul>
        <li v-for="(r, idx) in entries" :key="r.scope_key">
          <button
            class="group flex w-full items-center justify-between gap-3 px-4 py-3.5 text-left transition-colors hover:bg-accent/40"
            :class="idx < entries.length - 1 && 'border-b'"
            @click="openEntry(r)"
          >
            <div class="min-w-0 flex-1">
              <div class="mb-1 flex items-center gap-2">
                <span class="text-[14px] font-semibold">{{ r.title ?? r.scope_key }}</span>
                <Badge variant="secondary">{{ t('insights.reflect.list.digest_count', { n: r.digest_count }) }}</Badge>
              </div>
              <p class="line-clamp-1 text-[12px] text-muted-foreground">
                {{ r.scope_key }} · {{ fmtTime(r.created_at) }}
              </p>
            </div>
            <ChevronRight class="size-4 shrink-0 text-muted-foreground" />
          </button>
        </li>
        <li v-if="!entries.length" class="px-4 py-6 text-center text-[12px] italic text-muted-foreground">
          {{ t('insights.reflect.list.empty') }}
        </li>
      </ul>
    </Card>

    <Dialog v-model:open="openDetail">
      <!--
        ! 前缀是为了覆盖 shadcn-vue DialogContent 默认的 sm:max-w-sm（~384px），
        与 LibrarySessionDrawer 的 DialogContent 保持完全一致的宽度（~896px）。
      -->
      <DialogContent class="w-[92vw] !max-w-4xl">
        <DialogHeader>
          <DialogTitle>{{ detail?.title ?? detail?.scope_key ?? t('insights.reflect.detail.fallback_title') }}</DialogTitle>
        </DialogHeader>
        <p v-if="detailLoading" class="text-center text-[12px] text-muted-foreground">{{ t('insights.reflect.detail.loading') }}</p>
        <div v-else-if="detail" class="max-h-[70vh] space-y-4 overflow-y-auto pr-2">
          <MarkdownContent :content="detail.markdown" />
          <div v-if="detail.patterns.length">
            <div class="mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
              {{ t('insights.reflect.detail.section.patterns') }}
            </div>
            <ul class="space-y-1.5 text-[13px]">
              <li v-for="p in detail.patterns" :key="p" class="flex gap-2">
                <span class="mt-2 size-1 shrink-0 rounded-full bg-primary" />
                <span>{{ p }}</span>
              </li>
            </ul>
          </div>
          <div v-if="detail.open_loops.length">
            <div class="mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
              {{ t('insights.reflect.detail.section.open_loops') }}
            </div>
            <ul class="space-y-1.5 text-[13px]">
              <li v-for="o in detail.open_loops" :key="o" class="flex gap-2">
                <span class="mt-2 size-1 shrink-0 rounded-full bg-amber-500" />
                <span>{{ o }}</span>
              </li>
            </ul>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  </div>
</template>
