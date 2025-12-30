<script setup lang="ts">
import { computed, ref } from 'vue'
import type { Ticket } from '../../api/types'
import { approveTicket, rejectTicket } from '../../api/tickets'
import Badge from '../common/Badge.vue'

const props = defineProps<{
  ticket: Ticket
}>()

const emit = defineEmits<{
  cancel: []
  refresh: []
}>()

const actionLoading = ref(false)
const actionError = ref<string | null>(null)
const selectedCandidateIdx = ref(0)
const rejectReason = ref('')

// State variant for badge color
const stateVariant = computed(() => {
  switch (props.ticket.state.type) {
    case 'pending':
      return 'info'
    case 'acquiring':
      return 'info'
    case 'acquisition_failed':
      return 'danger'
    case 'needs_approval':
      return 'warning'
    case 'auto_approved':
    case 'approved':
      return 'success'
    case 'rejected':
      return 'warning'
    case 'downloading':
      return 'info'
    case 'converting':
    case 'placing':
      return 'info'
    case 'completed':
      return 'success'
    case 'failed':
      return 'danger'
    case 'cancelled':
      return 'warning'
    default:
      return 'default'
  }
})

// Human-readable state name
const stateName = computed(() => {
  const type = props.ticket.state.type
  return type.replace(/_/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase())
})

// Can this ticket be cancelled?
const canCancel = computed(() => {
  const type = props.ticket.state.type
  return ['pending', 'needs_approval', 'downloading'].includes(type)
})

// Is this an active/in-progress state?
const isActive = computed(() => {
  const type = props.ticket.state.type
  return ['acquiring', 'downloading', 'converting', 'placing'].includes(type)
})

const formattedCreatedAt = computed(() => {
  return new Date(props.ticket.created_at).toLocaleString()
})

const formattedUpdatedAt = computed(() => {
  return new Date(props.ticket.updated_at).toLocaleString()
})

// Format bytes to human readable
function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
}

// Format speed to human readable
function formatSpeed(bps: number): string {
  return formatBytes(bps) + '/s'
}

// Format duration
function formatDuration(secs: number): string {
  if (secs < 60) return `${secs}s`
  if (secs < 3600) return `${Math.floor(secs / 60)}m ${secs % 60}s`
  const hours = Math.floor(secs / 3600)
  const mins = Math.floor((secs % 3600) / 60)
  return `${hours}h ${mins}m`
}

// Handle approve action
async function handleApprove() {
  actionLoading.value = true
  actionError.value = null
  try {
    await approveTicket(props.ticket.id, { candidate_idx: selectedCandidateIdx.value })
    emit('refresh')
  } catch (e) {
    actionError.value = e instanceof Error ? e.message : 'Failed to approve'
  } finally {
    actionLoading.value = false
  }
}

// Handle reject action
async function handleReject() {
  actionLoading.value = true
  actionError.value = null
  try {
    await rejectTicket(props.ticket.id, {
      reason: rejectReason.value || undefined,
    })
    emit('refresh')
  } catch (e) {
    actionError.value = e instanceof Error ? e.message : 'Failed to reject'
  } finally {
    actionLoading.value = false
  }
}
</script>

