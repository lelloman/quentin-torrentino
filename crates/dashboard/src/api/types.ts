// API response types matching backend structures

export interface HealthResponse {
  status: string
}

export interface SanitizedConfig {
  auth: {
    method: string
  }
  server: {
    host: string
    port: number
  }
  database: {
    path: string
  }
}

export interface QueryContext {
  tags: string[]
  description: string
}

// TicketState uses discriminated union with 'type' field
export type TicketState =
  | { type: 'pending' }
  | {
      type: 'cancelled'
      cancelled_by: string
      reason: string | null
      cancelled_at: string
    }
  | {
      type: 'completed'
      completed_at: string
    }
  | {
      type: 'failed'
      error: string
      failed_at: string
    }

export interface Ticket {
  id: string
  created_at: string
  created_by: string
  state: TicketState
  priority: number
  query_context: QueryContext
  dest_path: string
  updated_at: string
}

export interface TicketListResponse {
  tickets: Ticket[]
  total: number
  limit: number
  offset: number
}

export interface CreateTicketRequest {
  priority?: number
  query_context: {
    tags: string[]
    description: string
  }
  dest_path: string
}

export interface CancelTicketRequest {
  reason?: string
}

export interface ApiError {
  error: string
}

export type TicketStateType = 'pending' | 'cancelled' | 'completed' | 'failed'

// Search types

export type SearchCategory = 'audio' | 'music' | 'movies' | 'tv' | 'books' | 'software' | 'other'

export type SearchMode = 'cache_only' | 'external_only' | 'both'

export interface SearchRequest {
  query: string
  indexers?: string[]
  categories?: SearchCategory[]
  limit?: number
  mode?: SearchMode
}

export interface SearchQueryResponse {
  query: string
  indexers?: string[]
  categories?: SearchCategory[]
  limit?: number
}

export interface TorrentSource {
  indexer: string
  magnet_uri?: string
  torrent_url?: string
  seeders: number
  leechers: number
  details_url?: string
}

export interface TorrentFile {
  path: string
  size_bytes: number
}

export interface TorrentCandidate {
  title: string
  info_hash: string
  size_bytes: number
  seeders: number
  leechers: number
  category?: string
  publish_date?: string
  files?: TorrentFile[]
  sources: TorrentSource[]
  from_cache: boolean
}

export interface SearchResponse {
  query: SearchQueryResponse
  candidates: TorrentCandidate[]
  duration_ms: number
  indexer_errors?: Record<string, string>
  cache_hits: number
  external_hits: number
}

// Indexer status (read-only, configured in Jackett)
export interface IndexerStatus {
  name: string
  enabled: boolean
}

export interface IndexersResponse {
  indexers: IndexerStatus[]
}

export interface SearcherStatusResponse {
  backend: string
  configured: boolean
  indexers_count: number
  indexers_enabled: number
}

// Torrent client types

export type TorrentState =
  | 'downloading'
  | 'seeding'
  | 'paused'
  | 'checking'
  | 'queued'
  | 'stalled'
  | 'error'
  | 'unknown'

export interface TorrentInfo {
  hash: string
  name: string
  state: TorrentState
  progress: number
  size_bytes: number
  downloaded_bytes: number
  uploaded_bytes: number
  download_speed: number
  upload_speed: number
  seeders: number
  leechers: number
  ratio: number
  eta_secs?: number
  added_at?: string
  completed_at?: string
  save_path?: string
  category?: string
  upload_limit: number
  download_limit: number
}

export interface TorrentListResponse {
  torrents: TorrentInfo[]
  count: number
}

export interface TorrentFilterParams {
  state?: TorrentState
  category?: string
  search?: string
}

export interface AddMagnetRequest {
  uri: string
  download_path?: string
  category?: string
  paused?: boolean
  ticket_id?: string
}

export interface AddFromUrlRequest {
  url: string
  download_path?: string
  category?: string
  paused?: boolean
  ticket_id?: string
}

