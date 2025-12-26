<script setup lang="ts">
defineProps<{
  size?: 'sm' | 'md' | 'lg'
}>()
</script>

<template>
  <div class="flex items-center justify-center">
    <div
      class="border-2 border-gray-200 border-t-primary rounded-full animate-spin"
      :class="{
        'w-4 h-4': size === 'sm',
        'w-6 h-6': size === 'md' || !size,
        'w-8 h-8': size === 'lg',
      }"
    ></div>
  </div>
</template>
