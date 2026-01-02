<script setup lang="ts">
import { onMounted, ref, computed } from 'vue'
import { useRoute, useRouter, RouterLink } from 'vue-router'
import { useAudit } from '../composables/useAudit'
import { eventTypeLabels, eventTypeCategories } from '../api/audit'
import type { AuditRecord, AuditEventType, AuditEventData } from '../api/types'
import ErrorAlert from '../components/common/ErrorAlert.vue'
import LoadingSpinner from '../components/common/LoadingSpinner.vue'

const route = useRoute()
const router = useRouter()

const {
  events,
  total,
  loading,
  error,
  filters,
  hasMore,
  currentPage,
  totalPages,
  fetchEvents,
  loadMore,
  goToPage,
  setFilters,
  clearFilters,
  clearError,
} = useAudit()

// Local filter inputs (to avoid immediate updates)
const localFilters = ref({
  ticketId: '',
  eventType: '' as AuditEventType | '',
  userId: '',
  fromDate: '',
  toDate: '',
})

// Expanded event detail
const expandedEventId = ref<number | null>(null)

// Check for ticket_id in URL query params
onMounted(() => {
  const ticketId = route.query.ticket_id as string | undefined
  if (ticketId) {
    localFilters.value.ticketId = ticketId
    setFilters({ ticketId })
  } else {
    fetchEvents({})
  }
})

function applyFilters() {
  setFilters(localFilters.value)
}

function resetFilters() {
  localFilters.value = {
    ticketId: '',
    eventType: '',
    userId: '',
    fromDate: '',
    toDate: '',
  }
  clearFilters()
  // Remove query params
  router.replace({ query: {} })
}

function toggleEventDetail(id: number) {
  expandedEventId.value = expandedEventId.value === id ? null : id
}

// Format timestamp for display
function formatTimestamp(ts: string): string {
  const date = new Date(ts)
  return date.toLocaleString()
}

// Format relative time
function formatRelativeTime(ts: string): string {
  const now = new Date()
  const date = new Date(ts)
  const diffMs = now.getTime() - date.getTime()
  const diffSec = Math.floor(diffMs / 1000)
  const diffMin = Math.floor(diffSec / 60)
  const diffHour = Math.floor(diffMin / 60)
  const diffDay = Math.floor(diffHour / 24)

  if (diffSec < 60) return 'just now'
  if (diffMin < 60) return `${diffMin}m ago`
  if (diffHour < 24) return `${diffHour}h ago`
  if (diffDay < 7) return `${diffDay}d ago`
  return formatTimestamp(ts)
}

// Format bytes to human readable
function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
}

// Get event type badge color
function getEventTypeColor(eventType: AuditEventType): string {
  if (eventTypeCategories.system.includes(eventType)) return 'bg-gray-500'
  if (eventTypeCategories.ticket.includes(eventType)) return 'bg-blue-500'
  if (eventTypeCategories.search.includes(eventType)) return 'bg-green-500'
  if (eventTypeCategories.torrent.includes(eventType)) return 'bg-purple-500'
  if (eventTypeCategories.acquisition.includes(eventType)) return 'bg-teal-500'
  if (eventTypeCategories.textbrain.includes(eventType)) return 'bg-orange-500'
  if (eventTypeCategories.training.includes(eventType)) return 'bg-pink-500'
  if (eventTypeCategories.pipeline.includes(eventType)) return 'bg-indigo-500'
  return 'bg-gray-500'
}

