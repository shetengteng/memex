<script setup lang="ts">
defineProps<{ class?: string }>()
</script>

<template>
  <div
    :class="[
      'rounded-lg border border-border bg-card text-card-foreground shadow-sm',
      $props.class,
    ]"
  >
    <slot />
  </div>
</template>
