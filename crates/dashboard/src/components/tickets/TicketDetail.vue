<script setup lang="ts">
import { computed, ref } from 'vue'
import type { Ticket } from '../../api/types'
import { approveTicket, rejectTicket, retryTicket } from '../../api/tickets'
import Badge from '../common/Badge.vue'

const props = defineProps<{
  ticket: Ticket
}>()

const emit = defineEmits<{
  cancel: []
  showDelete: []
  refresh: []
}>()

const actionLoading = ref(false)
const actionError = ref<string | null>(null)
const selectedCandidateIdx = ref(0)
const rejectReason = ref('')
const retryLoading = ref(false)
const showFailoverCandidates = ref(false)

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

// Can this ticket be retried?
const canRetry = computed(() => {
  const state = props.ticket.state
  if (state.type === 'failed' && state.retryable) return true
  if (state.type === 'acquisition_failed') return true
  if (state.type === 'rejected') return true
  if (state.type === 'cancelled') return true
  return false
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

// Format track duration (mm:ss)
function formatTrackDuration(secs: number): string {
  const mins = Math.floor(secs / 60)
  const s = secs % 60
  return `${mins}:${s.toString().padStart(2, '0')}`
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

// Handle retry action
async function handleRetry() {
  retryLoading.value = true
  actionError.value = null
  try {
    await retryTicket(props.ticket.id)
    emit('refresh')
  } catch (e) {
    actionError.value = e instanceof Error ? e.message : 'Failed to retry'
  } finally {
    retryLoading.value = false
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

    <!-- Catalog Reference (from wizard) -->
    <div v-if="ticket.query_context.catalog_reference" class="card border-purple-200 bg-purple-50">
      <h3 class="text-lg font-semibold mb-4 text-purple-800">Catalog Reference</h3>
      <div class="space-y-2">
        <div class="flex justify-between py-2 border-b border-purple-100">
          <span class="text-gray-600">Source</span>
          <Badge variant="info">
            {{ ticket.query_context.catalog_reference.type === 'music_brainz' ? 'MusicBrainz' : 'TMDB' }}
          </Badge>
        </div>
        <template v-if="ticket.query_context.catalog_reference.type === 'music_brainz'">
          <div class="flex justify-between py-2 border-b border-purple-100">
            <span class="text-gray-600">Release ID</span>
            <a
              :href="`https://musicbrainz.org/release/${ticket.query_context.catalog_reference.release_id}`"
              target="_blank"
              class="font-mono text-sm text-purple-600 hover:text-purple-800"
            >
              {{ ticket.query_context.catalog_reference.release_id }}
            </a>
          </div>
          <div class="flex justify-between py-2 border-b border-purple-100">
            <span class="text-gray-600">Track Count</span>
            <span>{{ ticket.query_context.catalog_reference.track_count }}</span>
          </div>
          <div v-if="ticket.query_context.catalog_reference.total_duration_ms" class="flex justify-between py-2">
            <span class="text-gray-600">Total Duration</span>
            <span>{{ formatDuration(Math.round(ticket.query_context.catalog_reference.total_duration_ms / 1000)) }}</span>
          </div>
        </template>
        <template v-else-if="ticket.query_context.catalog_reference.type === 'tmdb'">
          <div class="flex justify-between py-2 border-b border-purple-100">
            <span class="text-gray-600">Media Type</span>
            <Badge :variant="ticket.query_context.catalog_reference.media_type === 'movie' ? 'info' : 'success'">
              {{ ticket.query_context.catalog_reference.media_type === 'movie' ? 'Movie' : 'TV Show' }}
            </Badge>
          </div>
          <div class="flex justify-between py-2 border-b border-purple-100">
            <span class="text-gray-600">TMDB ID</span>
            <a
              :href="`https://www.themoviedb.org/${ticket.query_context.catalog_reference.media_type}/${ticket.query_context.catalog_reference.id}`"
              target="_blank"
              class="font-mono text-sm text-purple-600 hover:text-purple-800"
            >
              {{ ticket.query_context.catalog_reference.id }}
            </a>
          </div>
          <div v-if="ticket.query_context.catalog_reference.runtime_minutes" class="flex justify-between py-2 border-b border-purple-100">
            <span class="text-gray-600">Runtime</span>
            <span>{{ ticket.query_context.catalog_reference.runtime_minutes }} min</span>
          </div>
          <div v-if="ticket.query_context.catalog_reference.episode_count" class="flex justify-between py-2">
            <span class="text-gray-600">Episodes</span>
            <span>{{ ticket.query_context.catalog_reference.episode_count }}</span>
          </div>
        </template>
      </div>
    </div>

    <!-- Expected Content (from wizard) -->
    <div v-if="ticket.query_context.expected" class="card border-green-200 bg-green-50">
      <h3 class="text-lg font-semibold mb-4 text-green-800">Expected Content</h3>

      <!-- Album -->
      <template v-if="ticket.query_context.expected.type === 'album'">
        <div class="space-y-3">
          <div v-if="ticket.query_context.expected.artist" class="flex justify-between">
            <span class="text-gray-600">Artist</span>
            <span class="font-medium">{{ ticket.query_context.expected.artist }}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-gray-600">Album</span>
            <span class="font-medium">{{ ticket.query_context.expected.title }}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-gray-600">Tracks</span>
            <span>{{ ticket.query_context.expected.tracks.length }}</span>
          </div>
          <div v-if="ticket.query_context.expected.tracks.length > 0" class="mt-3">
            <p class="text-sm text-gray-600 mb-2">Track List:</p>
            <div class="bg-white rounded-lg border border-green-200 overflow-hidden">
              <div
                v-for="track in ticket.query_context.expected.tracks"
                :key="track.number"
                class="flex items-center justify-between px-3 py-1.5 text-sm border-b border-green-100 last:border-b-0"
              >
                <div class="flex items-center gap-2 min-w-0 flex-1">
                  <span class="text-gray-400 w-6 text-right">{{ track.number }}.</span>
                  <span class="truncate">{{ track.title }}</span>
                </div>
                <span v-if="track.duration_secs || track.duration_ms" class="text-gray-500 text-xs ml-2">
                  {{ formatTrackDuration(track.duration_ms ? Math.round(track.duration_ms / 1000) : track.duration_secs!) }}
                </span>
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- Movie -->
      <template v-else-if="ticket.query_context.expected.type === 'movie'">
        <div class="space-y-2">
          <div class="flex justify-between">
            <span class="text-gray-600">Title</span>
            <span class="font-medium">{{ ticket.query_context.expected.title }}</span>
          </div>
          <div v-if="ticket.query_context.expected.year" class="flex justify-between">
            <span class="text-gray-600">Year</span>
            <span>{{ ticket.query_context.expected.year }}</span>
          </div>
        </div>
      </template>

      <!-- TV Episode -->
      <template v-else-if="ticket.query_context.expected.type === 'tv_episode'">
        <div class="space-y-2">
          <div class="flex justify-between">
            <span class="text-gray-600">Series</span>
            <span class="font-medium">{{ ticket.query_context.expected.series }}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-gray-600">Season</span>
            <span>{{ ticket.query_context.expected.season }}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-gray-600">Episodes</span>
            <span>{{ ticket.query_context.expected.episodes.join(', ') }}</span>
          </div>
        </div>
      </template>
    </div>

    <!-- Search Constraints (from wizard) -->
    <div v-if="ticket.query_context.search_constraints" class="card border-blue-200 bg-blue-50">
      <h3 class="text-lg font-semibold mb-4 text-blue-800">Search Constraints</h3>
      <div class="space-y-2">
        <!-- Audio constraints -->
        <template v-if="ticket.query_context.search_constraints.audio">
          <div v-if="ticket.query_context.search_constraints.audio.preferred_formats?.length" class="flex justify-between py-2 border-b border-blue-100">
            <span class="text-gray-600">Preferred Formats</span>
            <span>{{ ticket.query_context.search_constraints.audio.preferred_formats.join(', ').toUpperCase() }}</span>
          </div>
          <div v-if="ticket.query_context.search_constraints.audio.min_bitrate_kbps" class="flex justify-between py-2 border-b border-blue-100">
            <span class="text-gray-600">Min Bitrate</span>
            <span>{{ ticket.query_context.search_constraints.audio.min_bitrate_kbps }} kbps</span>
          </div>
          <div v-if="ticket.query_context.search_constraints.audio.avoid_compilations" class="flex justify-between py-2 border-b border-blue-100">
            <span class="text-gray-600">Avoid Compilations</span>
            <Badge variant="warning">Yes</Badge>
          </div>
          <div v-if="ticket.query_context.search_constraints.audio.avoid_live" class="flex justify-between py-2">
            <span class="text-gray-600">Avoid Live Recordings</span>
            <Badge variant="warning">Yes</Badge>
          </div>
        </template>
        <!-- Video constraints -->
        <template v-if="ticket.query_context.search_constraints.video">
          <div v-if="ticket.query_context.search_constraints.video.min_resolution" class="flex justify-between py-2 border-b border-blue-100">
            <span class="text-gray-600">Min Resolution</span>
            <span>{{ ticket.query_context.search_constraints.video.min_resolution.replace('r', '') }}</span>
          </div>
          <div v-if="ticket.query_context.search_constraints.video.preferred_resolution" class="flex justify-between py-2 border-b border-blue-100">
            <span class="text-gray-600">Preferred Resolution</span>
            <span>{{ ticket.query_context.search_constraints.video.preferred_resolution.replace('r', '') }}</span>
          </div>
          <div v-if="ticket.query_context.search_constraints.video.preferred_sources?.length" class="flex justify-between py-2 border-b border-blue-100">
            <span class="text-gray-600">Preferred Sources</span>
            <span>{{ ticket.query_context.search_constraints.video.preferred_sources.join(', ') }}</span>
          </div>
          <div v-if="ticket.query_context.search_constraints.video.preferred_codecs?.length" class="flex justify-between py-2 border-b border-blue-100">
            <span class="text-gray-600">Preferred Codecs</span>
            <span>{{ ticket.query_context.search_constraints.video.preferred_codecs.join(', ') }}</span>
          </div>
          <div v-if="ticket.query_context.search_constraints.video.preferred_language" class="flex justify-between py-2 border-b border-blue-100">
            <span class="text-gray-600">Preferred Language</span>
            <span>{{ ticket.query_context.search_constraints.video.preferred_language }}</span>
          </div>
          <div v-if="ticket.query_context.search_constraints.video.exclude_hardcoded_subs" class="flex justify-between py-2">
            <span class="text-gray-600">Exclude Hardcoded Subs</span>
            <Badge variant="warning">Yes</Badge>
          </div>
        </template>
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
          <Badge variant="info">{{ ticket.state.phase.phase.replace(/_/g, ' ') }}</Badge>
        </div>
        <div v-if="ticket.state.phase.phase === 'searching'" class="flex justify-between">
          <span class="text-gray-600">Current Query</span>
          <span class="font-mono text-sm">{{ ticket.state.phase.query }}</span>
        </div>
        <div v-if="ticket.state.phase.phase === 'scoring'" class="flex justify-between">
          <span class="text-gray-600">Scoring Candidates</span>
          <span>{{ ticket.state.phase.candidates_count }}</span>
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
          <p class="text-sm text-gray-500 mb-2">Generated Queries:</p>
          <div class="space-y-1">
            <div
              v-for="(query, idx) in ticket.state.queries_tried"
              :key="idx"
              class="text-sm font-mono bg-gray-50 px-2 py-1 rounded flex items-center gap-2"
            >
              <span class="text-gray-400">{{ idx + 1 }}.</span>
              <span>{{ query }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Acquisition Failed State -->
    <div v-if="ticket.state.type === 'acquisition_failed'" class="card border-red-200 bg-red-50">
      <h3 class="text-lg font-semibold mb-4 text-red-800">Acquisition Failed</h3>
      <div class="space-y-3">
        <p class="text-red-700">{{ ticket.state.reason }}</p>
        <div class="grid grid-cols-2 gap-4 text-sm">
          <div>
            <span class="text-gray-600">Queries Tried</span>
            <p class="font-medium">{{ ticket.state.queries_tried.length }}</p>
          </div>
          <div>
            <span class="text-gray-600">Candidates Evaluated</span>
            <p class="font-medium">{{ ticket.state.candidates_seen }}</p>
          </div>
        </div>
        <div v-if="ticket.state.queries_tried.length > 0">
          <p class="text-sm text-gray-600 mb-2">Queries that were tried:</p>
          <div class="space-y-1">
            <div
              v-for="(query, idx) in ticket.state.queries_tried"
              :key="idx"
              class="text-sm font-mono bg-red-100 px-2 py-1 rounded flex items-center gap-2"
            >
              <span class="text-red-400">{{ idx + 1 }}.</span>
              <span class="text-red-800">{{ query }}</span>
            </div>
          </div>
        </div>
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
              <p class="text-xs font-mono text-gray-400 mt-1">{{ candidate.info_hash }}</p>
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

      <p class="text-sm text-gray-500 mb-4">
        All {{ ticket.state.candidates.length }} candidates will be available for failover if the selected one fails.
      </p>

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
      <div class="space-y-4">
        <!-- Selected candidate -->
        <div class="bg-white rounded-lg p-3 border border-green-300">
          <div class="flex items-start justify-between">
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-2 mb-1">
                <Badge variant="success" class="text-xs">Selected</Badge>
              </div>
              <p class="font-medium truncate" :title="ticket.state.selected.title">
                {{ ticket.state.selected.title }}
              </p>
              <p class="text-sm text-gray-500">
                {{ formatBytes(ticket.state.selected.size_bytes) }} ·
                Score: {{ (ticket.state.selected.score * 100).toFixed(0) }}%
              </p>
              <p class="text-xs font-mono text-gray-400 mt-1 truncate">
                {{ ticket.state.selected.info_hash }}
              </p>
            </div>
          </div>
        </div>

        <!-- Approval info -->
        <div class="text-sm text-gray-600">
          <span v-if="ticket.state.type === 'approved'">
            Approved by {{ ticket.state.approved_by }} at {{ new Date(ticket.state.approved_at).toLocaleString() }}
          </span>
          <span v-else>
            Auto-approved with {{ (ticket.state.confidence * 100).toFixed(0) }}% confidence at {{ new Date(ticket.state.approved_at).toLocaleString() }}
          </span>
        </div>

        <!-- Failover candidates -->
        <div v-if="ticket.state.candidates && ticket.state.candidates.length > 1">
          <button
            @click="showFailoverCandidates = !showFailoverCandidates"
            class="text-sm text-green-700 hover:text-green-900 flex items-center gap-1"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              class="h-4 w-4 transition-transform"
              :class="{ 'rotate-90': showFailoverCandidates }"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
            >
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
            </svg>
            {{ ticket.state.candidates.length - 1 }} failover candidate{{ ticket.state.candidates.length > 2 ? 's' : '' }} available
          </button>
          <div v-if="showFailoverCandidates" class="mt-2 space-y-2">
            <div
              v-for="(candidate, idx) in ticket.state.candidates.slice(1)"
              :key="candidate.info_hash"
              class="bg-white rounded-lg p-2 border border-gray-200 text-sm"
            >
              <div class="flex items-start justify-between">
                <div class="flex-1 min-w-0">
                  <p class="font-medium truncate" :title="candidate.title">
                    <span class="text-gray-400 mr-1">#{{ idx + 2 }}</span>
                    {{ candidate.title }}
                  </p>
                  <p class="text-xs text-gray-500">
                    {{ formatBytes(candidate.size_bytes) }} ·
                    Score: {{ (candidate.score * 100).toFixed(0) }}%
                  </p>
                </div>
              </div>
            </div>
          </div>
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
      <div class="space-y-4">
        <!-- Current candidate being downloaded -->
        <div v-if="ticket.state.candidates && ticket.state.candidates.length > 0" class="bg-white rounded-lg p-3 border border-blue-300">
          <div class="flex items-center gap-2 mb-1">
            <Badge variant="info" class="text-xs">
              Candidate {{ ticket.state.candidate_idx + 1 }} of {{ ticket.state.candidates.length }}
            </Badge>
            <Badge v-if="ticket.state.failover_round > 1" variant="warning" class="text-xs">
              Round {{ ticket.state.failover_round }}
            </Badge>
          </div>
          <p class="font-medium truncate" :title="ticket.state.candidates[ticket.state.candidate_idx]?.title">
            {{ ticket.state.candidates[ticket.state.candidate_idx]?.title || 'Unknown' }}
          </p>
          <p class="text-sm text-gray-500">
            {{ formatBytes(ticket.state.candidates[ticket.state.candidate_idx]?.size_bytes || 0) }} ·
            Score: {{ ((ticket.state.candidates[ticket.state.candidate_idx]?.score || 0) * 100).toFixed(0) }}%
          </p>
          <p class="text-xs font-mono text-gray-400 mt-1">
            {{ ticket.state.info_hash }}
          </p>
        </div>

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

        <!-- Failover candidates remaining -->
        <div v-if="ticket.state.candidates && ticket.state.candidates.length > ticket.state.candidate_idx + 1">
          <button
            @click="showFailoverCandidates = !showFailoverCandidates"
            class="text-sm text-blue-700 hover:text-blue-900 flex items-center gap-1"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              class="h-4 w-4 transition-transform"
              :class="{ 'rotate-90': showFailoverCandidates }"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
            >
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
            </svg>
            {{ ticket.state.candidates.length - ticket.state.candidate_idx - 1 }} backup candidate{{ ticket.state.candidates.length - ticket.state.candidate_idx - 1 > 1 ? 's' : '' }} remaining
          </button>
          <div v-if="showFailoverCandidates" class="mt-2 space-y-2">
            <div
              v-for="(candidate, idx) in ticket.state.candidates.slice(ticket.state.candidate_idx + 1)"
              :key="candidate.info_hash"
              class="bg-white rounded-lg p-2 border border-gray-200 text-sm"
            >
              <p class="font-medium truncate" :title="candidate.title">
                <span class="text-gray-400 mr-1">#{{ ticket.state.candidate_idx + idx + 2 }}</span>
                {{ candidate.title }}
              </p>
              <p class="text-xs text-gray-500">
                {{ formatBytes(candidate.size_bytes) }} ·
                Score: {{ (candidate.score * 100).toFixed(0) }}%
              </p>
            </div>
          </div>
        </div>

        <!-- Already tried candidates -->
        <div v-if="ticket.state.candidate_idx > 0" class="text-sm text-orange-600">
          {{ ticket.state.candidate_idx }} candidate{{ ticket.state.candidate_idx > 1 ? 's' : '' }} already tried (failed or stalled)
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

    <!-- Action Buttons -->
    <div class="card border-gray-200">
      <h3 class="text-lg font-semibold mb-4">Actions</h3>

      <div v-if="actionError" class="mb-4 p-3 bg-red-100 text-red-700 rounded text-sm">
        {{ actionError }}
      </div>

      <div class="flex flex-wrap gap-3">
        <button
          v-if="canRetry"
          @click="handleRetry"
          :disabled="retryLoading"
          class="btn-primary flex items-center gap-2"
        >
          <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
          </svg>
          {{ retryLoading ? 'Retrying...' : 'Retry' }}
        </button>
        <button v-if="canCancel" @click="emit('cancel')" class="btn-danger">
          Cancel Ticket
        </button>
        <button
          @click="emit('showDelete')"
          class="px-4 py-2 border border-red-300 text-red-600 rounded-lg hover:bg-red-50 transition-colors flex items-center gap-2"
        >
          <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
          </svg>
          Delete Permanently
        </button>
      </div>
    </div>

  </div>
</template>
