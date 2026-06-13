<script setup lang="ts">
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Lightbulb, Newspaper, TrendingUp } from 'lucide-vue-next'
import ReportsTab from './components/ReportsTab.vue'
import ReflectionsTab from './components/ReflectionsTab.vue'
import TrendsTab from './components/TrendsTab.vue'
import { useI18n } from '@/i18n'

const { t } = useI18n()
</script>

<template>
  <div class="@container/main flex flex-1 flex-col min-h-0 overflow-hidden">
    <Tabs default-value="reports" class="flex flex-1 min-h-0 flex-col">
      <Teleport to="#memex-header-center" defer>
        <TabsList class="h-8">
          <TabsTrigger value="reports" class="gap-1.5 text-[12px]">
            <Newspaper class="size-3.5" />
            {{ t('insights.tabs.reports') }}
          </TabsTrigger>
          <TabsTrigger value="reflections" class="gap-1.5 text-[12px]">
            <Lightbulb class="size-3.5" />
            {{ t('insights.tabs.reflections') }}
          </TabsTrigger>
          <TabsTrigger value="trends" class="gap-1.5 text-[12px]">
            <TrendingUp class="size-3.5" />
            {{ t('insights.tabs.trends') }}
          </TabsTrigger>
        </TabsList>
      </Teleport>

      <TabsContent value="reports" class="flex-1 min-h-0 overflow-y-auto p-0">
        <ReportsTab />
      </TabsContent>

      <TabsContent value="reflections" class="flex-1 min-h-0 overflow-y-auto p-0">
        <ReflectionsTab />
      </TabsContent>

      <TabsContent value="trends" class="flex-1 min-h-0 overflow-y-auto p-0">
        <TrendsTab />
      </TabsContent>
    </Tabs>
  </div>
</template>
