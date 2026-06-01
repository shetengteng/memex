<script setup lang="ts">
import { cn } from '@/lib/utils'
import { ToggleGroupItem, type ToggleGroupItemProps, useForwardProps } from 'reka-ui'
import { computed, inject, type HTMLAttributes } from 'vue'

const props = defineProps<ToggleGroupItemProps & { class?: HTMLAttributes['class'] }>()

const ctx = inject<{ size: string; variant: string }>('toggleGroupContext', { size: 'default', variant: 'default' })

const delegated = computed(() => {
  const { class: _c, ...rest } = props
  return rest
})
const forwarded = useForwardProps(delegated)

const sizeClass = computed(() => {
  switch (ctx.size) {
    case 'sm': return 'h-8 min-w-8 px-2'
    case 'lg': return 'h-10 min-w-10 px-3'
    default: return 'h-9 min-w-9 px-2.5'
  }
})

const variantClass = computed(() => {
  return ctx.variant === 'outline'
    ? 'border border-input shadow-xs'
    : ''
})
</script>

<template>
  <ToggleGroupItem
    v-bind="forwarded"
    :class="cn(
      'inline-flex items-center justify-center gap-2 rounded-md text-sm font-medium transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50',
      'data-[state=on]:bg-primary/10 data-[state=on]:text-primary',
      sizeClass,
      variantClass,
      props.class,
    )"
  >
    <slot />
  </ToggleGroupItem>
</template>