export interface AddTorrentResponse {
  hash: string
  name?: string
}

export interface TorrentClientStatusResponse {
  backend: string
  configured: boolean
}

export interface SetLimitRequest {
  limit: number
}

export interface SuccessResponse {
  message: string
}

// Catalog types (search result cache)

export interface CatalogStats {
  total_torrents: number
  total_files: number
  total_size_bytes: number
  unique_indexers: number
  oldest_entry?: string
  newest_entry?: string
}

// Audit types

export type AuditEventType =
  | 'service_started'
  | 'service_stopped'
  | 'ticket_created'
  | 'ticket_state_changed'
  | 'ticket_cancelled'
  | 'search_executed'
  | 'indexer_rate_limit_updated'
  | 'indexer_enabled_changed'
  | 'torrent_added'
  | 'torrent_removed'
  | 'torrent_paused'
  | 'torrent_resumed'
  | 'torrent_limit_changed'
  | 'torrent_rechecked'
  | 'queries_generated'
  | 'candidates_scored'
  | 'candidate_selected'

// Discriminated union for audit event data
export type AuditEventData =
  | { type: 'service_started'; version: string; config_hash: string }
  | { type: 'service_stopped'; reason: string }
  | {
      type: 'ticket_created'
      ticket_id: string
      requested_by: string
      priority: number
      tags: string[]
      description: string
      dest_path: string
    }
  | {
      type: 'ticket_state_changed'
      ticket_id: string
      from_state: string
      to_state: string
      reason?: string
    }
  | {
      type: 'ticket_cancelled'
      ticket_id: string
      cancelled_by: string
      reason?: string
      previous_state: string
    }
  | {
      type: 'search_executed'
      user_id: string
      searcher: string
      query: string
      indexers_queried: string[]
      results_count: number
      duration_ms: number
      indexer_errors?: Record<string, string>
    }
  | {
      type: 'indexer_rate_limit_updated'
      user_id: string
      indexer: string
      old_rpm: number
      new_rpm: number
    }
  | {
      type: 'indexer_enabled_changed'
      user_id: string
      indexer: string
      enabled: boolean
    }
  | {
      type: 'torrent_added'
      user_id: string
      hash: string
      name?: string
      source: string
      ticket_id?: string
    }
  | {
      type: 'torrent_removed'
      user_id: string
      hash: string
      name: string
      delete_files: boolean
    }
  | {
      type: 'torrent_paused'
      user_id: string
      hash: string
      name: string
    }
  | {
      type: 'torrent_resumed'
      user_id: string
      hash: string
      name: string
    }
  | {
      type: 'torrent_limit_changed'
      user_id: string
      hash: string
      name: string
      limit_type: string
      old_limit: number
      new_limit: number
    }
  | {
      type: 'torrent_rechecked'
      user_id: string
      hash: string
      name: string
    }
  | {
      type: 'queries_generated'
      ticket_id: string
      queries: string[]
      method: string
      llm_input_tokens?: number
      llm_output_tokens?: number
      duration_ms: number
    }
  | {
      type: 'candidates_scored'
      ticket_id: string
      candidates_count: number
      top_candidate_hash?: string
      top_candidate_score?: number
      method: string
      llm_input_tokens?: number
      llm_output_tokens?: number
      duration_ms: number
    }
  | {
      type: 'candidate_selected'
      ticket_id: string
      selected_by: string
      hash: string
      title: string
      score: number
      auto_selected: boolean
    }

export interface AuditRecord {
  id: number
  timestamp: string
  event_type: AuditEventType
  ticket_id?: string
  user_id?: string
  data: AuditEventData
}

export interface AuditQueryParams {
  ticket_id?: string
  event_type?: AuditEventType
  user_id?: string
  from?: string
  to?: string
  limit?: number
  offset?: number
}

export interface AuditQueryResponse {
  events: AuditRecord[]
  total: number
  limit: number
  offset: number
}
