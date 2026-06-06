<script setup lang="ts">
import { ref } from 'vue'
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
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
import { Monitor, Moon, Shield, Sun } from '@lucide/vue'
import { themeMode } from '@/composables/useTheme'

const privacy = ref({ autoRedact: false, privateFromMcp: false })

const notifications = ref([
  { label: '生成新的周报', sub: '每周日 22:00', on: true },
  { label: '反思待处理超过 24 小时', sub: '避免提示词放置太久', on: true },
  { label: '采集源同步失败', sub: '当无法解析某个会话时通知', on: true },
])
</script>

<template>
  <div class="space-y-4">
    <Card>
      <CardHeader>
        <CardDescription>外观</CardDescription>
        <CardTitle class="text-base">主题与语言</CardTitle>
      </CardHeader>
      <CardContent class="space-y-5">
        <div class="space-y-2">
          <Label class="text-xs text-muted-foreground">主题</Label>
          <div class="grid grid-cols-3 gap-3">
            <button
              type="button"
              class="group flex flex-col items-center gap-2 rounded-lg border bg-card p-3 transition-colors hover:border-primary/40 hover:bg-accent"
              :class="themeMode === 'light' && 'border-primary bg-accent ring-1 ring-primary/40'"
              @click="themeMode = 'light'"
            >
              <div
                class="flex size-12 items-center justify-center rounded-md bg-white text-amber-500 shadow-sm ring-1 ring-zinc-200"
              >
                <Sun class="size-5" />
              </div>
              <div class="text-center">
                <div class="text-[12px] font-medium">浅色</div>
                <div class="text-[10px] text-muted-foreground">Light</div>
              </div>
            </button>
            <button
              type="button"
              class="group flex flex-col items-center gap-2 rounded-lg border bg-card p-3 transition-colors hover:border-primary/40 hover:bg-accent"
              :class="themeMode === 'dark' && 'border-primary bg-accent ring-1 ring-primary/40'"
              @click="themeMode = 'dark'"
            >
              <div
                class="flex size-12 items-center justify-center rounded-md bg-zinc-900 text-zinc-100 shadow-sm ring-1 ring-zinc-800"
              >
                <Moon class="size-5" />
              </div>
              <div class="text-center">
                <div class="text-[12px] font-medium">深色</div>
                <div class="text-[10px] text-muted-foreground">Dark</div>
              </div>
            </button>
            <button
              type="button"
              class="group flex flex-col items-center gap-2 rounded-lg border bg-card p-3 transition-colors hover:border-primary/40 hover:bg-accent"
              :class="themeMode === 'system' && 'border-primary bg-accent ring-1 ring-primary/40'"
              @click="themeMode = 'system'"
            >
              <div
                class="flex size-12 items-center justify-center rounded-md bg-gradient-to-br from-white to-zinc-900 text-zinc-700 shadow-sm ring-1 ring-zinc-300"
              >
                <Monitor class="size-5" />
              </div>
              <div class="text-center">
                <div class="text-[12px] font-medium">跟随系统</div>
                <div class="text-[10px] text-muted-foreground">Auto</div>
              </div>
            </button>
          </div>
        </div>
        <Separator />
        <div class="flex items-center justify-between">
          <Label class="text-sm">界面语言</Label>
          <Select default-value="zh">
            <SelectTrigger class="w-40"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="zh">简体中文</SelectItem>
              <SelectItem value="en">English</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </CardContent>
    </Card>

    <Card>
      <CardHeader>
        <CardDescription>提醒</CardDescription>
        <CardTitle class="text-base">在以下情况通知我</CardTitle>
      </CardHeader>
      <CardContent class="space-y-4">
        <div
          v-for="(item, i) in notifications"
          :key="i"
          class="flex items-center justify-between"
        >
          <div>
            <Label class="text-sm">{{ item.label }}</Label>
            <p class="text-xs text-muted-foreground">{{ item.sub }}</p>
          </div>
          <Switch v-model="item.on" />
        </div>
      </CardContent>
    </Card>

    <Card>
      <CardHeader>
        <CardDescription>隐私</CardDescription>
        <CardTitle class="text-base">数据保护</CardTitle>
        <CardAction>
          <Shield class="size-4 text-muted-foreground" />
        </CardAction>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="flex items-center justify-between">
          <div>
            <Label class="text-sm">自动脱敏</Label>
            <p class="text-xs text-muted-foreground">
              入库前自动移除 API Key、密码等敏感信息
            </p>
          </div>
          <Switch v-model="privacy.autoRedact" />
        </div>
        <div class="flex items-center justify-between">
          <div>
            <Label class="text-sm">对 MCP 隐藏私有会话</Label>
            <p class="text-xs text-muted-foreground">
              标记为「私有」的会话不会通过 MCP 暴露给 IDE
            </p>
          </div>
          <Switch v-model="privacy.privateFromMcp" />
        </div>
      </CardContent>
    </Card>
  </div>
</template>
