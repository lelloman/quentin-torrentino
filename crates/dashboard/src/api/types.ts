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
