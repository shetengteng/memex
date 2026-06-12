<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { Loader2, Radio, RefreshCw } from 'lucide-vue-next'
import IdeDot from '@/components/shell/IdeDot.vue'
import { toast } from 'vue-sonner'
import { adapters, breakdownByAdapter, refreshBreakdown } from '@/stores/memex'
import { useMemex } from '@/composables/useMemex'
import { toastBackendError } from '@/lib/toast-error'

const memex = useMemex()
const activeCount = computed(() => adapters.filter((a) => a.status === 'active').length)
const totalAdapters = computed(() => adapters.length)

// 真实的"按 adapter 切的会话数"住在 breakdownByAdapter 这个 reactive map 里，
// 由 initMemexStore / refreshBreakdown 通过 IPC 拉来。adapter 自身的 a.sessions 字段
// 来自原型默认值，永远是 0，不能拿来展示真实计数。
function sessionCountFor(id: string): number {
  return breakdownByAdapter[id] ?? 0
}

const rescanning = ref<Record<string, boolean>>({})
const toggling = ref<Record<string, boolean>>({})
const globalScanning = ref(false)

// 启动时把每个 adapter 的真实 enabled 状态从 config 拉一遍，
// 并主动刷一次 breakdown 保证"个会话"列即使从其他路由切过来也是新鲜的。
// 写 a.status 时做幂等检查，避免触发不必要的响应式更新（reka-ui Switch 在
// model-value 频繁切换时可能误触发 update 事件）。
onMounted(async () => {
  await Promise.all([
    refreshBreakdown().catch(() => {}),
    ...adapters.map(async (a) => {
      try {
        const v = await memex.getConfig(`adapter.${a.id}.enabled`)
        if (v == null) return
        const next: 'active' | 'disabled' = v === 'true' ? 'active' : 'disabled'
        if (a.status !== next) a.status = next
      } catch {
        /* ignore */
      }
    }),
  ])
})

async function toggleAdapter(id: string, enabled: boolean) {
  // 防抖：当 Switch 因 onMounted 拉取 config 后状态被同步，可能在某些 reka-ui 内部时序下
  // 误回吐一次 update:model-value。如果新值与当前 store 状态一致，跳过 IPC + toast。
  const target = adapters.find((a) => a.id === id)
  const currentEnabled = target?.status === 'active'
  if (target && currentEnabled === enabled) {
    return
  }

  toggling.value[id] = true
  try {
    await memex.toggleAdapter(id, enabled)
    if (target) target.status = enabled ? 'active' : 'disabled'
    toast.success(`${id} 已${enabled ? '启用' : '停用'}`)
  } catch (e) {
    toastBackendError('切换失败', e)
  } finally {
    toggling.value[id] = false
  }
}

async function rescanAdapter(id: string) {
  rescanning.value[id] = true
  try {
    const r = await memex.triggerIngest(id)
    toast.success(`${id} 采集完成：${r.messages_ingested} 条消息`)
    await refreshBreakdown()
  } catch (e) {
    toastBackendError(`${id} 采集失败`, e)
  } finally {
    rescanning.value[id] = false
  }
}

async function rescanAll() {
  if (globalScanning.value) return
  globalScanning.value = true
  try {
    const r = await memex.triggerIngest()
    toast.success(`采集完成：${r.messages_ingested} 条消息`)
    await refreshBreakdown()
  } catch (e) {
    toastBackendError('采集失败', e)
  } finally {
    globalScanning.value = false
  }
}
</script>

<template>
  <section>
    <div class="mb-3 flex items-end justify-between">
      <div>
        <div class="flex items-center gap-2">
          <Radio class="size-3.5" :style="{ color: 'var(--success)' }" />
          <h2 class="text-[15px] font-semibold">采集源</h2>
          <Badge class="border-emerald-500/30 bg-emerald-500/10 text-emerald-700">
            {{ activeCount }} / {{ totalAdapters }} 个启用
          </Badge>
        </div>
        <p class="mt-0.5 text-[11px] text-muted-foreground">
          监听 IDE 会话目录，2 秒内自动入库
        </p>
      </div>
      <Button variant="outline" size="sm" class="h-8 gap-1.5" :disabled="globalScanning" @click="rescanAll">
        <RefreshCw :class="['size-3.5', globalScanning && 'animate-spin']" />
        {{ globalScanning ? '扫描中…' : '立即扫描' }}
      </Button>
    </div>

    <Card class="overflow-hidden p-0">
      <div
        v-for="(a, i) in adapters"
        :key="a.id"
        class="flex items-center gap-3 px-4 py-3"
        :class="i < adapters.length - 1 && 'border-b'"
      >
        <IdeDot :adapter="a.id" size="lg" />
        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-2 text-[13px] font-semibold">
            {{ a.label }}
            <Badge
              v-if="a.status === 'active'"
              class="border-emerald-500/30 bg-emerald-500/10 text-emerald-700"
            >
              ● 已启用
            </Badge>
            <Badge v-else variant="outline" class="text-muted-foreground">○ 未启用</Badge>
          </div>
          <div class="truncate font-mono text-[11px] text-muted-foreground">{{ a.path }}</div>
        </div>
        <div
          class="text-right"
          :class="a.status === 'disabled' && sessionCountFor(a.id) === 0 && 'opacity-50'"
        >
          <div class="text-[13px] font-semibold tabular-nums">
            {{ sessionCountFor(a.id) === 0 ? '—' : sessionCountFor(a.id).toLocaleString() }}
          </div>
          <div class="text-[10px] text-muted-foreground">个会话</div>
        </div>
        <Switch
          :model-value="a.status === 'active'"
          :disabled="toggling[a.id]"
          @update:model-value="(v: boolean) => toggleAdapter(a.id, v)"
        />
        <Tooltip>
          <TooltipTrigger as-child>
            <Button
              variant="ghost"
              size="icon"
              class="size-7"
              :disabled="a.status !== 'active' || rescanning[a.id]"
              @click="rescanAdapter(a.id)"
            >
              <Loader2 v-if="rescanning[a.id]" class="size-3.5 animate-spin" />
              <RefreshCw v-else class="size-3.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="top" class="text-[11px]">重扫该采集源</TooltipContent>
        </Tooltip>
      </div>
    </Card>
  </section>
</template>
