import { ref, computed } from 'vue'
import type {
  Ticket,
  TicketListResponse,
  CreateTicketRequest,
  CancelTicketRequest,
  TicketStateType,
} from '../api/types'
import {
  listTickets as apiListTickets,
  getTicket as apiGetTicket,
  createTicket as apiCreateTicket,
  cancelTicket as apiCancelTicket,
  type ListTicketsParams,
} from '../api/tickets'

export function useTickets() {
  const tickets = ref<Ticket[]>([])
  const currentTicket = ref<Ticket | null>(null)
  const total = ref(0)
  const loading = ref(false)
  const error = ref<string | null>(null)

  // Filter state
  const stateFilter = ref<TicketStateType | undefined>(undefined)
  const limit = ref(20)
  const offset = ref(0)

  const hasMore = computed(() => tickets.value.length < total.value)

  async function fetchTickets(append = false) {
    loading.value = true
    error.value = null

    const params: ListTicketsParams = {
      state: stateFilter.value,
      limit: limit.value,
      offset: append ? offset.value : 0,
    }

    try {
      const response: TicketListResponse = await apiListTickets(params)
      if (append) {
        tickets.value = [...tickets.value, ...response.tickets]
      } else {
        tickets.value = response.tickets
        offset.value = 0
      }
      total.value = response.total
      offset.value = response.offset + response.tickets.length
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch tickets'
    } finally {
      loading.value = false
    }
  }

  async function fetchTicket(id: string) {
    loading.value = true
    error.value = null
    currentTicket.value = null

    try {
      currentTicket.value = await apiGetTicket(id)
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch ticket'
    } finally {
      loading.value = false
    }
  }

  async function createTicket(request: CreateTicketRequest): Promise<Ticket | null> {
    loading.value = true
    error.value = null

    try {
      const ticket = await apiCreateTicket(request)
      // Prepend to list if we have tickets loaded
      if (tickets.value.length > 0) {
        tickets.value = [ticket, ...tickets.value]
        total.value++
      }
      return ticket
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to create ticket'
      return null
    } finally {
      loading.value = false
    }
  }

  async function cancelTicket(id: string, request?: CancelTicketRequest): Promise<Ticket | null> {
    loading.value = true
    error.value = null

    try {
      const ticket = await apiCancelTicket(id, request)
      // Update in list
      const index = tickets.value.findIndex((t) => t.id === id)
      if (index !== -1) {
        tickets.value[index] = ticket
      }
      // Update current ticket if it's the one being cancelled
      if (currentTicket.value?.id === id) {
        currentTicket.value = ticket
      }
      return ticket
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to cancel ticket'
      return null
    } finally {
      loading.value = false
    }
  }

  function setStateFilter(state: TicketStateType | undefined) {
    stateFilter.value = state
    offset.value = 0
  }

  function clearError() {
    error.value = null
  }

  return {
    tickets,
    currentTicket,
    total,
    loading,
    error,
    stateFilter,
    hasMore,
    fetchTickets,
    fetchTicket,
    createTicket,
    cancelTicket,
    setStateFilter,
    clearError,
  }
}
