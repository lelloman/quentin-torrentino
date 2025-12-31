<script setup lang="ts">
import { onMounted, onUnmounted, ref, computed, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useTickets } from '../composables/useTickets'
import { deleteTicket } from '../api/tickets'
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
const showDeleteDialog = ref(false)
let pollInterval: ReturnType<typeof setInterval> | null = null

// Check if ticket is in an active state that needs polling
const isActiveState = computed(() => {
  if (!currentTicket.value) return false
  const activeStates = ['acquiring', 'downloading', 'converting', 'placing']
  return activeStates.includes(currentTicket.value.state.type)
})

// Check if any dialog is open (pause polling when dialogs are open)
const isDialogOpen = computed(() => showCancelDialog.value || showDeleteDialog.value)

// Start/stop polling based on active state and dialog state
watch([isActiveState, isDialogOpen], ([active, dialogOpen]) => {
  if (active && !dialogOpen && !pollInterval) {
    pollInterval = setInterval(() => {
      const id = route.params.id as string
      fetchTicket(id)
    }, 3000)
  } else if ((!active || dialogOpen) && pollInterval) {
    clearInterval(pollInterval)
    pollInterval = null
  }
})

onMounted(() => {
  const id = route.params.id as string
  fetchTicket(id)
})

onUnmounted(() => {
  if (pollInterval) {
    clearInterval(pollInterval)
  }
})

function handleRefresh() {
  const id = route.params.id as string
  fetchTicket(id)
}

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

const deleteLoading = ref(false)
const deleteError = ref<string | null>(null)

async function handleDelete() {
  if (!currentTicket.value) return

  deleteLoading.value = true
  deleteError.value = null
  try {
    await deleteTicket(currentTicket.value.id)
    showDeleteDialog.value = false
    goBack()
  } catch (e) {
    deleteError.value = e instanceof Error ? e.message : 'Failed to delete'
  } finally {
    deleteLoading.value = false
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
        @showDelete="showDeleteDialog = true"
        @refresh="handleRefresh"
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

      <!-- Delete Dialog -->
      <div
        v-if="showDeleteDialog"
        class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
        @click.self="showDeleteDialog = false"
      >
        <div class="bg-white rounded-lg p-6 max-w-md w-full mx-4 shadow-xl">
          <h3 class="text-lg font-bold text-red-800 mb-4">Delete Ticket Permanently?</h3>
          <p class="text-gray-600 mb-2">
            This action cannot be undone. The ticket and all associated data will be permanently removed.
          </p>
          <p class="text-sm text-gray-500 mb-6 font-mono">
            ID: {{ currentTicket.id }}
          </p>

          <div v-if="deleteError" class="mb-4 p-3 bg-red-100 text-red-700 rounded text-sm">
            {{ deleteError }}
          </div>

          <div class="flex justify-end gap-3">
            <button
              @click="showDeleteDialog = false"
              :disabled="deleteLoading"
              class="px-4 py-2 text-gray-600 hover:text-gray-800"
            >
              Cancel
            </button>
            <button
              @click="handleDelete"
              :disabled="deleteLoading"
              class="btn-danger"
            >
              {{ deleteLoading ? 'Deleting...' : 'Yes, Delete Permanently' }}
            </button>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>
