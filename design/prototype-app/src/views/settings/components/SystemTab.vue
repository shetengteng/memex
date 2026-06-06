<script setup lang="ts">
import { ref } from 'vue'
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import {
  CheckCircle2,
  Download,
  RefreshCw,
  Stethoscope,
  Terminal,
  Trash2,
} from '@lucide/vue'

const cliInstalled = ref(true)
const appVersion = '0.3.4'
</script>

<template>
  <div class="space-y-4">
    <Card>
      <CardHeader>
        <CardDescription>命令行</CardDescription>
        <CardTitle class="text-base">CLI 工具</CardTitle>
        <CardAction>
          <Badge :variant="cliInstalled ? 'default' : 'outline'" class="gap-1">
            <Terminal class="size-3" />
            {{ cliInstalled ? '已安装' : '未安装' }}
          </Badge>
        </CardAction>
      </CardHeader>
      <CardContent class="space-y-3">
        <div class="flex items-center justify-between text-sm">
          <span>安装路径</span>
          <code class="font-mono text-xs text-muted-foreground">/usr/local/bin/memex</code>
        </div>
        <div
          v-if="cliInstalled"
          class="flex items-center gap-1.5 rounded-md border border-emerald-500/30 bg-emerald-500/5 px-3 py-2 text-[12px] text-emerald-600"
        >
          <CheckCircle2 class="size-3.5 shrink-0" />
          <span>CLI 已安装且在 PATH 中可用</span>
        </div>
      </CardContent>
      <Separator />
      <CardFooter class="gap-2">
        <Button v-if="cliInstalled" size="sm" variant="outline" class="gap-1.5">
          <Trash2 class="size-3.5" />
          卸载 CLI
        </Button>
        <Button v-else size="sm" class="gap-1.5">
          <Download class="size-3.5" />
          安装到 PATH
        </Button>
      </CardFooter>
    </Card>

    <Card>
      <CardHeader>
        <CardDescription>诊断</CardDescription>
        <CardTitle class="text-base">Doctor 系统检查</CardTitle>
      </CardHeader>
      <CardContent class="space-y-3">
        <div class="rounded-md border bg-muted/30 p-4 text-[12px]">
          <div class="grid grid-cols-2 gap-3">
            <div class="flex items-center justify-between">
              <span class="text-muted-foreground">数据目录</span>
              <code class="font-mono text-[11px]">~/.memex</code>
            </div>
            <div class="flex items-center justify-between">
              <span class="text-muted-foreground">数据库</span>
              <span class="text-emerald-600">Schema v12</span>
            </div>
            <div class="flex items-center justify-between">
              <span class="text-muted-foreground">FTS5 索引</span>
              <span class="text-emerald-600">正常</span>
            </div>
            <div class="flex items-center justify-between">
              <span class="text-muted-foreground">Cursor 数据</span>
              <span class="text-emerald-600">2,148 composers</span>
            </div>
          </div>
        </div>
      </CardContent>
      <Separator />
      <CardFooter>
        <Button size="sm" variant="outline" class="gap-1.5">
          <Stethoscope class="size-3.5" />
          运行 Doctor
        </Button>
      </CardFooter>
    </Card>

    <Card>
      <CardHeader>
        <CardDescription>关于</CardDescription>
        <CardTitle class="text-base">Memex</CardTitle>
        <CardAction>
          <Badge variant="secondary" class="font-mono text-[11px]">v{{ appVersion }}</Badge>
        </CardAction>
      </CardHeader>
      <CardContent class="space-y-3">
        <div class="flex items-center justify-between text-sm">
          <span>当前版本</span>
          <div class="flex items-center gap-2">
            <span class="font-mono text-xs text-muted-foreground">v{{ appVersion }}</span>
            <Button size="sm" variant="outline" class="h-7 gap-1 text-xs">
              <RefreshCw class="size-3" />
              检查更新
            </Button>
          </div>
        </div>
        <div
          class="flex items-center gap-1.5 rounded-md border border-emerald-500/30 bg-emerald-500/5 px-3 py-2 text-[12px] text-emerald-600"
        >
          <CheckCircle2 class="size-3.5 shrink-0" />
          <span>已是最新版本</span>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
