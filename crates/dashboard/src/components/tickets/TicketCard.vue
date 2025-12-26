<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import type { Ticket } from '../../api/types'
import Badge from '../common/Badge.vue'

const props = defineProps<{
  ticket: Ticket
}>()

const stateVariant = computed(() => {
  switch (props.ticket.state.type) {
    case 'pending':
      return 'info'
    case 'completed':
      return 'success'
    case 'cancelled':
      return 'warning'
    case 'failed':
      return 'danger'
    default:
      return 'default'
  }
})

const formattedDate = computed(() => {
  return new Date(props.ticket.created_at).toLocaleString()
})
</script>

<template>
  <RouterLink
    :to="`/tickets/${ticket.id}`"
    class="card block hover:shadow-md transition-shadow"
  >
    <div class="flex items-start justify-between gap-4">
      <div class="flex-1 min-w-0">
        <p class="font-mono text-sm text-gray-500 truncate">{{ ticket.id }}</p>
        <p class="mt-1 text-gray-900">{{ ticket.query_context.description }}</p>
        <div class="mt-2 flex flex-wrap gap-1">
          <span
            v-for="tag in ticket.query_context.tags"
            :key="tag"
            class="inline-block bg-gray-100 text-gray-600 text-xs px-2 py-0.5 rounded"
          >
            {{ tag }}
          </span>
        </div>
      </div>
      <div class="flex flex-col items-end gap-2">
        <Badge :variant="stateVariant">{{ ticket.state.type }}</Badge>
        <span class="text-xs text-gray-500">{{ formattedDate }}</span>
      </div>
    </div>
  </RouterLink>
</template>
