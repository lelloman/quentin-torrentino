<script setup lang="ts">
import type { Ticket } from '../../api/types'
import TicketCard from './TicketCard.vue'
import LoadingSpinner from '../common/LoadingSpinner.vue'

defineProps<{
  tickets: Ticket[]
  loading?: boolean
  hasMore?: boolean
}>()

const emit = defineEmits<{
  loadMore: []
}>()
</script>

<template>
  <div class="space-y-3">
    <TicketCard
      v-for="ticket in tickets"
      :key="ticket.id"
      :ticket="ticket"
    />

    <div v-if="tickets.length === 0 && !loading" class="text-center py-12 text-gray-500">
      No tickets found
    </div>

    <div v-if="loading" class="flex justify-center py-6">
      <LoadingSpinner />
    </div>

    <div v-if="hasMore && !loading" class="flex justify-center pt-4">
      <button @click="emit('loadMore')" class="btn-secondary">
        Load More
      </button>
    </div>
  </div>
</template>
