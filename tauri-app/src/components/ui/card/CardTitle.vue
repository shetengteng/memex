<script setup lang="ts">
defineProps<{ class?: string }>()
</script>

<template>
  <h3
    :class="[
      'text-xs font-semibold uppercase tracking-wider text-muted-foreground',
      $props.class,
    ]"
  >
    <slot />
  </h3>
</template>
