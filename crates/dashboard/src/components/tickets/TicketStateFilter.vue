<script setup lang="ts">
import type { TicketStateType } from '../../api/types'

defineProps<{
  modelValue: TicketStateType | undefined
}>()

const emit = defineEmits<{
  'update:modelValue': [value: TicketStateType | undefined]
}>()

const states: { value: TicketStateType | undefined; label: string }[] = [
  { value: undefined, label: 'All' },
  { value: 'pending', label: 'Pending' },
  { value: 'cancelled', label: 'Cancelled' },
  { value: 'completed', label: 'Completed' },
  { value: 'failed', label: 'Failed' },
]
</script>

<template>
  <select
    :value="modelValue ?? ''"
    @change="emit('update:modelValue', ($event.target as HTMLSelectElement).value as TicketStateType || undefined)"
    class="input"
  >
    <option
      v-for="state in states"
      :key="state.value ?? 'all'"
      :value="state.value ?? ''"
    >
      {{ state.label }}
    </option>
  </select>
</template>
