<script setup lang="ts">
import { computed } from 'vue'

type Variant = 'default' | 'secondary' | 'outline' | 'destructive'

const props = withDefaults(
  defineProps<{ variant?: Variant }>(),
  { variant: 'default' },
)

const cls = computed(() => {
  const base = 'inline-flex items-center gap-1 rounded-md border px-2 py-0.5 text-xs font-medium leading-none whitespace-nowrap'
  switch (props.variant) {
    case 'secondary':
      return `${base} border-transparent bg-secondary text-secondary-foreground`
    case 'outline':
      return `${base} border-border bg-transparent text-foreground`
    case 'destructive':
      return `${base} border-transparent bg-destructive/10 text-destructive`
    case 'default':
    default:
      return `${base} border-transparent bg-primary/10 text-primary`
  }
})
</script>

<template>
  <span :class="cls">
    <slot />
  </span>
</template>
