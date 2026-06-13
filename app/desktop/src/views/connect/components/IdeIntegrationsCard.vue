<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import { Switch } from '@/components/ui/switch'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { Puzzle, RefreshCw } from 'lucide-vue-next'
import IdeDot from '@/components/shell/IdeDot.vue'
import { toast } from 'vue-sonner'
import { useMemex } from '@/composables/useMemex'
import { parseBackendError } from '@/lib/utils'
import { useI18n } from '@/i18n'
import type { IdeStatus, SkillStatus, HookStatus } from '@/types'

const { t } = useI18n()

interface IdeRow {
  id: string
  label: string
  configPath: string
  mcpInstalled: boolean
  skillInstalled: boolean
  hookSupported: boolean
  hookInstalled: boolean
}

const memex = useMemex()
const rows = ref<IdeRow[]>([])
const loading = ref(false)
const busy = ref<Record<string, boolean>>({})

// 注意：CLI 返回的 IDE id 是 `claude-code`（横线），不是 `claude_code`。
// 旧版误用下划线导致 Claude Code 一行 fallback 显示成 "claude-code" 小写裸串。
const IDE_LABEL: Record<string, string> = {
  cursor: 'Cursor',
  'claude-code': 'Claude Code',
  codex: 'Codex',
  opencode: 'OpenCode',
}

async function loadStatus() {
  loading.value = true
  try {
    const [ides, skills, hooks] = await Promise.all([
      memex.ideListStatus().catch(() => [] as IdeStatus[]),
      memex.skillListStatus().catch(() => [] as SkillStatus[]),
      memex.hookListStatus().catch(() => [] as HookStatus[]),
    ])
    const skillMap = new Map(skills.map((s) => [s.ide, s]))
    const hookMap = new Map(hooks.map((h) => [h.ide, h]))
    rows.value = ides.map<IdeRow>((i) => ({
      id: i.ide,
      label: IDE_LABEL[i.ide] ?? i.ide,
      configPath: i.config_path,
      mcpInstalled: i.installed,
      skillInstalled: skillMap.get(i.ide)?.installed ?? false,
      hookSupported: hookMap.get(i.ide)?.supported ?? false,
      hookInstalled: hookMap.get(i.ide)?.installed ?? false,
    }))
  } catch (e) {
    console.warn('[IdeIntegrationsCard] loadStatus failed', e)
  } finally {
    loading.value = false
  }
}

onMounted(loadStatus)

const installedIdeCount = computed(
  () => rows.value.filter((r) => r.mcpInstalled && r.skillInstalled).length,
)

function formatToggleError(e: unknown): string {
  const parsed = parseBackendError(e)
  return parsed.message || parsed.kind
}

function actionLabel(installed: boolean): string {
  return installed
    ? t('connect.ide.toast.action.installed')
    : t('connect.ide.toast.action.uninstalled')
}

async function toggleMcp(row: IdeRow, next: boolean) {
  busy.value[row.id] = true
  try {
    const s = next ? await memex.ideInstall(row.id) : await memex.ideUninstall(row.id)
    row.mcpInstalled = s.installed
    toast.success(t('connect.ide.toast.mcp_action', { label: row.label, action: actionLabel(next) }))
  } catch (e) {
    toast.error(t('connect.ide.toast.mcp_failed', { label: row.label, err: formatToggleError(e) }))
  } finally {
    busy.value[row.id] = false
  }
}

async function toggleSkill(row: IdeRow, next: boolean) {
  busy.value[row.id + ':skill'] = true
  try {
    const s = next ? await memex.skillInstall(row.id) : await memex.skillUninstall(row.id)
    row.skillInstalled = s.installed
    toast.success(t('connect.ide.toast.skill_action', { label: row.label, action: actionLabel(next) }))
  } catch (e) {
    toast.error(t('connect.ide.toast.skill_failed', { label: row.label, err: formatToggleError(e) }))
  } finally {
    busy.value[row.id + ':skill'] = false
  }
}

async function toggleHook(row: IdeRow, next: boolean) {
  busy.value[row.id + ':hook'] = true
  try {
    const s = next ? await memex.hookInstall(row.id) : await memex.hookUninstall(row.id)
    row.hookInstalled = s.installed
    toast.success(t('connect.ide.toast.hook_action', { label: row.label, action: actionLabel(next) }))
  } catch (e) {
    toast.error(t('connect.ide.toast.hook_failed', { label: row.label, err: formatToggleError(e) }))
  } finally {
    busy.value[row.id + ':hook'] = false
  }
}
</script>

