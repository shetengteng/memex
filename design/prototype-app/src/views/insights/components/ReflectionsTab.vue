<script setup lang="ts">
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
import { BrainCircuit, ChevronRight } from '@lucide/vue'
import { reflections } from '@/mock/data'

const fmtTime = (iso: string) =>
  new Date(iso).toLocaleString('zh-CN', { dateStyle: 'short', timeStyle: 'short' })
</script>

<template>
  <div class="mx-auto w-full max-w-5xl space-y-4 px-4 py-4 lg:px-6 lg:py-6">
    <Card class="p-5">
      <div class="mb-3 flex items-center gap-2">
        <BrainCircuit class="size-4 text-primary" />
        <h3 class="text-[14px] font-semibold">让 AI 反思一下</h3>
      </div>
      <div class="flex items-center gap-2">
        <span class="text-[12px] text-muted-foreground">时间范围</span>
        <Select default-value="7d">
          <SelectTrigger class="h-8 w-40">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="3d">近 3 天</SelectItem>
            <SelectItem value="7d">近 7 天</SelectItem>
            <SelectItem value="30d">近 30 天</SelectItem>
          </SelectContent>
        </Select>
        <Button size="sm" class="h-8 gap-1.5">
          <BrainCircuit class="size-3.5" />
          开始反思
        </Button>
        <span class="ml-2 text-[11px] italic text-muted-foreground">通常需要 30~60 秒</span>
      </div>
    </Card>

    <div class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
      历史反思
    </div>
    <Card class="overflow-hidden">
      <ul>
        <li v-for="(r, idx) in reflections" :key="r.id">
          <button
            class="group flex w-full items-center justify-between gap-3 px-4 py-3.5 text-left transition-colors hover:bg-accent/40"
            :class="idx < reflections.length - 1 && 'border-b'"
          >
            <div class="min-w-0 flex-1">
              <div class="mb-1 flex items-center gap-2">
                <span class="text-[14px] font-semibold">{{ r.weekSpan }}</span>
                <Badge variant="secondary">{{ r.sessions }} 个会话</Badge>
                <Badge
                  v-if="r.unread"
                  variant="outline"
                  class="border-amber-500/30 text-amber-700"
                >
                  未读
                </Badge>
              </div>
              <p class="line-clamp-1 text-[12px] text-muted-foreground">
                {{ r.summary }} · {{ fmtTime(r.date) }}
              </p>
            </div>
            <ChevronRight class="size-4 shrink-0 text-muted-foreground" />
          </button>
        </li>
      </ul>
    </Card>
  </div>
</template>
