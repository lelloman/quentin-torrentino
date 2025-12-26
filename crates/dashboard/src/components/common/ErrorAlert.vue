<script setup lang="ts">
defineProps<{
  message: string
}>()

const emit = defineEmits<{
  dismiss: []
}>()
</script>

<template>
  <div class="bg-red-50 border border-red-200 rounded-md p-4 flex items-start gap-3">
    <span class="i-carbon-warning text-danger text-lg flex-shrink-0 mt-0.5"></span>
    <div class="flex-1">
      <p class="text-sm text-red-800">{{ message }}</p>
    </div>
    <button
      @click="emit('dismiss')"
      class="text-red-400 hover:text-red-600 transition-colors"
    >
      <span class="i-carbon-close"></span>
    </button>
  </div>
</template>
