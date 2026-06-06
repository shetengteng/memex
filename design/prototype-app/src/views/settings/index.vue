<script setup lang="ts">
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Bell, Brain, Database, Stethoscope } from '@lucide/vue'
import LlmTab from './components/LlmTab.vue'
import PreferencesTab from './components/PreferencesTab.vue'
import DataTab from './components/DataTab.vue'
import SystemTab from './components/SystemTab.vue'
</script>

<template>
  <div class="@container/main flex flex-1 flex-col min-h-0 overflow-y-auto gap-2">
    <div class="flex flex-col gap-4 py-4 md:gap-6 md:py-6">
      <div class="mx-auto w-full max-w-4xl px-4 lg:px-6">
        <Tabs default-value="llm" class="w-full">
          <Teleport to="#memex-header-center" defer>
            <TabsList class="h-8">
              <TabsTrigger value="llm" class="gap-1.5 text-[12px]">
                <Brain class="size-3.5" />
                LLM
              </TabsTrigger>
              <TabsTrigger value="prefs" class="gap-1.5 text-[12px]">
                <Bell class="size-3.5" />
                偏好
              </TabsTrigger>
              <TabsTrigger value="data" class="gap-1.5 text-[12px]">
                <Database class="size-3.5" />
                数据
              </TabsTrigger>
              <TabsTrigger value="system" class="gap-1.5 text-[12px]">
                <Stethoscope class="size-3.5" />
                系统
              </TabsTrigger>
            </TabsList>
          </Teleport>

          <TabsContent value="llm" class="mt-4">
            <LlmTab />
          </TabsContent>
          <TabsContent value="prefs" class="mt-4">
            <PreferencesTab />
          </TabsContent>
          <TabsContent value="data" class="mt-4">
            <DataTab />
          </TabsContent>
          <TabsContent value="system" class="mt-4">
            <SystemTab />
          </TabsContent>
        </Tabs>
      </div>
    </div>
  </div>
</template>
