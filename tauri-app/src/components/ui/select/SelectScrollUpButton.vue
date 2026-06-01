<script setup lang="ts">
import { cn } from '@/lib/utils'
import { SelectScrollUpButton, type SelectScrollUpButtonProps } from 'reka-ui'
import { ChevronUp } from 'lucide-vue-next'
import type { HTMLAttributes } from 'vue'

const props = defineProps<SelectScrollUpButtonProps & { class?: HTMLAttributes['class'] }>()
</script>

<template>
  <SelectScrollUpButton :class="cn('flex cursor-default items-center justify-center py-1', props.class)">
    <slot>
      <ChevronUp class="h-4 w-4" />
    </slot>
  </SelectScrollUpButton>
</template>
