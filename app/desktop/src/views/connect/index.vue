<script setup lang="ts">
import AdaptersCard from './components/AdaptersCard.vue'
import IdeIntegrationsCard from './components/IdeIntegrationsCard.vue'
import McpActivityCard from './components/McpActivityCard.vue'
</script>

<template>
  <div class="@container/main flex flex-1 flex-col min-h-0 overflow-y-auto">
    <div class="mx-auto w-full max-w-5xl space-y-6 px-4 py-4 lg:px-6 lg:py-6">
      <AdaptersCard />
      <IdeIntegrationsCard />
      <McpActivityCard />
    </div>
  </div>
</template>
