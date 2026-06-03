<script setup lang="ts">
defineProps<{ class?: string }>()
</script>

<template>
  <div :class="['flex flex-col px-4 pb-3 pt-1', $props.class]">
    <slot />
  </div>
</template>
