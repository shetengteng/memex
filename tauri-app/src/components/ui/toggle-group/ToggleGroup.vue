<script setup lang="ts">
import { cn } from '@/lib/utils'
import {
  ToggleGroupRoot,
  type ToggleGroupRootEmits,
  type ToggleGroupRootProps,
  useForwardPropsEmits,
} from 'reka-ui'
import { computed, provide, type HTMLAttributes } from 'vue'

const props = defineProps<ToggleGroupRootProps & {
  class?: HTMLAttributes['class']
  size?: 'default' | 'sm' | 'lg' | 'xl'
  variant?: 'default' | 'outline'
}>()
const emits = defineEmits<ToggleGroupRootEmits>()

const delegated = computed(() => {
  const { class: _c, size: _s, variant: _v, ...rest } = props
  return rest
})
const forwarded = useForwardPropsEmits(delegated, emits)

provide('toggleGroupContext', {
  size: props.size ?? 'default',
  variant: props.variant ?? 'default',
})
</script>

<template>
  <ToggleGroupRoot
    v-bind="forwarded"
    :class="cn('inline-flex items-center justify-center gap-1', props.class)"
  >
    <slot />
  </ToggleGroupRoot>
</template>
