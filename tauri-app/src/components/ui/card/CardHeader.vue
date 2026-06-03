<script setup lang="ts">
defineProps<{ class?: string }>()
</script>

<template>
  <div :class="['flex flex-col gap-1 px-4 pb-2 pt-3', $props.class]">
    <slot />
  </div>
</template>