// Get event icon
function getEventIcon(eventType: AuditEventType): string {
  switch (eventType) {
    case 'service_started':
      return 'i-carbon-power'
    case 'service_stopped':
      return 'i-carbon-power-off'
    case 'ticket_created':
      return 'i-carbon-add-alt'
    case 'ticket_state_changed':
      return 'i-carbon-arrows-horizontal'
    case 'ticket_cancelled':
      return 'i-carbon-close-outline'
    case 'ticket_deleted':
      return 'i-carbon-trash-can'
    case 'search_executed':
    case 'search_started':
    case 'search_completed':
      return 'i-carbon-search'
    case 'indexer_rate_limit_updated':
    case 'indexer_enabled_changed':
      return 'i-carbon-settings-adjust'
    case 'torrent_added':
      return 'i-carbon-download'
    case 'torrent_removed':
      return 'i-carbon-trash-can'
    case 'torrent_paused':
      return 'i-carbon-pause'
    case 'torrent_resumed':
      return 'i-carbon-play'
    case 'torrent_limit_changed':
      return 'i-carbon-meter'
    case 'torrent_rechecked':
      return 'i-carbon-reset'
    case 'acquisition_started':
    case 'acquisition_completed':
      return 'i-carbon-flow'
    case 'query_building_started':
    case 'query_building_completed':
      return 'i-carbon-text-creation'
    case 'scoring_started':
    case 'scoring_completed':
      return 'i-carbon-analytics'
    case 'queries_generated':
      return 'i-carbon-text-creation'
    case 'candidates_scored':
      return 'i-carbon-star'
    case 'candidate_selected':
      return 'i-carbon-checkmark'
    // LLM events
    case 'llm_call_started':
    case 'llm_call_completed':
      return 'i-carbon-chat-bot'
    case 'llm_call_failed':
      return 'i-carbon-warning-alt'
    // Conversion events
    case 'conversion_started':
    case 'conversion_progress':
      return 'i-carbon-transform-binary'
    case 'conversion_completed':
      return 'i-carbon-checkmark-filled'
    case 'conversion_failed':
      return 'i-carbon-error'
    // Placement events
    case 'placement_started':
    case 'placement_progress':
      return 'i-carbon-folder-move-to'
    case 'placement_completed':
      return 'i-carbon-folder-add'
    case 'placement_failed':
      return 'i-carbon-error'
    case 'placement_rolled_back':
      return 'i-carbon-undo'
    default:
      return 'i-carbon-document'
  }
}

// Get event summary text
function getEventSummary(event: AuditRecord): string {
  const data = event.data
  switch (data.type) {
    case 'service_started':
      return `Service started (v${data.version})`
    case 'service_stopped':
      return `Service stopped: ${data.reason}`
    case 'ticket_created':
      return `Ticket created: ${data.description.slice(0, 50)}${data.description.length > 50 ? '...' : ''}`
    case 'ticket_state_changed':
      return `State: ${data.from_state} → ${data.to_state}`
    case 'ticket_cancelled':
      return `Cancelled by ${data.cancelled_by}${data.reason ? `: ${data.reason}` : ''}`
    case 'search_executed':
      return `"${data.query}" - ${data.results_count} results (${data.duration_ms}ms)`
    case 'indexer_rate_limit_updated':
      return `${data.indexer}: ${data.old_rpm} → ${data.new_rpm} RPM`
    case 'indexer_enabled_changed':
      return `${data.indexer}: ${data.enabled ? 'enabled' : 'disabled'}`
    case 'torrent_added':
      return data.name || data.hash.slice(0, 12)
    case 'torrent_removed':
      return `${data.name}${data.delete_files ? ' (files deleted)' : ''}`
    case 'torrent_paused':
    case 'torrent_resumed':
    case 'torrent_rechecked':
      return data.name
    case 'torrent_limit_changed':
      return `${data.name}: ${data.limit_type} ${data.old_limit} → ${data.new_limit}`
    case 'queries_generated':
      return `${data.queries.length} queries via ${data.method} (${data.duration_ms}ms)`
    case 'candidates_scored':
      return `${data.candidates_count} candidates, top score: ${data.top_candidate_score ?? 'N/A'}`
    case 'candidate_selected':
      return `${data.title} (score: ${data.score})${data.auto_selected ? ' [auto]' : ''}`
    case 'training_query_context':
      return `Training data: ${data.output_queries?.length ?? 0} queries generated`
    case 'training_scoring_context':
      return `Training data: ${data.input_candidates?.length ?? 0} candidates scored`
    case 'training_file_mapping_context':
      return `Training data: file mapping for ${data.torrent_title ?? 'unknown'}`
    case 'user_correction':
      return `User correction: ${data.correction_type}`
    case 'ticket_deleted':
      return `Deleted by ${data.deleted_by}${data.hard_delete ? ' (hard delete)' : ''}`
    case 'acquisition_started':
      return `${data.description} (mode: ${data.mode})`
    case 'acquisition_completed':
      return `Completed: ${data.result}`
    case 'query_building_started':
      return `Building queries (method: ${data.method})`
    case 'query_building_completed':
      return `${data.queries?.length ?? 0} queries via ${data.method} (${data.duration_ms}ms)`
    case 'search_started':
      return `"${data.query}" (${data.query_index + 1}/${data.total_queries})`
    case 'search_completed':
      return `"${data.query?.slice(0, 30)}${data.query?.length > 30 ? '...' : ''}" - ${data.candidates_found} results (${data.duration_ms}ms)`
    case 'scoring_started':
      return `Scoring ${data.candidates_count} candidates (${data.method})`
    case 'scoring_completed':
      return `Top score: ${data.top_candidate_score?.toFixed(2) ?? 'N/A'} (${data.candidates_count} candidates, ${data.duration_ms}ms)`
    // LLM events
    case 'llm_call_started':
      return `${data.purpose} via ${data.provider}/${data.model}`
    case 'llm_call_completed':
      return `${data.purpose}: ${data.input_tokens}→${data.output_tokens} tokens (${data.duration_ms}ms)`
    case 'llm_call_failed':
      return `${data.purpose} failed${data.is_timeout ? ' (timeout)' : ''}: ${data.error}`
    // Conversion events
    case 'conversion_started':
      return `Converting ${data.total_files} file(s) → ${data.target_format}`
    case 'conversion_progress':
      return `Converting ${data.current_file} (${data.current_idx + 1}/${data.total_files}, ${data.percent}%)`
    case 'conversion_completed':
      return `Converted ${data.files_converted} file(s): ${data.input_format} → ${data.output_format} (${data.duration_ms}ms)`
    case 'conversion_failed':
      return `Conversion failed${data.failed_file ? ` on ${data.failed_file}` : ''}: ${data.error}`
    // Placement events
    case 'placement_started':
      return `Placing ${data.total_files} file(s) (${formatBytes(data.total_bytes)})`
    case 'placement_progress':
      return `Placing ${data.current_file} (${data.files_placed}/${data.total_files})`
    case 'placement_completed':
      return `Placed ${data.files_placed} file(s) to ${data.dest_dir} (${data.duration_ms}ms)`
    case 'placement_failed':
      return `Placement failed${data.failed_file ? ` on ${data.failed_file}` : ''}: ${data.error}`
    case 'placement_rolled_back':
      return `Rolled back: ${data.files_removed} files, ${data.directories_removed} dirs${data.success ? '' : ' (with errors)'}`
    default:
      return 'Unknown event'
  }
}

