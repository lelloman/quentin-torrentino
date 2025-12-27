import { ref, computed, watch } from 'vue'
import type { AuditRecord, AuditQueryParams, AuditEventType } from '../api/types'
import { queryAudit as apiQueryAudit } from '../api/audit'

export interface AuditFilters {
  ticketId: string
  eventType: AuditEventType | ''
  userId: string
  fromDate: string
  toDate: string
}

export function useAudit() {
  const events = ref<AuditRecord[]>([])
  const total = ref(0)
  const loading = ref(false)
  const error = ref<string | null>(null)

  // Pagination
  const limit = ref(50)
  const offset = ref(0)

  // Filters
  const filters = ref<AuditFilters>({
    ticketId: '',
    eventType: '',
    userId: '',
    fromDate: '',
    toDate: '',
  })

  // Track the starting offset for the current page view
  const pageStartOffset = ref(0)

  const hasMore = computed(() => events.value.length < total.value)
  const currentPage = computed(() => Math.floor(pageStartOffset.value / limit.value) + 1)
  const totalPages = computed(() => Math.ceil(total.value / limit.value))

  function buildParams(customOffset?: number): AuditQueryParams {
    const params: AuditQueryParams = {
      limit: limit.value,
      offset: customOffset ?? offset.value,
    }

    if (filters.value.ticketId) {
      params.ticket_id = filters.value.ticketId
    }
    if (filters.value.eventType) {
      params.event_type = filters.value.eventType
    }
    if (filters.value.userId) {
      params.user_id = filters.value.userId
    }
    if (filters.value.fromDate) {
      params.from = new Date(filters.value.fromDate).toISOString()
    }
    if (filters.value.toDate) {
      // Add time to make it end of day
      params.to = new Date(filters.value.toDate + 'T23:59:59').toISOString()
    }

    return params
  }

  async function fetchEvents(options: { append?: boolean; customOffset?: number } = {}) {
    const { append = false, customOffset } = options
    loading.value = true
    error.value = null

    try {
      const queryOffset = customOffset ?? (append ? offset.value : 0)
      const response = await apiQueryAudit(buildParams(queryOffset))
      if (append) {
        events.value = [...events.value, ...response.events]
      } else {
        events.value = response.events
        pageStartOffset.value = queryOffset
      }
      total.value = response.total
      offset.value = queryOffset + response.events.length
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch audit events'
    } finally {
      loading.value = false
    }
  }

  async function loadMore() {
    if (!loading.value && hasMore.value) {
      await fetchEvents({ append: true })
    }
  }

  async function goToPage(page: number) {
    if (page < 1 || page > totalPages.value) return
    const pageOffset = (page - 1) * limit.value
    await fetchEvents({ customOffset: pageOffset })
  }

  function setFilters(newFilters: Partial<AuditFilters>) {
    filters.value = { ...filters.value, ...newFilters }
    offset.value = 0
    pageStartOffset.value = 0
  }

  function clearFilters() {
    filters.value = {
      ticketId: '',
      eventType: '',
      userId: '',
      fromDate: '',
      toDate: '',
    }
    offset.value = 0
    pageStartOffset.value = 0
  }

  function clearError() {
    error.value = null
  }

  // Auto-refresh when filters change
  watch(
    filters,
    () => {
      fetchEvents()
    },
    { deep: true }
  )

  return {
    events,
    total,
    loading,
    error,
    filters,
    limit,
    hasMore,
    currentPage,
    totalPages,
    fetchEvents,
    loadMore,
    goToPage,
    setFilters,
    clearFilters,
    clearError,
  }
}
