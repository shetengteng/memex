<script setup lang="ts">
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
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import { Switch } from '@/components/ui/switch'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Download, RefreshCw, Trash2, Upload } from '@lucide/vue'
</script>

<template>
  <div class="space-y-4">
    <Card>
      <CardHeader>
        <CardDescription>存储</CardDescription>
        <CardTitle class="text-base">本地 SQLite 数据库</CardTitle>
        <CardAction>
          <Badge variant="outline">184 MB</Badge>
        </CardAction>
      </CardHeader>
      <CardContent class="space-y-3 text-sm">
        <div class="flex items-center justify-between">
          <span>数据库路径</span>
          <code class="text-xs text-muted-foreground">
            ~/Library/Application Support/memex/memex.db
          </code>
        </div>
        <div class="flex items-center justify-between">
          <span>会话数</span>
          <span class="font-medium tabular-nums">6,521</span>
        </div>
        <div class="flex items-center justify-between">
          <span>消息数</span>
          <span class="font-medium tabular-nums">184,209</span>
        </div>
        <div class="flex items-center justify-between">
          <span>摘要数</span>
          <span class="font-medium tabular-nums">6,389</span>
        </div>
      </CardContent>
      <Separator />
      <CardFooter class="gap-2">
        <Button size="sm" variant="outline">
          <Download class="mr-1.5 size-3.5" />
          导出数据库
        </Button>
        <Button size="sm" variant="outline">
          <Upload class="mr-1.5 size-3.5" />
          导入
        </Button>
      </CardFooter>
    </Card>

    <Card>
      <CardHeader>
        <CardDescription>备份</CardDescription>
        <CardTitle class="text-base">自动快照</CardTitle>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="flex items-center justify-between">
          <div>
            <Label class="text-sm">每晚自动快照</Label>
            <p class="text-xs text-muted-foreground">
              保存于 ~/Library/Application Support/memex/backups
            </p>
          </div>
          <Switch :model-value="true" />
        </div>
        <div class="flex items-center justify-between">
          <div>
            <Label class="text-sm">保留最近</Label>
            <p class="text-xs text-muted-foreground">更老的快照会自动清理</p>
          </div>
          <Select default-value="7">
            <SelectTrigger class="w-32"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="3">3 天</SelectItem>
              <SelectItem value="7">7 天</SelectItem>
              <SelectItem value="30">30 天</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </CardContent>
    </Card>

    <Card>
      <CardHeader>
        <CardDescription class="text-destructive">危险区</CardDescription>
        <CardTitle class="text-base">重置数据</CardTitle>
      </CardHeader>
      <CardContent class="space-y-3 text-sm">
        <div class="flex items-center justify-between gap-3">
          <div class="flex-1">
            <Label class="text-sm">仅重建索引</Label>
            <p class="text-xs text-muted-foreground">
              清空 FTS5 索引并按现有数据库重建，不会删除会话
            </p>
          </div>
          <Button
            size="sm"
            variant="outline"
            class="gap-1.5 border-amber-500/50 text-amber-700 hover:bg-amber-500/10"
          >
            <RefreshCw class="size-3.5" />
            重建索引
          </Button>
        </div>
        <Separator />
        <div class="flex items-center justify-between gap-3">
          <div class="flex-1">
            <Label class="text-sm text-destructive">清空全部数据</Label>
            <p class="text-xs text-muted-foreground">
              删除所有会话、摘要和索引，此操作不可恢复
            </p>
          </div>
          <Button size="sm" variant="destructive" class="gap-1.5">
            <Trash2 class="size-3.5" />
            清空全部
          </Button>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
