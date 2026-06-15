<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { AlertTriangle, Eraser, ShieldCheck } from 'lucide-vue-next'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { useMemex } from '@/composables/useMemex'
import { useI18n } from '@/i18n'
import { toast } from 'vue-sonner'

const { t } = useI18n()
const memex = useMemex()

const open = ref(false)
const leftoverCount = ref(0)
const cleaning = ref(false)

const FIRST_RUN_KEY = 'app.first_run_handled'

interface InstalledTriple {
  ide: string
  mcp: boolean
  skill: boolean
  hook: boolean
}

const triples = ref<InstalledTriple[]>([])

async function detectLeftovers() {
  // 任何步骤失败都视为"无法决定，跳过引导"——避免在 sidecar / config 异常时
  // 把用户卡在这个 dialog 里。
  try {
    const handled = await memex.getConfig(FIRST_RUN_KEY)
    if (handled === 'true') return

    const [ides, skills, hooks] = await Promise.all([
      memex.ideListStatus().catch(() => []),
      memex.skillListStatus().catch(() => []),
      memex.hookListStatus().catch(() => []),
    ])
    const skillMap = new Map(skills.map((s) => [s.ide, s.installed]))
    const hookMap = new Map(hooks.map((h) => [h.ide, h.installed]))
    const collected: InstalledTriple[] = ides.map((i) => ({
      ide: i.ide,
      mcp: i.installed,
      skill: !!skillMap.get(i.ide),
      hook: !!hookMap.get(i.ide),
    }))

    const total = collected.reduce(
      (n, r) => n + (r.mcp ? 1 : 0) + (r.skill ? 1 : 0) + (r.hook ? 1 : 0),
      0,
    )
    if (total === 0) {
      // 没残留 → 直接打首次启动 ack 标记，下次不再扫
      await memex.setConfig(FIRST_RUN_KEY, 'true').catch(() => {})
      return
    }
    triples.value = collected
    leftoverCount.value = total
    open.value = true
  } catch {
    /* swallow — 出错不弹 */
  }
}

async function keep() {
  open.value = false
  try {
    await memex.setConfig(FIRST_RUN_KEY, 'true')
  } catch {
    /* best-effort */
  }
}

/// 用户选择"全部清空"。逐项 uninstall MCP / SKILL / Hook，失败 best-effort 不阻塞，
/// 收集错误最终一条 toast。完成后写 first_run_handled=true。
async function cleanAll() {
  if (cleaning.value) return
  cleaning.value = true
  const errors: string[] = []
  for (const r of triples.value) {
    if (r.mcp) {
      try {
        await memex.ideUninstall(r.ide)
      } catch (e) {
        errors.push(`${r.ide} MCP: ${formatErr(e)}`)
      }
    }
    if (r.skill) {
      try {
        await memex.skillUninstall(r.ide)
      } catch (e) {
        errors.push(`${r.ide} SKILL: ${formatErr(e)}`)
      }
    }
    if (r.hook) {
      try {
        await memex.hookUninstall(r.ide)
      } catch (e) {
        errors.push(`${r.ide} Hook: ${formatErr(e)}`)
      }
    }
  }
  try {
    await memex.setConfig(FIRST_RUN_KEY, 'true')
  } catch {
    /* best-effort */
  }
  cleaning.value = false
  open.value = false
  if (errors.length === 0) {
    toast.success(t('connect.ide.toast.reset.success', { count: leftoverCount.value }))
  } else {
    toast.error(t('connect.ide.toast.reset.partial', { err: errors.slice(0, 3).join(' · ') }))
  }
}

function formatErr(e: unknown): string {
  if (typeof e === 'object' && e !== null && 'message' in e) {
    return String((e as { message: unknown }).message)
  }
  return String(e)
}

onMounted(() => {
  void detectLeftovers()
})
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-md">
      <div class="flex items-start gap-3">
        <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-amber-500/10">
          <AlertTriangle class="size-4 text-amber-500" />
        </div>
        <div class="flex-1 space-y-3">
          <DialogTitle class="text-sm font-semibold leading-snug">
            {{ t('connect.ide.first_run.title') }}
          </DialogTitle>
          <DialogDescription class="text-xs leading-relaxed text-muted-foreground">
            {{ t('connect.ide.first_run.body', { count: leftoverCount }) }}
          </DialogDescription>
        </div>
      </div>

      <div class="mt-4 flex items-center justify-end gap-2 border-t border-border/40 pt-3">
        <Button
          variant="ghost"
          size="sm"
          class="h-7 gap-1 text-xs"
          :disabled="cleaning"
          @click="keep"
        >
          <ShieldCheck class="size-3" />
          {{ t('connect.ide.first_run.action.keep') }}
        </Button>
        <Button
          variant="default"
          size="sm"
          class="h-7 gap-1 text-xs"
          :disabled="cleaning"
          @click="cleanAll"
        >
          <Eraser :class="['size-3', cleaning && 'animate-spin']" />
          {{ cleaning ? t('connect.ide.action.resetting') : t('connect.ide.first_run.action.clean') }}
        </Button>
      </div>
    </DialogContent>
  </Dialog>
</template>
