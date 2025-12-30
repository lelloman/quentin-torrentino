<script setup lang="ts">
import { onMounted, computed } from 'vue'
import { RouterLink } from 'vue-router'
import { storeToRefs } from 'pinia'
import { useAppStore } from '../stores/app'
import { useTickets } from '../composables/useTickets'
import LoadingSpinner from '../components/common/LoadingSpinner.vue'
import ErrorAlert from '../components/common/ErrorAlert.vue'
import Badge from '../components/common/Badge.vue'
import TicketCard from '../components/tickets/TicketCard.vue'
import OrchestratorStatus from '../components/orchestrator/OrchestratorStatus.vue'

const store = useAppStore()
const { health, healthLoading, healthError } = storeToRefs(store)

const { tickets, loading: ticketsLoading, error: ticketsError, fetchTickets, clearError } = useTickets()

const recentTickets = computed(() => tickets.value.slice(0, 5))

const stats = computed(() => {
  const pending = tickets.value.filter((t) => t.state.type === 'pending').length
  const completed = tickets.value.filter((t) => t.state.type === 'completed').length
  const failed = tickets.value.filter((t) => t.state.type === 'failed').length
  return { pending, completed, failed, total: tickets.value.length }
})

onMounted(() => {
  store.fetchHealth()
  fetchTickets()
})
</script>

<template>
  <div>
    <h1 class="text-2xl font-bold mb-6">Dashboard</h1>

    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-8">
      <!-- Health Status Card -->
      <div class="card">
        <div class="flex items-center justify-between">
          <div>
            <p class="text-sm text-gray-500">System Status</p>
            <div class="mt-1">
              <LoadingSpinner v-if="healthLoading" size="sm" />
              <Badge
                v-else-if="health"
                :variant="health.status === 'ok' ? 'success' : 'danger'"
              >
                {{ health.status }}
              </Badge>
              <Badge v-else-if="healthError" variant="danger">Error</Badge>
            </div>
          </div>
          <span class="i-carbon-activity text-2xl text-gray-400"></span>
        </div>
      </div>

      <!-- Total Tickets -->
      <div class="card">
        <div class="flex items-center justify-between">
          <div>
            <p class="text-sm text-gray-500">Total Tickets</p>
            <p class="text-2xl font-bold mt-1">{{ stats.total }}</p>
          </div>
          <span class="i-carbon-ticket text-2xl text-gray-400"></span>
        </div>
      </div>

      <!-- Pending Tickets -->
      <div class="card">
        <div class="flex items-center justify-between">
          <div>
            <p class="text-sm text-gray-500">Pending</p>
            <p class="text-2xl font-bold mt-1 text-blue-600">{{ stats.pending }}</p>
          </div>
          <span class="i-carbon-time text-2xl text-blue-400"></span>
        </div>
      </div>

      <!-- Completed Tickets -->
      <div class="card">
        <div class="flex items-center justify-between">
          <div>
            <p class="text-sm text-gray-500">Completed</p>
            <p class="text-2xl font-bold mt-1 text-green-600">{{ stats.completed }}</p>
          </div>
          <span class="i-carbon-checkmark text-2xl text-green-400"></span>
        </div>
      </div>
    </div>

    <!-- Orchestrator Status -->
    <div class="mb-8">
      <OrchestratorStatus />
    </div>

    <ErrorAlert
      v-if="ticketsError"
      :message="ticketsError"
      @dismiss="clearError"
      class="mb-4"
    />

    <div class="flex items-center justify-between mb-4">
      <h2 class="text-lg font-semibold">Recent Tickets</h2>
      <RouterLink to="/tickets" class="text-primary hover:underline text-sm">
        View all
      </RouterLink>
    </div>

    <div v-if="ticketsLoading" class="flex justify-center py-8">
      <LoadingSpinner />
    </div>

    <div v-else-if="recentTickets.length === 0" class="card text-center py-8 text-gray-500">
      No tickets yet
    </div>

    <div v-else class="space-y-3">
      <TicketCard
        v-for="ticket in recentTickets"
        :key="ticket.id"
        :ticket="ticket"
      />
    </div>
  </div>
</template>
