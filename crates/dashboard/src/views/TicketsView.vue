<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { useTickets } from '../composables/useTickets'
import TicketList from '../components/tickets/TicketList.vue'
import TicketStateFilter from '../components/tickets/TicketStateFilter.vue'
import CreateTicketForm from '../components/tickets/CreateTicketForm.vue'
import ErrorAlert from '../components/common/ErrorAlert.vue'
import type { CreateTicketRequest } from '../api/types'

const {
  tickets,
  loading,
  error,
  stateFilter,
  hasMore,
  fetchTickets,
  createTicket,
  setStateFilter,
  clearError,
} = useTickets()

const showCreateForm = ref(false)

onMounted(() => {
  fetchTickets()
})

watch(stateFilter, () => {
  fetchTickets()
})

function handleFilterChange(value: typeof stateFilter.value) {
  setStateFilter(value)
}

function handleLoadMore() {
  fetchTickets(true)
}

async function handleCreateTicket(request: CreateTicketRequest) {
  const ticket = await createTicket(request)
  if (ticket) {
    showCreateForm.value = false
  }
}
</script>

<template>
  <div>
    <div class="flex items-center justify-between mb-6">
      <h1 class="text-2xl font-bold">Tickets</h1>
      <button
        v-if="!showCreateForm"
        @click="showCreateForm = true"
        class="btn-primary"
      >
        Create Ticket
      </button>
    </div>

    <ErrorAlert
      v-if="error"
      :message="error"
      @dismiss="clearError"
      class="mb-4"
    />

    <CreateTicketForm
      v-if="showCreateForm"
      @submit="handleCreateTicket"
      @cancel="showCreateForm = false"
      class="mb-6"
    />

    <div class="flex items-center gap-4 mb-4">
      <label class="text-sm text-gray-600">Filter by state:</label>
      <TicketStateFilter
        :model-value="stateFilter"
        @update:model-value="handleFilterChange"
      />
    </div>

    <TicketList
      :tickets="tickets"
      :loading="loading"
      :has-more="hasMore"
      @load-more="handleLoadMore"
    />
  </div>
</template>
