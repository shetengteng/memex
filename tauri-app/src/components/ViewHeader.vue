<script setup lang="ts">
import { inject } from 'vue'
import { ArrowLeft } from 'lucide-vue-next'

defineProps<{
  title: string
  showBack?: boolean
}>()

const back = inject<() => void>('back')!
</script>

<template>
  <div class="flex h-11 shrink-0 items-center gap-2 border-b border-border px-3">
    <button
      v-if="showBack"
      class="inline-flex h-7 w-7 items-center justify-center rounded-md transition-colors hover:bg-accent"
      @click="back"
    >
      <ArrowLeft class="h-4 w-4" />
    </button>
    <h1 class="text-sm font-semibold">{{ title }}</h1>
  </div>
</template>
