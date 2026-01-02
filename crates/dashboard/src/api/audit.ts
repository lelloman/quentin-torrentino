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
  'ticket_deleted',
  'search_executed',
  'search_started',
  'search_completed',
  'indexer_rate_limit_updated',
  'indexer_enabled_changed',
  'torrent_added',
  'torrent_removed',
  'torrent_paused',
  'torrent_resumed',
  'torrent_limit_changed',
  'torrent_rechecked',
  'acquisition_started',
  'acquisition_completed',
  'query_building_started',
  'query_building_completed',
  'scoring_started',
  'scoring_completed',
  'queries_generated',
  'candidates_scored',
  'candidate_selected',
  'training_query_context',
  'training_scoring_context',
  'training_file_mapping_context',
  'user_correction',
]

// Group event types by category for the UI
export const eventTypeCategories = {
  system: ['service_started', 'service_stopped'] as AuditEventType[],
  ticket: ['ticket_created', 'ticket_state_changed', 'ticket_cancelled', 'ticket_deleted'] as AuditEventType[],
  search: ['search_executed', 'search_started', 'search_completed', 'indexer_rate_limit_updated', 'indexer_enabled_changed'] as AuditEventType[],
  torrent: [
    'torrent_added',
    'torrent_removed',
    'torrent_paused',
    'torrent_resumed',
    'torrent_limit_changed',
    'torrent_rechecked',
  ] as AuditEventType[],
  acquisition: [
    'acquisition_started',
    'acquisition_completed',
    'query_building_started',
    'query_building_completed',
    'scoring_started',
    'scoring_completed',
  ] as AuditEventType[],
  textbrain: ['queries_generated', 'candidates_scored', 'candidate_selected'] as AuditEventType[],
  training: ['training_query_context', 'training_scoring_context', 'training_file_mapping_context', 'user_correction'] as AuditEventType[],
}

// Human-readable labels for event types
export const eventTypeLabels: Record<AuditEventType, string> = {
  service_started: 'Service Started',
  service_stopped: 'Service Stopped',
  ticket_created: 'Ticket Created',
  ticket_state_changed: 'State Changed',
  ticket_cancelled: 'Ticket Cancelled',
  ticket_deleted: 'Ticket Deleted',
  search_executed: 'Search Executed',
  search_started: 'Search Started',
  search_completed: 'Search Completed',
  indexer_rate_limit_updated: 'Indexer Rate Limit Updated',
  indexer_enabled_changed: 'Indexer Enabled Changed',
  torrent_added: 'Torrent Added',
  torrent_removed: 'Torrent Removed',
  torrent_paused: 'Torrent Paused',
  torrent_resumed: 'Torrent Resumed',
  torrent_limit_changed: 'Torrent Limit Changed',
  torrent_rechecked: 'Torrent Rechecked',
  acquisition_started: 'Acquisition Started',
  acquisition_completed: 'Acquisition Completed',
  query_building_started: 'Query Building Started',
  query_building_completed: 'Query Building Completed',
  scoring_started: 'Scoring Started',
  scoring_completed: 'Scoring Completed',
  queries_generated: 'Queries Generated',
  candidates_scored: 'Candidates Scored',
  candidate_selected: 'Candidate Selected',
  training_query_context: 'Training: Query Context',
  training_scoring_context: 'Training: Scoring Context',
  training_file_mapping_context: 'Training: File Mapping',
  user_correction: 'User Correction',
}
