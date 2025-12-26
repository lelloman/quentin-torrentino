<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useTickets } from '../composables/useTickets'
import TicketDetail from '../components/tickets/TicketDetail.vue'
import LoadingSpinner from '../components/common/LoadingSpinner.vue'
import ErrorAlert from '../components/common/ErrorAlert.vue'

const route = useRoute()
const router = useRouter()

const {
  currentTicket,
  loading,
  error,
  fetchTicket,
  cancelTicket,
  clearError,
} = useTickets()

const cancelReason = ref('')
const showCancelDialog = ref(false)

onMounted(() => {
  const id = route.params.id as string
  fetchTicket(id)
})

async function handleCancel() {
  if (!currentTicket.value) return

  const ticket = await cancelTicket(currentTicket.value.id, {
    reason: cancelReason.value || undefined,
  })

  if (ticket) {
    showCancelDialog.value = false
    cancelReason.value = ''
  }
}

function goBack() {
  router.push('/tickets')
}
</script>

<template>
  <div>
    <div class="mb-6">
      <button @click="goBack" class="text-gray-600 hover:text-gray-900 flex items-center gap-1">
        <span class="i-carbon-arrow-left"></span>
        Back to Tickets
      </button>
    </div>

    <div v-if="loading" class="flex justify-center py-12">
      <LoadingSpinner size="lg" />
    </div>

    <ErrorAlert
      v-else-if="error"
      :message="error"
      @dismiss="clearError"
    />

    <template v-else-if="currentTicket">
      <h1 class="text-2xl font-bold mb-6">Ticket Detail</h1>

      <TicketDetail
        :ticket="currentTicket"
        @cancel="showCancelDialog = true"
      />

      <!-- Cancel Dialog -->
      <div
        v-if="showCancelDialog"
        class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
        @click.self="showCancelDialog = false"
      >
        <div class="card w-full max-w-md mx-4">
          <h2 class="text-lg font-semibold mb-4">Cancel Ticket</h2>
          <p class="text-gray-600 mb-4">
            Are you sure you want to cancel this ticket? This action cannot be undone.
          </p>
          <div class="mb-4">
            <label for="cancelReason" class="block text-sm font-medium text-gray-700 mb-1">
              Reason (optional)
            </label>
            <textarea
              id="cancelReason"
              v-model="cancelReason"
              class="input w-full"
              rows="2"
              placeholder="Why are you cancelling this ticket?"
            ></textarea>
          </div>
          <div class="flex justify-end gap-3">
            <button @click="showCancelDialog = false" class="btn-secondary">
              Keep Ticket
            </button>
            <button @click="handleCancel" class="btn-danger">
              Cancel Ticket
            </button>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>