<template>
  <div class="space-y-4">
    <!-- Header -->
    <div class="card">
      <div class="flex items-start justify-between">
        <div>
          <p class="text-sm text-gray-500">Ticket ID</p>
          <p class="font-mono">{{ ticket.id }}</p>
        </div>
        <div class="flex items-center gap-2">
          <span v-if="isActive" class="animate-pulse w-2 h-2 bg-blue-500 rounded-full"></span>
          <Badge :variant="stateVariant" class="text-sm">{{ stateName }}</Badge>
        </div>
      </div>
    </div>

    <!-- Query Context -->
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

    <!-- Details -->
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

    <!-- State-specific content -->

    <!-- Acquiring State -->
    <div v-if="ticket.state.type === 'acquiring'" class="card">
      <h3 class="text-lg font-semibold mb-4">Acquisition Progress</h3>
      <div class="space-y-3">
        <div class="flex justify-between">
          <span class="text-gray-600">Phase</span>
          <Badge variant="info">{{ ticket.state.phase.replace(/_/g, ' ') }}</Badge>
        </div>
        <div class="flex justify-between">
          <span class="text-gray-600">Queries Tried</span>
          <span>{{ ticket.state.queries_tried.length }}</span>
        </div>
        <div class="flex justify-between">
          <span class="text-gray-600">Candidates Found</span>
          <span>{{ ticket.state.candidates_found }}</span>
        </div>
        <div v-if="ticket.state.queries_tried.length > 0">
          <p class="text-sm text-gray-500 mb-2">Queries:</p>
          <div class="space-y-1">
            <div
              v-for="(query, idx) in ticket.state.queries_tried"
              :key="idx"
              class="text-sm font-mono bg-gray-50 px-2 py-1 rounded"
            >
              {{ query }}
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Acquisition Failed State -->
    <div v-if="ticket.state.type === 'acquisition_failed'" class="card border-red-200 bg-red-50">
      <h3 class="text-lg font-semibold mb-4 text-red-800">Acquisition Failed</h3>
      <div class="space-y-2 text-red-700">
        <p>{{ ticket.state.reason }}</p>
        <p class="text-sm">
          Tried {{ ticket.state.queries_tried.length }} queries,
          evaluated {{ ticket.state.candidates_seen }} candidates.
        </p>
      </div>
    </div>

    <!-- Needs Approval State -->
    <div v-if="ticket.state.type === 'needs_approval'" class="card border-orange-200 bg-orange-50">
      <h3 class="text-lg font-semibold mb-4 text-orange-800">Needs Approval</h3>

      <div v-if="actionError" class="mb-4 p-3 bg-red-100 text-red-700 rounded">
        {{ actionError }}
      </div>

      <div class="mb-4">
        <p class="text-sm text-gray-600 mb-2">
          Confidence: <span class="font-medium">{{ (ticket.state.confidence * 100).toFixed(0) }}%</span>
          (below auto-approve threshold)
        </p>
      </div>

      <div class="space-y-3 mb-4">
        <p class="text-sm font-medium text-gray-700">Select a candidate:</p>
        <div
          v-for="(candidate, idx) in ticket.state.candidates"
          :key="candidate.info_hash"
          class="border rounded-lg p-3 cursor-pointer transition-colors"
          :class="{
            'border-orange-500 bg-white': selectedCandidateIdx === idx,
            'border-gray-200 bg-white hover:border-gray-300': selectedCandidateIdx !== idx,
          }"
          @click="selectedCandidateIdx = idx"
        >
          <div class="flex items-start justify-between">
            <div class="flex-1 min-w-0">
              <p class="font-medium truncate" :title="candidate.title">{{ candidate.title }}</p>
              <p class="text-sm text-gray-500">
                {{ formatBytes(candidate.size_bytes) }} · {{ candidate.seeders }} seeders
              </p>
              <p class="text-sm text-gray-600 mt-1">{{ candidate.reasoning }}</p>
            </div>
            <div class="ml-3 flex flex-col items-end">
              <Badge v-if="idx === ticket.state.recommended_idx" variant="success" class="text-xs mb-1">
                Recommended
              </Badge>
              <span class="text-lg font-bold" :class="{
                'text-green-600': candidate.score >= 0.8,
                'text-yellow-600': candidate.score >= 0.5 && candidate.score < 0.8,
                'text-red-600': candidate.score < 0.5,
              }">
                {{ (candidate.score * 100).toFixed(0) }}%
              </span>
            </div>
          </div>
        </div>
      </div>

      <div class="flex gap-3">
        <button
          @click="handleApprove"
          :disabled="actionLoading"
          class="btn-primary flex-1"
        >
          {{ actionLoading ? 'Approving...' : 'Approve Selected' }}
        </button>
        <button
          @click="handleReject"
          :disabled="actionLoading"
          class="btn-danger"
        >
          Reject
        </button>
      </div>
    </div>

    <!-- Approved / Auto-Approved State -->
    <div v-if="ticket.state.type === 'approved' || ticket.state.type === 'auto_approved'" class="card border-green-200 bg-green-50">
      <h3 class="text-lg font-semibold mb-4 text-green-800">
        {{ ticket.state.type === 'auto_approved' ? 'Auto-Approved' : 'Approved' }}
      </h3>
      <div class="space-y-2">
        <div>
          <p class="text-sm text-gray-600">Selected Torrent</p>
          <p class="font-medium">{{ ticket.state.selected.title }}</p>
          <p class="text-sm text-gray-500">
            {{ formatBytes(ticket.state.selected.size_bytes) }} ·
            Score: {{ (ticket.state.selected.score * 100).toFixed(0) }}%
          </p>
        </div>
        <div v-if="ticket.state.type === 'approved'" class="text-sm text-gray-600">
          Approved by: {{ ticket.state.approved_by }}
        </div>
        <div v-else class="text-sm text-gray-600">
          Confidence: {{ (ticket.state.confidence * 100).toFixed(0) }}%
        </div>
      </div>
    </div>

    <!-- Rejected State -->
    <div v-if="ticket.state.type === 'rejected'" class="card border-orange-200 bg-orange-50">
      <h3 class="text-lg font-semibold mb-4 text-orange-800">Rejected</h3>
      <div class="space-y-2">
        <p class="text-sm text-gray-600">Rejected by: {{ ticket.state.rejected_by }}</p>
        <p v-if="ticket.state.reason" class="text-gray-700">{{ ticket.state.reason }}</p>
        <p v-else class="text-gray-500 italic">No reason provided</p>
      </div>
    </div>

    <!-- Downloading State -->
    <div v-if="ticket.state.type === 'downloading'" class="card border-blue-200 bg-blue-50">
      <h3 class="text-lg font-semibold mb-4 text-blue-800">Downloading</h3>
      <div class="space-y-3">
        <!-- Progress bar -->
        <div>
          <div class="flex justify-between text-sm mb-1">
            <span>Progress</span>
            <span class="font-medium">{{ ticket.state.progress_pct.toFixed(1) }}%</span>
          </div>
          <div class="w-full bg-gray-200 rounded-full h-3">
            <div
              class="bg-blue-600 h-3 rounded-full transition-all duration-300"
              :style="{ width: `${ticket.state.progress_pct}%` }"
            ></div>
          </div>
        </div>

        <div class="grid grid-cols-2 gap-4 text-sm">
          <div>
            <p class="text-gray-600">Speed</p>
            <p class="font-medium">{{ formatSpeed(ticket.state.speed_bps) }}</p>
          </div>
          <div>
            <p class="text-gray-600">ETA</p>
            <p class="font-medium">
              {{ ticket.state.eta_secs ? formatDuration(ticket.state.eta_secs) : 'Unknown' }}
            </p>
          </div>
        </div>

        <div v-if="ticket.state.failover_round > 1" class="text-sm text-orange-600">
          Failover round {{ ticket.state.failover_round }}, candidate {{ ticket.state.candidate_idx + 1 }}
        </div>
      </div>
    </div>

    <!-- Converting State -->
    <div v-if="ticket.state.type === 'converting'" class="card border-purple-200 bg-purple-50">
      <h3 class="text-lg font-semibold mb-4 text-purple-800">Converting</h3>
      <div class="space-y-3">
        <div>
          <div class="flex justify-between text-sm mb-1">
            <span>File {{ ticket.state.current_idx + 1 }} of {{ ticket.state.total }}</span>
            <span class="font-medium">
              {{ ((ticket.state.current_idx / ticket.state.total) * 100).toFixed(0) }}%
            </span>
          </div>
          <div class="w-full bg-gray-200 rounded-full h-3">
            <div
              class="bg-purple-600 h-3 rounded-full transition-all duration-300"
              :style="{ width: `${(ticket.state.current_idx / ticket.state.total) * 100}%` }"
            ></div>
          </div>
        </div>
        <p class="text-sm text-gray-600 truncate">
          Current: {{ ticket.state.current_name }}
        </p>
      </div>
    </div>

    <!-- Placing State -->
    <div v-if="ticket.state.type === 'placing'" class="card border-indigo-200 bg-indigo-50">
      <h3 class="text-lg font-semibold mb-4 text-indigo-800">Placing Files</h3>
      <div class="space-y-3">
        <div>
          <div class="flex justify-between text-sm mb-1">
            <span>{{ ticket.state.files_placed }} of {{ ticket.state.total_files }} files</span>
            <span class="font-medium">
              {{ ((ticket.state.files_placed / ticket.state.total_files) * 100).toFixed(0) }}%
            </span>
          </div>
          <div class="w-full bg-gray-200 rounded-full h-3">
            <div
              class="bg-indigo-600 h-3 rounded-full transition-all duration-300"
              :style="{ width: `${(ticket.state.files_placed / ticket.state.total_files) * 100}%` }"
            ></div>
          </div>
        </div>
      </div>
    </div>

    <!-- Completed State -->
    <div v-if="ticket.state.type === 'completed'" class="card border-green-200 bg-green-50">
      <h3 class="text-lg font-semibold mb-4 text-green-800">Completed</h3>
      <div v-if="ticket.state.stats" class="grid grid-cols-2 gap-4 text-sm">
        <div>
          <p class="text-gray-600">Downloaded</p>
          <p class="font-medium">{{ formatBytes(ticket.state.stats.total_download_bytes) }}</p>
        </div>
        <div>
          <p class="text-gray-600">Final Size</p>
          <p class="font-medium">{{ formatBytes(ticket.state.stats.final_size_bytes) }}</p>
        </div>
        <div>
          <p class="text-gray-600">Download Time</p>
          <p class="font-medium">{{ formatDuration(ticket.state.stats.download_duration_secs) }}</p>
        </div>
        <div>
          <p class="text-gray-600">Files Placed</p>
          <p class="font-medium">{{ ticket.state.stats.files_placed }}</p>
        </div>
      </div>
      <p v-else class="text-green-700">
        Completed at {{ new Date(ticket.state.completed_at).toLocaleString() }}
      </p>
    </div>

    <!-- Failed State -->
    <div v-if="ticket.state.type === 'failed'" class="card border-red-200 bg-red-50">
      <h3 class="text-lg font-semibold mb-4 text-red-800">Failed</h3>
      <div class="space-y-2">
        <p class="text-red-700">{{ ticket.state.error }}</p>
        <div class="flex gap-4 text-sm text-gray-600">
          <span v-if="ticket.state.retryable">
            <Badge variant="info">Retryable</Badge>
          </span>
          <span v-if="ticket.state.retry_count">
            Retry attempts: {{ ticket.state.retry_count }}
          </span>
        </div>
      </div>
    </div>

    <!-- Cancelled State -->
    <div v-if="ticket.state.type === 'cancelled'" class="card border-gray-200 bg-gray-50">
      <h3 class="text-lg font-semibold mb-4 text-gray-800">Cancelled</h3>
      <div class="space-y-2">
        <p class="text-sm text-gray-600">Cancelled by: {{ ticket.state.cancelled_by }}</p>
        <p v-if="ticket.state.reason" class="text-gray-700">{{ ticket.state.reason }}</p>
        <p v-else class="text-gray-500 italic">No reason provided</p>
      </div>
    </div>

    <!-- Cancel Button -->
    <div v-if="canCancel" class="flex justify-end">
      <button @click="emit('cancel')" class="btn-danger">Cancel Ticket</button>
    </div>
  </div>
</template>
