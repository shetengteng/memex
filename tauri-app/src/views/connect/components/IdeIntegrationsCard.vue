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
import type { IdeStatus, SkillStatus, HookStatus } from '@/types'

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

const IDE_LABEL: Record<string, string> = {
  cursor: 'Cursor',
  claude_code: 'Claude Code',
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

async function toggleMcp(row: IdeRow, next: boolean) {
  busy.value[row.id] = true
  try {
    const s = next ? await memex.ideInstall(row.id) : await memex.ideUninstall(row.id)
    row.mcpInstalled = s.installed
    toast.success(`${row.label} MCP 已${next ? '安装' : '卸载'}`)
  } catch (e) {
    toast.error(`${row.label} MCP 切换失败：${formatToggleError(e)}`)
  } finally {
    busy.value[row.id] = false
  }
}

async function toggleSkill(row: IdeRow, next: boolean) {
  busy.value[row.id + ':skill'] = true
  try {
    const s = next ? await memex.skillInstall(row.id) : await memex.skillUninstall(row.id)
    row.skillInstalled = s.installed
    toast.success(`${row.label} SKILL 已${next ? '安装' : '卸载'}`)
  } catch (e) {
    toast.error(`${row.label} SKILL 切换失败：${formatToggleError(e)}`)
  } finally {
    busy.value[row.id + ':skill'] = false
  }
}

async function toggleHook(row: IdeRow, next: boolean) {
  busy.value[row.id + ':hook'] = true
  try {
    const s = next ? await memex.hookInstall(row.id) : await memex.hookUninstall(row.id)
    row.hookInstalled = s.installed
    toast.success(`${row.label} Hook 已${next ? '安装' : '卸载'}`)
  } catch (e) {
    toast.error(`${row.label} Hook 切换失败：${formatToggleError(e)}`)
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
          <h2 class="text-[15px] font-semibold">IDE 集成</h2>
          <Badge variant="secondary" class="text-[10px]">
            {{ installedIdeCount }} / {{ rows.length }} 已接入
          </Badge>
        </div>
        <p class="mt-0.5 text-[11px] text-muted-foreground">
          一键把 Memex MCP / SKILL / 项目记忆注入到目标 IDE
        </p>
      </div>
    </div>

    <Card class="overflow-hidden p-0">
      <div v-if="loading && rows.length === 0" class="px-4 py-6 text-center text-[12px] text-muted-foreground">
        加载中…
      </div>
      <div v-else-if="rows.length === 0" class="px-4 py-6 text-center text-[12px] text-muted-foreground">
        未检测到可接入的 IDE
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
            <div v-else class="text-[10px] text-muted-foreground">未找到配置</div>
          </div>

          <Tooltip>
            <TooltipTrigger as-child>
              <div class="flex items-center gap-1.5">
                <span class="text-[11px] font-medium text-muted-foreground">MCP</span>
                <Switch
                  :model-value="row.mcpInstalled"
                  :disabled="busy[row.id]"
                  @update:model-value="(v: boolean) => toggleMcp(row, v)"
                />
              </div>
            </TooltipTrigger>
            <TooltipContent side="top" class="max-w-xs text-[11px]">
              MCP server — 让 AI 用 search_memory / get_session 等工具读取 Memex 数据
            </TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger as-child>
              <div class="flex items-center gap-1.5">
                <span class="text-[11px] font-medium text-muted-foreground">SKILL</span>
                <Switch
                  :model-value="row.skillInstalled"
                  :disabled="busy[row.id + ':skill']"
                  @update:model-value="(v: boolean) => toggleSkill(row, v)"
                />
              </div>
            </TooltipTrigger>
            <TooltipContent side="top" class="max-w-xs text-[11px]">
              SKILL.md — 把 Memex 用法写入 IDE 的 skills 目录，AI 自动学会怎么用
            </TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger as-child>
              <div
                class="flex items-center gap-1.5"
                :class="!row.hookSupported && 'opacity-40'"
              >
                <span class="text-[11px] font-medium text-muted-foreground">Hook 钩子</span>
                <Switch
                  :model-value="row.hookInstalled"
                  :disabled="!row.hookSupported || busy[row.id + ':hook']"
                  @update:model-value="(v: boolean) => toggleHook(row, v)"
                />
              </div>
            </TooltipTrigger>
            <TooltipContent side="top" class="max-w-xs text-[11px]">
              <span v-if="row.hookSupported">
                Hook 钩子：AI 会话启动时自动注入「项目工作记忆」（最近的叙述摘要 + 相关决策）
              </span>
              <span v-else>该 IDE 暂不支持自动注入项目记忆（仅 Claude Code Hook 支持）</span>
            </TooltipContent>
          </Tooltip>
        </div>
        <Separator />
        <div class="flex items-center justify-between px-4 py-2.5">
          <span class="text-[10px] italic text-muted-foreground">修改后请重启对应 IDE 生效</span>
          <Button variant="ghost" size="sm" class="h-7 gap-1 text-xs" :disabled="loading" @click="loadStatus">
            <RefreshCw :class="['size-3', loading && 'animate-spin']" />
            重新检测
          </Button>
        </div>
      </template>
    </Card>
  </section>
</template>
