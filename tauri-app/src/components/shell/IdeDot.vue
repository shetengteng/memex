<script setup lang="ts">
defineProps<{
  adapter: string
  size?: 'sm' | 'md' | 'lg'
}>()
</script>

<template>
  <span
    :class="[
      'ide-dot',
      `ide-dot-${adapter.replace('_', '-').replace('claude-code', 'claude')}`,
      size === 'lg' && 'size-3',
      size === 'md' && 'size-2.5',
    ]"
  />
</template>
