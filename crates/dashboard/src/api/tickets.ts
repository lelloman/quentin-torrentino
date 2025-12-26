import { get, post, del } from './client'
import type {
  Ticket,
  TicketListResponse,
  CreateTicketRequest,
  CancelTicketRequest,
  TicketStateType,
} from './types'

export interface ListTicketsParams {
  state?: TicketStateType
  created_by?: string
  limit?: number
  offset?: number
}

function buildQueryString(params: ListTicketsParams): string {
  const searchParams = new URLSearchParams()
  if (params.state) searchParams.set('state', params.state)
  if (params.created_by) searchParams.set('created_by', params.created_by)
  if (params.limit !== undefined) searchParams.set('limit', String(params.limit))
  if (params.offset !== undefined) searchParams.set('offset', String(params.offset))
  const qs = searchParams.toString()
  return qs ? `?${qs}` : ''
}

export async function listTickets(params: ListTicketsParams = {}): Promise<TicketListResponse> {
  return get<TicketListResponse>(`/tickets${buildQueryString(params)}`)
}

export async function getTicket(id: string): Promise<Ticket> {
  return get<Ticket>(`/tickets/${id}`)
}

export async function createTicket(request: CreateTicketRequest): Promise<Ticket> {
  return post<Ticket>('/tickets', request)
}

export async function cancelTicket(id: string, request?: CancelTicketRequest): Promise<Ticket> {
  return del<Ticket>(`/tickets/${id}`, request)
}
