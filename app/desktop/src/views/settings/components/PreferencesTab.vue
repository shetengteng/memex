<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Shield } from 'lucide-vue-next'
import { themeMode, type ThemeMode } from '@/composables/useTheme'
import { surfaceMode, type SurfaceMode } from '@/composables/useSurface'
import { useMemex } from '@/composables/useMemex'
import { useI18n, setLocale, type Locale } from '@/i18n'

const memex = useMemex()
const { t, locale } = useI18n()
const ready = ref(false)

const lang = ref<Locale>(locale.value)

const privacy = reactive({ autoRedact: false, privateFromMcp: false })

interface NotificationItem {
  key: string
  on: boolean
}

const notifications = reactive<NotificationItem[]>([
  { key: 'weekly_report', on: true },
  { key: 'reflect_pending', on: true },
  { key: 'ingest_failed', on: true },
])

interface NotificationItemView extends NotificationItem {
  label: string
  sub: string
}

const NOTIF_LABEL_KEY: Record<string, { label: string; sub: string }> = {
  weekly_report: { label: 'settings.prefs.notify.weekly.label', sub: 'settings.prefs.notify.weekly.sub' },
  reflect_pending: { label: 'settings.prefs.notify.reflect.label', sub: 'settings.prefs.notify.reflect.sub' },
  ingest_failed: { label: 'settings.prefs.notify.ingest.label', sub: 'settings.prefs.notify.ingest.sub' },
}

const notificationsView = computed<NotificationItemView[]>(() =>
  notifications.map((n) => {
    const keys = NOTIF_LABEL_KEY[n.key]
    return { ...n, label: t(keys.label), sub: t(keys.sub) }
  }),
)

async function readBool(key: string, fallback: boolean): Promise<boolean> {
  try {
    const v = await memex.getConfig(key)
    if (v == null) return fallback
    return v === 'true'
  } catch {
    return fallback
  }
}

onMounted(async () => {
  const [autoR, privM, notifVals] = await Promise.all([
    readBool('pref.privacy.auto_redact', false),
    readBool('pref.privacy.private_from_mcp', false),
    Promise.all(notifications.map((n) => readBool(`pref.notify.${n.key}`, n.on))),
  ])
  privacy.autoRedact = autoR
  privacy.privateFromMcp = privM
  lang.value = locale.value
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
// 切换 UI 语言：调用 setLocale 同时落 localStorage + 后端 kv，整个 app 立刻重渲染。
// 同时 watch locale，外部（其它窗口 / 同步钩子）改变时把本页 select 也带过来，
// 避免出现 select 显示中文但其他文案已经切到英文的不一致。
watch(
  lang,
  (v) => {
    if (!ready.value) return
    if (v === locale.value) return
    void setLocale(v)
  },
)
watch(
  locale,
  (v) => {
    if (v !== lang.value) lang.value = v
  },
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
        <CardDescription>{{ t('settings.prefs.appearance') }}</CardDescription>
        <CardTitle class="text-base">{{ t('settings.prefs.theme_lang_title') }}</CardTitle>
      </CardHeader>
      <CardContent class="space-y-3">
        <div class="flex items-center justify-between">
          <Label class="text-sm">{{ t('settings.prefs.theme') }}</Label>
          <Select :model-value="themeMode" @update:model-value="(v) => (themeMode = String(v) as ThemeMode)">
            <SelectTrigger class="h-9 w-40"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="light">{{ t('settings.prefs.theme_light') }}</SelectItem>
              <SelectItem value="dark">{{ t('settings.prefs.theme_dark') }}</SelectItem>
              <SelectItem value="system">{{ t('settings.prefs.theme_system') }}</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div class="flex items-start justify-between">
          <div>
            <Label class="text-sm">{{ t('settings.prefs.surface') }}</Label>
            <p class="text-xs text-muted-foreground">{{ t('settings.prefs.surface_hint') }}</p>
          </div>
          <Select :model-value="surfaceMode" @update:model-value="(v) => (surfaceMode = String(v) as SurfaceMode)">
            <SelectTrigger class="h-9 w-40"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="solid">{{ t('settings.prefs.surface_solid') }}</SelectItem>
              <SelectItem value="glass">{{ t('settings.prefs.surface_glass') }}</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div class="flex items-center justify-between">
          <Label class="text-sm">{{ t('settings.prefs.language') }}</Label>
          <Select v-model="lang">
            <SelectTrigger class="h-9 w-40"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="zh">{{ t('settings.prefs.lang_zh') }}</SelectItem>
              <SelectItem value="en">{{ t('settings.prefs.lang_en') }}</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </CardContent>
    </Card>

    <Card>
      <CardHeader>
        <CardDescription>{{ t('settings.prefs.notify_section') }}</CardDescription>
        <CardTitle class="text-base">{{ t('settings.prefs.notify_title') }}</CardTitle>
      </CardHeader>
      <CardContent class="space-y-4">
        <div
          v-for="(item, i) in notificationsView"
          :key="i"
          class="flex items-center justify-between"
        >
          <div>
            <Label class="text-sm">{{ item.label }}</Label>
            <p class="text-xs text-muted-foreground">{{ item.sub }}</p>
          </div>
          <Switch v-model="notifications[i].on" />
        </div>
      </CardContent>
    </Card>

    <Card>
      <CardHeader>
        <CardDescription>{{ t('settings.prefs.privacy_section') }}</CardDescription>
        <CardTitle class="text-base">{{ t('settings.prefs.privacy_title') }}</CardTitle>
        <CardAction>
          <Shield class="size-4 text-muted-foreground" />
        </CardAction>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="flex items-center justify-between">
          <div>
            <Label class="text-sm">{{ t('settings.prefs.privacy.redact.label') }}</Label>
            <p class="text-xs text-muted-foreground">
              {{ t('settings.prefs.privacy.redact.sub') }}
            </p>
          </div>
          <Switch v-model="privacy.autoRedact" />
        </div>
        <div class="flex items-center justify-between">
          <div>
            <Label class="text-sm">{{ t('settings.prefs.privacy.mcp.label') }}</Label>
            <p class="text-xs text-muted-foreground">
              {{ t('settings.prefs.privacy.mcp.sub') }}
            </p>
          </div>
          <Switch v-model="privacy.privateFromMcp" />
        </div>
      </CardContent>
    </Card>
  </div>
</template>
