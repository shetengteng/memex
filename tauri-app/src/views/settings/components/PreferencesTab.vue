<script setup lang="ts">
import { onMounted, reactive, ref, watch } from 'vue'
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
import { Monitor, Moon, Shield, Sun } from 'lucide-vue-next'
import { themeMode } from '@/composables/useTheme'
import { useMemex } from '@/composables/useMemex'

const memex = useMemex()
const ready = ref(false)

const lang = ref<'zh' | 'en'>('zh')

const privacy = reactive({ autoRedact: false, privateFromMcp: false })

interface NotificationItem {
  key: string
  label: string
  sub: string
  on: boolean
}

const notifications = reactive<NotificationItem[]>([
  { key: 'weekly_report', label: '生成新的周报', sub: '每周日 22:00', on: true },
  { key: 'reflect_pending', label: '反思待处理超过 24 小时', sub: '避免提示词放置太久', on: true },
  { key: 'ingest_failed', label: '采集源同步失败', sub: '当无法解析某个会话时通知', on: true },
])

async function readBool(key: string, fallback: boolean): Promise<boolean> {
  try {
    const v = await memex.getConfig(key)
    if (v == null) return fallback
    return v === 'true'
  } catch {
    return fallback
  }
}

async function readString(key: string, fallback: string): Promise<string> {
  try {
    const v = await memex.getConfig(key)
    return v ?? fallback
  } catch {
    return fallback
  }
}

onMounted(async () => {
  const [autoR, privM, langV, notifVals] = await Promise.all([
    readBool('pref.privacy.auto_redact', false),
    readBool('pref.privacy.private_from_mcp', false),
    readString('pref.language', 'zh'),
    Promise.all(notifications.map((n) => readBool(`pref.notify.${n.key}`, n.on))),
  ])
  privacy.autoRedact = autoR
  privacy.privateFromMcp = privM
  lang.value = (langV === 'en' ? 'en' : 'zh') as 'zh' | 'en'
  notifications.forEach((n, i) => (n.on = notifVals[i]))
  ready.value = true
})

watch(
  () => privacy.autoRedact,
  (v) => ready.value && memex.setConfig('pref.privacy.auto_redact', String(v)).catch(() => {}),
)
watch(
  () => privacy.privateFromMcp,
  (v) => ready.value && memex.setConfig('pref.privacy.private_from_mcp', String(v)).catch(() => {}),
)
watch(
  lang,
  (v) => ready.value && memex.setConfig('pref.language', v).catch(() => {}),
)
notifications.forEach((n) => {
  watch(
    () => n.on,
    (v) => ready.value && memex.setConfig(`pref.notify.${n.key}`, String(v)).catch(() => {}),
  )
})
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
          <Select v-model="lang">
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