<template>
  <section>
    <div class="mb-3 flex items-end justify-between">
      <div>
        <div class="flex items-center gap-2">
          <Puzzle class="size-3.5" :style="{ color: 'var(--adapter-claude)' }" />
          <h2 class="text-[15px] font-semibold">{{ t('connect.ide.title') }}</h2>
          <Badge variant="secondary" class="text-[10px]">
            {{ t('connect.ide.summary', { installed: installedIdeCount, total: rows.length }) }}
          </Badge>
        </div>
        <p class="mt-0.5 text-[11px] text-muted-foreground">
          {{ t('connect.ide.subtitle') }}
        </p>
      </div>
    </div>

    <Card class="overflow-hidden p-0">
      <div v-if="loading && rows.length === 0" class="px-4 py-6 text-center text-[12px] text-muted-foreground">
        {{ t('connect.ide.loading') }}
      </div>
      <div v-else-if="rows.length === 0" class="px-4 py-6 text-center text-[12px] text-muted-foreground">
        {{ t('connect.ide.empty') }}
      </div>
      <template v-else>
        <div
          v-for="(row, i) in rows"
          :key="row.id"
          class="flex items-center gap-3 px-4 py-3"
          :class="i < rows.length - 1 && 'border-b'"
        >
          <IdeDot :adapter="row.id" size="lg" />
          <div class="min-w-0 flex-1">
            <div class="text-[13px] font-semibold">{{ row.label }}</div>
            <div v-if="row.configPath" class="truncate font-mono text-[10px] text-muted-foreground">
              {{ row.configPath }}
            </div>
            <div v-else class="text-[10px] text-muted-foreground">{{ t('connect.ide.config_missing') }}</div>
          </div>

          <Tooltip>
            <TooltipTrigger as-child>
              <div class="flex items-center gap-1.5">
                <span class="text-[11px] font-medium text-muted-foreground">{{ t('connect.ide.col.mcp') }}</span>
                <Switch
                  :model-value="row.mcpInstalled"
                  :disabled="busy[row.id]"
                  @update:model-value="(v: boolean) => toggleMcp(row, v)"
                />
              </div>
            </TooltipTrigger>
            <TooltipContent side="top" class="max-w-xs text-[11px]">
              {{ t('connect.ide.tooltip.mcp') }}
            </TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger as-child>
              <div class="flex items-center gap-1.5">
                <span class="text-[11px] font-medium text-muted-foreground">{{ t('connect.ide.col.skill') }}</span>
                <Switch
                  :model-value="row.skillInstalled"
                  :disabled="busy[row.id + ':skill']"
                  @update:model-value="(v: boolean) => toggleSkill(row, v)"
                />
              </div>
            </TooltipTrigger>
            <TooltipContent side="top" class="max-w-xs text-[11px]">
              {{ t('connect.ide.tooltip.skill') }}
            </TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger as-child>
              <div
                class="flex items-center gap-1.5"
                :class="!row.hookSupported && 'opacity-40'"
              >
                <span class="text-[11px] font-medium text-muted-foreground">{{ t('connect.ide.col.hook') }}</span>
                <Switch
                  :model-value="row.hookInstalled"
                  :disabled="!row.hookSupported || busy[row.id + ':hook']"
                  @update:model-value="(v: boolean) => toggleHook(row, v)"
                />
              </div>
            </TooltipTrigger>
            <TooltipContent side="top" class="max-w-xs text-[11px]">
              <span v-if="row.hookSupported">
                {{ t('connect.ide.tooltip.hook_supported') }}
              </span>
              <span v-else>{{ t('connect.ide.tooltip.hook_unsupported') }}</span>
            </TooltipContent>
          </Tooltip>
        </div>
        <Separator />
        <div class="flex items-center justify-between px-4 py-2.5">
          <span class="text-[10px] italic text-muted-foreground">{{ t('connect.ide.hint.restart') }}</span>
          <Button variant="ghost" size="sm" class="h-7 gap-1 text-xs" :disabled="loading" @click="loadStatus">
            <RefreshCw :class="['size-3', loading && 'animate-spin']" />
            {{ t('connect.ide.action.recheck') }}
          </Button>
        </div>
      </template>
    </Card>
  </section>
</template>
