import { get } from './client'
import type { AuditQueryParams, AuditQueryResponse, AuditEventType } from './types'

function buildQueryString(params: AuditQueryParams): string {
  const searchParams = new URLSearchParams()
  if (params.ticket_id) searchParams.set('ticket_id', params.ticket_id)
  if (params.event_type) searchParams.set('event_type', params.event_type)
  if (params.user_id) searchParams.set('user_id', params.user_id)
  if (params.from) searchParams.set('from', params.from)
  if (params.to) searchParams.set('to', params.to)
  if (params.limit !== undefined) searchParams.set('limit', String(params.limit))
  if (params.offset !== undefined) searchParams.set('offset', String(params.offset))
  const qs = searchParams.toString()
  return qs ? `?${qs}` : ''
}

export async function queryAudit(params: AuditQueryParams = {}): Promise<AuditQueryResponse> {
  return get<AuditQueryResponse>(`/audit${buildQueryString(params)}`)
}

// Helper to get all event types for filtering UI
export const allEventTypes: AuditEventType[] = [
  'service_started',
  'service_stopped',
  'ticket_created',
  'ticket_state_changed',
  'ticket_cancelled',
  'search_executed',
  'indexer_rate_limit_updated',
  'indexer_enabled_changed',
  'torrent_added',
  'torrent_removed',
  'torrent_paused',
  'torrent_resumed',
  'torrent_limit_changed',
  'torrent_rechecked',
  'queries_generated',
  'candidates_scored',
  'candidate_selected',
]

// Group event types by category for the UI
export const eventTypeCategories = {
  system: ['service_started', 'service_stopped'] as AuditEventType[],
  ticket: ['ticket_created', 'ticket_state_changed', 'ticket_cancelled'] as AuditEventType[],
  search: ['search_executed', 'indexer_rate_limit_updated', 'indexer_enabled_changed'] as AuditEventType[],
  torrent: [
    'torrent_added',
    'torrent_removed',
    'torrent_paused',
    'torrent_resumed',
    'torrent_limit_changed',
    'torrent_rechecked',
  ] as AuditEventType[],
  textbrain: ['queries_generated', 'candidates_scored', 'candidate_selected'] as AuditEventType[],
}

// Human-readable labels for event types
export const eventTypeLabels: Record<AuditEventType, string> = {
  service_started: 'Service Started',
  service_stopped: 'Service Stopped',
  ticket_created: 'Ticket Created',
  ticket_state_changed: 'Ticket State Changed',
  ticket_cancelled: 'Ticket Cancelled',
  search_executed: 'Search Executed',
  indexer_rate_limit_updated: 'Indexer Rate Limit Updated',
  indexer_enabled_changed: 'Indexer Enabled Changed',
  torrent_added: 'Torrent Added',
  torrent_removed: 'Torrent Removed',
  torrent_paused: 'Torrent Paused',
  torrent_resumed: 'Torrent Resumed',
  torrent_limit_changed: 'Torrent Limit Changed',
  torrent_rechecked: 'Torrent Rechecked',
  queries_generated: 'Queries Generated',
  candidates_scored: 'Candidates Scored',
  candidate_selected: 'Candidate Selected',
}
