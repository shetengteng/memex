<script setup lang="ts">
import { computed } from 'vue'
import { ADAPTER_MAP, type Adapter } from '@/mock/data'
import IdeDot from './IdeDot.vue'

const props = withDefaults(
  defineProps<{
    adapter: Adapter
    label?: string
    fixedWidth?: boolean
  }>(),
  { fixedWidth: false },
)

const display = computed(() => props.label ?? ADAPTER_MAP[props.adapter].label)
</script>

<template>
  <span :class="['ide-chip', fixedWidth && 'min-w-[6rem] justify-start']">
    <IdeDot :adapter="adapter" />
    {{ display }}
  </span>
</template>
