<script setup lang="ts">
import { computed, ref } from 'vue'
import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import { Switch } from '@/components/ui/switch'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { Puzzle, RefreshCw } from '@lucide/vue'
import IdeDot from '@/components/shell/IdeDot.vue'
import { ideIntegrations } from '@/mock/data'

const ideRows = ref(ideIntegrations.map((i) => ({ ...i })))

const installedIdeCount = computed(
  () => ideRows.value.filter((r) => r.mcpInstalled && r.skillInstalled).length,
)
</script>

<template>
  <section>
    <div class="mb-3 flex items-end justify-between">
      <div>
        <div class="flex items-center gap-2">
          <Puzzle class="size-3.5" :style="{ color: 'var(--adapter-claude)' }" />
          <h2 class="text-[15px] font-semibold">IDE 集成</h2>
          <Badge variant="secondary" class="text-[10px]">
            {{ installedIdeCount }} / {{ ideRows.length }} 已接入
          </Badge>
        </div>
        <p class="mt-0.5 text-[11px] text-muted-foreground">
          一键把 Memex MCP / SKILL / 项目记忆注入到目标 IDE
        </p>
      </div>
    </div>

    <Card class="overflow-hidden p-0">
      <div
        v-for="(row, i) in ideRows"
        :key="row.id"
        class="flex items-center gap-3 px-4 py-3"
        :class="i < ideRows.length - 1 && 'border-b'"
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
              <Switch v-model="row.mcpInstalled" />
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
              <Switch v-model="row.skillInstalled" />
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
                v-model="row.hookInstalled"
                :disabled="!row.hookSupported"
              />
            </div>
          </TooltipTrigger>
          <TooltipContent side="top" class="max-w-xs text-[11px]">
            <span v-if="row.hookSupported">
              Hook 钩子：AI 会话启动时自动注入「项目工作记忆」（最近的 L3 摘要 + 相关决策）
            </span>
            <span v-else>该 IDE 暂不支持自动注入项目记忆（仅 Claude Code Hook 支持）</span>
          </TooltipContent>
        </Tooltip>
      </div>
      <Separator />
      <div class="flex items-center justify-between px-4 py-2.5">
        <span class="text-[10px] italic text-muted-foreground">修改后请重启对应 IDE 生效</span>
        <Button variant="ghost" size="sm" class="h-7 gap-1 text-xs">
          <RefreshCw class="size-3" />
          重新检测
        </Button>
      </div>
    </Card>
  </section>
</template>
