<script setup lang="ts">
import type { TorrentState } from '../../api/types'

defineProps<{
  modelValue: TorrentState | undefined
}>()

defineEmits<{
  'update:modelValue': [value: TorrentState | undefined]
}>()

const states: { value: TorrentState | undefined; label: string }[] = [
  { value: undefined, label: 'All' },
  { value: 'downloading', label: 'Downloading' },
  { value: 'seeding', label: 'Seeding' },
  { value: 'paused', label: 'Paused' },
  { value: 'checking', label: 'Checking' },
  { value: 'queued', label: 'Queued' },
  { value: 'stalled', label: 'Stalled' },
  { value: 'error', label: 'Error' },
]
</script>

<template>
  <div class="flex gap-1 flex-wrap">
    <button
      v-for="state in states"
      :key="state.value ?? 'all'"
      @click="$emit('update:modelValue', state.value)"
      :class="[
        'px-3 py-1 text-sm rounded-full transition-colors',
        modelValue === state.value
          ? 'bg-blue-600 text-white'
          : 'bg-gray-100 text-gray-700 hover:bg-gray-200',
      ]"
    >
      {{ state.label }}
    </button>
  </div>
</template>