// Format event detail data for display
function formatEventDetail(data: AuditEventData): Record<string, unknown> {
  const { type, ...rest } = data
  return rest
}

// Active filter count
const activeFilterCount = computed(() => {
  let count = 0
  if (filters.value.ticketId) count++
  if (filters.value.eventType) count++
  if (filters.value.userId) count++
  if (filters.value.fromDate) count++
  if (filters.value.toDate) count++
  return count
})

// Show filters panel
const showFilters = ref(true)
</script>

<template>
  <div>
    <!-- Header -->
    <div class="flex items-center justify-between mb-6">
      <div class="flex items-center gap-3">
        <h1 class="text-2xl font-bold">Audit Log</h1>
        <span class="text-sm text-gray-500">({{ total }} events)</span>
      </div>
      <div class="flex items-center gap-2">
        <button
          @click="showFilters = !showFilters"
          class="btn-secondary flex items-center gap-2"
        >
          <span class="i-carbon-filter text-lg"></span>
          Filters
          <span
            v-if="activeFilterCount > 0"
            class="bg-primary text-white text-xs px-1.5 py-0.5 rounded-full"
          >
            {{ activeFilterCount }}
          </span>
        </button>
        <button
          @click="fetchEvents({})"
          :disabled="loading"
          class="btn-primary flex items-center gap-2"
        >
          <span class="i-carbon-refresh text-lg" :class="{ 'animate-spin': loading }"></span>
          Refresh
        </button>
      </div>
    </div>

    <ErrorAlert v-if="error" :message="error" @dismiss="clearError" class="mb-4" />

    <!-- Filters Panel -->
    <Transition name="slide">
      <div v-if="showFilters" class="bg-white rounded-lg shadow p-4 mb-6">
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-5 gap-4">
          <!-- Ticket ID -->
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">Ticket ID</label>
            <input
              v-model="localFilters.ticketId"
              type="text"
              placeholder="Filter by ticket..."
              class="input-field w-full"
              @keyup.enter="applyFilters"
            />
          </div>

          <!-- Event Type -->
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">Event Type</label>
            <select v-model="localFilters.eventType" class="input-field w-full">
              <option value="">All types</option>
              <optgroup label="System">
                <option
                  v-for="type in eventTypeCategories.system"
                  :key="type"
                  :value="type"
                >
                  {{ eventTypeLabels[type] }}
                </option>
              </optgroup>
              <optgroup label="Tickets">
                <option
                  v-for="type in eventTypeCategories.ticket"
                  :key="type"
                  :value="type"
                >
                  {{ eventTypeLabels[type] }}
                </option>
              </optgroup>
              <optgroup label="Search">
                <option
                  v-for="type in eventTypeCategories.search"
                  :key="type"
                  :value="type"
                >
                  {{ eventTypeLabels[type] }}
                </option>
              </optgroup>
              <optgroup label="Torrents">
                <option
                  v-for="type in eventTypeCategories.torrent"
                  :key="type"
                  :value="type"
                >
                  {{ eventTypeLabels[type] }}
                </option>
              </optgroup>
              <optgroup label="Acquisition">
                <option
                  v-for="type in eventTypeCategories.acquisition"
                  :key="type"
                  :value="type"
                >
                  {{ eventTypeLabels[type] }}
                </option>
              </optgroup>
              <optgroup label="TextBrain">
                <option
                  v-for="type in eventTypeCategories.textbrain"
                  :key="type"
                  :value="type"
                >
                  {{ eventTypeLabels[type] }}
                </option>
              </optgroup>
              <optgroup label="Training">
                <option
                  v-for="type in eventTypeCategories.training"
                  :key="type"
                  :value="type"
                >
                  {{ eventTypeLabels[type] }}
                </option>
              </optgroup>
              <optgroup label="Pipeline">
                <option
                  v-for="type in eventTypeCategories.pipeline"
                  :key="type"
                  :value="type"
                >
                  {{ eventTypeLabels[type] }}
                </option>
              </optgroup>
            </select>
          </div>

          <!-- User ID -->
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">User ID</label>
            <input
              v-model="localFilters.userId"
              type="text"
              placeholder="Filter by user..."
              class="input-field w-full"
              @keyup.enter="applyFilters"
            />
          </div>

          <!-- From Date -->
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">From Date</label>
            <input v-model="localFilters.fromDate" type="date" class="input-field w-full" />
          </div>

          <!-- To Date -->
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">To Date</label>
            <input v-model="localFilters.toDate" type="date" class="input-field w-full" />
          </div>
        </div>

        <div class="flex justify-end gap-2 mt-4">
          <button @click="resetFilters" class="btn-secondary">Reset</button>
          <button @click="applyFilters" class="btn-primary">Apply Filters</button>
        </div>
      </div>
    </Transition>

    <!-- Events List -->
    <div class="bg-white rounded-lg shadow overflow-hidden">
      <!-- Loading overlay -->
      <div v-if="loading && events.length === 0" class="p-8 text-center">
        <LoadingSpinner class="mx-auto mb-2" />
        <p class="text-gray-500">Loading audit events...</p>
      </div>

      <!-- Empty state -->
      <div v-else-if="events.length === 0" class="p-8 text-center">
        <span class="i-carbon-document-blank text-4xl text-gray-300 mb-2"></span>
        <p class="text-gray-500">No audit events found</p>
        <p v-if="activeFilterCount > 0" class="text-sm text-gray-400 mt-1">
          Try adjusting your filters
        </p>
      </div>

      <!-- Events table -->
      <div v-else class="divide-y divide-gray-200">
        <div
          v-for="event in events"
          :key="event.id"
          class="hover:bg-gray-50 transition-colors"
        >
          <!-- Event row -->
          <div
            class="flex items-center gap-4 p-4 cursor-pointer"
            @click="toggleEventDetail(event.id)"
          >
            <!-- Icon -->
            <div
              class="w-10 h-10 rounded-full flex items-center justify-center text-white"
              :class="getEventTypeColor(event.event_type)"
            >
              <span :class="getEventIcon(event.event_type)" class="text-lg"></span>
            </div>

            <!-- Content -->
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-2 mb-1">
                <span
                  class="text-xs font-medium px-2 py-0.5 rounded text-white"
                  :class="getEventTypeColor(event.event_type)"
                >
                  {{ eventTypeLabels[event.event_type] }}
                </span>
                <RouterLink
                  v-if="event.ticket_id"
                  :to="{ name: 'ticket-detail', params: { id: event.ticket_id } }"
                  class="text-xs text-blue-600 hover:underline"
                  @click.stop
                >
                  {{ event.ticket_id }}
                </RouterLink>
                <span v-if="event.user_id" class="text-xs text-gray-500">
                  by {{ event.user_id }}
                </span>
              </div>
              <p class="text-sm text-gray-900 truncate">
                {{ getEventSummary(event) }}
              </p>
            </div>

            <!-- Timestamp -->
            <div class="text-right flex-shrink-0">
              <p class="text-sm text-gray-500" :title="formatTimestamp(event.timestamp)">
                {{ formatRelativeTime(event.timestamp) }}
              </p>
              <p class="text-xs text-gray-400">
                #{{ event.id }}
              </p>
            </div>

            <!-- Expand indicator -->
            <span
              class="i-carbon-chevron-down text-gray-400 transition-transform"
              :class="{ 'rotate-180': expandedEventId === event.id }"
            ></span>
          </div>

          <!-- Expanded detail -->
          <Transition name="expand">
            <div
              v-if="expandedEventId === event.id"
              class="bg-gray-50 border-t border-gray-200 px-4 py-3"
            >
              <div class="grid grid-cols-2 gap-4 text-sm">
                <div>
                  <span class="text-gray-500">Event ID:</span>
                  <span class="ml-2 font-mono">{{ event.id }}</span>
                </div>
                <div>
                  <span class="text-gray-500">Timestamp:</span>
                  <span class="ml-2 font-mono">{{ formatTimestamp(event.timestamp) }}</span>
                </div>
                <div v-if="event.ticket_id">
                  <span class="text-gray-500">Ticket:</span>
                  <RouterLink
                    :to="{ name: 'ticket-detail', params: { id: event.ticket_id } }"
                    class="ml-2 text-blue-600 hover:underline"
                  >
                    {{ event.ticket_id }}
                  </RouterLink>
                </div>
                <div v-if="event.user_id">
                  <span class="text-gray-500">User:</span>
                  <span class="ml-2">{{ event.user_id }}</span>
                </div>
              </div>

              <!-- Event data -->
              <div class="mt-3">
                <p class="text-gray-500 text-sm mb-1">Event Data:</p>
                <pre class="bg-gray-900 text-green-400 p-3 rounded text-xs overflow-x-auto">{{ JSON.stringify(formatEventDetail(event.data), null, 2) }}</pre>
              </div>
            </div>
          </Transition>
        </div>
      </div>

      <!-- Pagination -->
      <div
        v-if="events.length > 0"
        class="border-t border-gray-200 px-4 py-3 flex items-center justify-between"
      >
        <div class="text-sm text-gray-500">
          Showing {{ events.length }} of {{ total }} events
        </div>

        <div class="flex items-center gap-2">
          <!-- Page buttons -->
          <button
            @click="goToPage(currentPage - 1)"
            :disabled="currentPage <= 1 || loading"
            class="btn-secondary text-sm disabled:opacity-50"
          >
            <span class="i-carbon-chevron-left"></span>
          </button>

          <span class="text-sm text-gray-600">
            Page {{ currentPage }} of {{ totalPages }}
          </span>

          <button
            @click="goToPage(currentPage + 1)"
            :disabled="currentPage >= totalPages || loading"
            class="btn-secondary text-sm disabled:opacity-50"
          >
            <span class="i-carbon-chevron-right"></span>
          </button>

          <!-- Load more -->
          <button
            v-if="hasMore"
            @click="loadMore"
            :disabled="loading"
            class="btn-primary text-sm ml-2"
          >
            <LoadingSpinner v-if="loading" class="w-4 h-4" />
            <span v-else>Load More</span>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.slide-enter-active,
.slide-leave-active {
  transition: all 0.2s ease;
}

.slide-enter-from,
.slide-leave-to {
  opacity: 0;
  transform: translateY(-10px);
}

.expand-enter-active,
.expand-leave-active {
  transition: all 0.2s ease;
}

.expand-enter-from,
.expand-leave-to {
  opacity: 0;
  max-height: 0;
}

.expand-enter-to,
.expand-leave-from {
  max-height: 500px;
}

.input-field {
  @apply px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-primary focus:border-primary text-sm;
}

.btn-primary {
  @apply px-4 py-2 bg-primary text-white rounded-md hover:bg-primary/90 disabled:opacity-50 transition-colors;
}

.btn-secondary {
  @apply px-4 py-2 bg-gray-100 text-gray-700 rounded-md hover:bg-gray-200 disabled:opacity-50 transition-colors;
}
</style>
