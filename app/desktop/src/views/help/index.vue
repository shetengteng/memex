<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  Rocket,
  Plug,
  Wrench,
  Wand2,
  Brain,
  ShieldCheck,
  Stethoscope,
} from 'lucide-vue-next'
import { useI18n } from '@/i18n'
import HelpMarkdown from './components/HelpMarkdown.vue'

// 把 7 段说明书 .md 在 build 时打包进 bundle，运行期不需要 fs 访问，
// 也不会触发任何网络请求；?raw 让 Vite 把每个文件原样导出为字符串。
const rawModules = import.meta.glob('./contents/*.md', {
  eager: true,
  query: '?raw',
  import: 'default',
}) as Record<string, string>

function loadMarkdown(slug: string): string {
  const path = `./contents/${slug}.md`
  return rawModules[path] ?? ''
}

const route = useRoute()
const router = useRouter()
const { t } = useI18n()

const TABS = [
  { value: 'quickstart', icon: Rocket, labelKey: 'help.tab.quickstart' },
  { value: 'integrations', icon: Plug, labelKey: 'help.tab.integrations' },
  { value: 'mcp', icon: Wrench, labelKey: 'help.tab.mcp' },
  { value: 'skills', icon: Wand2, labelKey: 'help.tab.skills' },
  { value: 'context', icon: Brain, labelKey: 'help.tab.context' },
  { value: 'privacy', icon: ShieldCheck, labelKey: 'help.tab.privacy' },
  { value: 'troubleshooting', icon: Stethoscope, labelKey: 'help.tab.troubleshooting' },
] as const

const ALLOWED = new Set(TABS.map((t) => t.value))

function pickTab(raw: unknown): string {
  if (typeof raw === 'string' && ALLOWED.has(raw as typeof TABS[number]['value'])) return raw
  return 'quickstart'
}

const activeTab = ref<string>(pickTab(route.query.tab))

watch(
  () => route.query.tab,
  (v) => {
    activeTab.value = pickTab(v)
  },
)

watch(activeTab, (v) => {
  if (route.query.tab === v) return
  router.replace({ path: '/help', query: { ...route.query, tab: v } })
})

const contents = computed(() =>
  TABS.map((tab) => ({
    value: tab.value,
    source: loadMarkdown(tab.value),
  })),
)
</script>

<template>
  <div class="@container/main flex flex-1 flex-col min-h-0 overflow-y-auto">
    <div class="mx-auto w-full max-w-4xl px-4 py-4 lg:px-6 lg:py-6">
      <Tabs v-model="activeTab" class="w-full">
        <Teleport to="#memex-header-center" defer>
          <TabsList class="h-8">
            <TabsTrigger
              v-for="tab in TABS"
              :key="tab.value"
              :value="tab.value"
              class="gap-1.5 text-[12px]"
            >
              <component :is="tab.icon" class="size-3.5" />
              {{ t(tab.labelKey) }}
            </TabsTrigger>
          </TabsList>
        </Teleport>

        <TabsContent
          v-for="item in contents"
          :key="item.value"
          :value="item.value"
          class="mt-2 focus-visible:outline-none"
        >
          <HelpMarkdown :source="item.source" />
        </TabsContent>
      </Tabs>
    </div>
  </div>
</template>
