<script setup lang="ts">
defineProps<{
  variant?: 'default' | 'success' | 'warning' | 'danger' | 'info'
}>()
</script>

<template>
  <span
    class="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium"
    :class="{
      'bg-gray-100 text-gray-800': variant === 'default' || !variant,
      'bg-green-100 text-green-800': variant === 'success',
      'bg-yellow-100 text-yellow-800': variant === 'warning',
      'bg-red-100 text-red-800': variant === 'danger',
      'bg-blue-100 text-blue-800': variant === 'info',
    }"
  >
    <slot />
  </span>
</template>
