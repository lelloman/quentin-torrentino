<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue'
import { useTickets } from '../composables/useTickets'
import { useGlobalWebSocket, type WsMessage } from '../composables/useWebSocket'
import TicketList from '../components/tickets/TicketList.vue'
import TicketStateFilter from '../components/tickets/TicketStateFilter.vue'
import CreateTicketForm from '../components/tickets/CreateTicketForm.vue'
import MusicTicketWizard from '../components/tickets/MusicTicketWizard.vue'
import VideoTicketWizard from '../components/tickets/VideoTicketWizard.vue'
import ErrorAlert from '../components/common/ErrorAlert.vue'
import type { CreateTicketRequest, CreateTicketWithCatalogRequest } from '../api/types'

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

// WebSocket for real-time updates
const ws = useGlobalWebSocket()

// Creation mode: 'none' | 'choose' | 'simple' | 'music' | 'video'
const creationMode = ref<'none' | 'choose' | 'simple' | 'music' | 'video'>('none')

// Handle WebSocket messages
function handleWsMessage(message: WsMessage) {
  if (message.type === 'ticket_update' || message.type === 'ticket_deleted') {
    // Refresh the ticket list when any ticket changes
    fetchTickets()
  }
}

onMounted(() => {
  fetchTickets()
  ws.addHandler(handleWsMessage)
})

onUnmounted(() => {
  ws.removeHandler(handleWsMessage)
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

async function handleCreateTicket(request: CreateTicketRequest | CreateTicketWithCatalogRequest) {
  const ticket = await createTicket(request)
  if (ticket) {
    creationMode.value = 'none'
  }
}

function handleCancelCreate() {
  creationMode.value = 'none'
}
</script>

<template>
  <div>
    <div class="flex items-center justify-between mb-6">
      <h1 class="text-2xl font-bold">Tickets</h1>
      <button
        v-if="creationMode === 'none'"
        @click="creationMode = 'choose'"
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

    <!-- Ticket Type Chooser -->
    <div v-if="creationMode === 'choose'" class="card mb-6">
      <h2 class="text-lg font-semibold mb-4">What do you want to download?</h2>
      <div class="grid grid-cols-1 sm:grid-cols-3 gap-4">
        <!-- Music Option -->
        <button
          @click="creationMode = 'music'"
          class="p-4 border-2 border-gray-200 rounded-lg hover:border-blue-500 hover:bg-blue-50 transition-colors text-left"
        >
          <span class="i-carbon-music text-2xl text-blue-600 mb-2 block"></span>
          <div class="font-medium">Music Album</div>
          <div class="text-sm text-gray-500">Search MusicBrainz catalog</div>
        </button>

        <!-- Video Option -->
        <button
          @click="creationMode = 'video'"
          class="p-4 border-2 border-gray-200 rounded-lg hover:border-blue-500 hover:bg-blue-50 transition-colors text-left"
        >
          <span class="i-carbon-video text-2xl text-blue-600 mb-2 block"></span>
          <div class="font-medium">Movie / TV Show</div>
          <div class="text-sm text-gray-500">Search TMDB catalog</div>
        </button>

        <!-- Simple Form Option -->
        <button
          @click="creationMode = 'simple'"
          class="p-4 border-2 border-gray-200 rounded-lg hover:border-gray-400 hover:bg-gray-50 transition-colors text-left"
        >
          <span class="i-carbon-document text-2xl text-gray-600 mb-2 block"></span>
          <div class="font-medium">Manual Entry</div>
          <div class="text-sm text-gray-500">Simple text description</div>
        </button>
      </div>
      <div class="mt-4 flex justify-end">
        <button @click="creationMode = 'none'" class="btn-secondary">Cancel</button>
      </div>
    </div>

    <!-- Simple Create Form -->
    <CreateTicketForm
      v-if="creationMode === 'simple'"
      @submit="handleCreateTicket"
      @cancel="handleCancelCreate"
      class="mb-6"
    />

    <!-- Music Wizard -->
    <MusicTicketWizard
      v-if="creationMode === 'music'"
      @submit="handleCreateTicket"
      @cancel="handleCancelCreate"
      class="mb-6"
    />

    <!-- Video Wizard -->
    <VideoTicketWizard
      v-if="creationMode === 'video'"
      @submit="handleCreateTicket"
      @cancel="handleCancelCreate"
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
