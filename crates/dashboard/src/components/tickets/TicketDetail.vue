<script setup lang="ts">
import { computed } from 'vue'
import type { Ticket } from '../../api/types'
import Badge from '../common/Badge.vue'

const props = defineProps<{
  ticket: Ticket
}>()

const emit = defineEmits<{
  cancel: []
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

const canCancel = computed(() => props.ticket.state.type === 'pending')

const formattedCreatedAt = computed(() => {
  return new Date(props.ticket.created_at).toLocaleString()
})

const formattedUpdatedAt = computed(() => {
  return new Date(props.ticket.updated_at).toLocaleString()
})

const stateDetails = computed(() => {
  const state = props.ticket.state
  switch (state.type) {
    case 'cancelled':
      return {
        label: 'Cancelled',
        details: [
          { label: 'Cancelled by', value: state.cancelled_by },
          { label: 'Reason', value: state.reason ?? 'No reason provided' },
          { label: 'Cancelled at', value: new Date(state.cancelled_at).toLocaleString() },
        ],
      }
    case 'completed':
      return {
        label: 'Completed',
        details: [
          { label: 'Completed at', value: new Date(state.completed_at).toLocaleString() },
        ],
      }
    case 'failed':
      return {
        label: 'Failed',
        details: [
          { label: 'Error', value: state.error },
          { label: 'Failed at', value: new Date(state.failed_at).toLocaleString() },
        ],
      }
    default:
      return { label: 'Pending', details: [] }
  }
})
</script>

<template>
  <div class="space-y-4">
    <div class="card">
      <div class="flex items-start justify-between">
        <div>
          <p class="text-sm text-gray-500">Ticket ID</p>
          <p class="font-mono">{{ ticket.id }}</p>
        </div>
        <Badge :variant="stateVariant" class="text-sm">{{ ticket.state.type }}</Badge>
      </div>
    </div>

    <div class="card">
      <h3 class="text-lg font-semibold mb-4">Query Context</h3>
      <div class="space-y-3">
        <div>
          <p class="text-sm text-gray-500">Description</p>
          <p>{{ ticket.query_context.description }}</p>
        </div>
        <div>
          <p class="text-sm text-gray-500 mb-1">Tags</p>
          <div class="flex flex-wrap gap-1">
            <span
              v-for="tag in ticket.query_context.tags"
              :key="tag"
              class="inline-block bg-gray-100 text-gray-600 text-sm px-2 py-0.5 rounded"
            >
              {{ tag }}
            </span>
            <span v-if="ticket.query_context.tags.length === 0" class="text-gray-400 text-sm">
              No tags
            </span>
          </div>
        </div>
      </div>
    </div>

    <div class="card">
      <h3 class="text-lg font-semibold mb-4">Details</h3>
      <div class="space-y-2">
        <div class="flex justify-between py-2 border-b border-gray-100">
          <span class="text-gray-600">Destination Path</span>
          <span class="font-mono text-sm">{{ ticket.dest_path }}</span>
        </div>
        <div class="flex justify-between py-2 border-b border-gray-100">
          <span class="text-gray-600">Priority</span>
          <span>{{ ticket.priority }}</span>
        </div>
        <div class="flex justify-between py-2 border-b border-gray-100">
          <span class="text-gray-600">Created by</span>
          <span>{{ ticket.created_by }}</span>
        </div>
        <div class="flex justify-between py-2 border-b border-gray-100">
          <span class="text-gray-600">Created at</span>
          <span>{{ formattedCreatedAt }}</span>
        </div>
        <div class="flex justify-between py-2">
          <span class="text-gray-600">Updated at</span>
          <span>{{ formattedUpdatedAt }}</span>
        </div>
      </div>
    </div>

    <div v-if="stateDetails.details.length > 0" class="card">
      <h3 class="text-lg font-semibold mb-4">State Details</h3>
      <div class="space-y-2">
        <div
          v-for="detail in stateDetails.details"
          :key="detail.label"
          class="flex justify-between py-2 border-b border-gray-100 last:border-b-0"
        >
          <span class="text-gray-600">{{ detail.label }}</span>
          <span>{{ detail.value }}</span>
        </div>
      </div>
    </div>

    <div v-if="canCancel" class="flex justify-end">
      <button @click="emit('cancel')" class="btn-danger">
        Cancel Ticket
      </button>
    </div>
  </div>
</template>
