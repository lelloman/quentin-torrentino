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

// Acquisition phase - tagged enum from Rust with #[serde(tag = "phase")]
export type AcquisitionPhase =
  | { phase: 'query_building' }
  | { phase: 'searching'; query: string }
  | { phase: 'scoring'; candidates_count: number }

// Scored candidate summary (for NeedsApproval state)
export interface ScoredCandidateSummaryState {
  title: string
  info_hash: string
  size_bytes: number
  seeders: number
  score: number
  reasoning: string
}

// Selected candidate (for Approved/Downloading states)
export interface SelectedCandidateState {
  title: string
  info_hash: string
  magnet_uri: string
  size_bytes: number
  score: number
}

// Completion stats
export interface CompletionStats {
  total_download_bytes: number
  download_duration_secs: number
  conversion_duration_secs: number
  final_size_bytes: number
  files_placed: number
}

// TicketState uses discriminated union with 'type' field
export type TicketState =
  | { type: 'pending' }
  | {
      type: 'acquiring'
      started_at: string
      queries_tried: string[]
      candidates_found: number
      phase: AcquisitionPhase
    }
  | {
      type: 'acquisition_failed'
      queries_tried: string[]
      candidates_seen: number
      reason: string
      failed_at: string
    }
  | {
      type: 'needs_approval'
      candidates: ScoredCandidateSummaryState[]
      recommended_idx: number
      confidence: number
      waiting_since: string
    }
  | {
      type: 'auto_approved'
      selected: SelectedCandidateState
      candidates: SelectedCandidateState[]
      confidence: number
      approved_at: string
    }
  | {
      type: 'approved'
      selected: SelectedCandidateState
      candidates: SelectedCandidateState[]
      approved_by: string
      approved_at: string
    }
  | {
      type: 'rejected'
      rejected_by: string
      reason: string | null
      rejected_at: string
    }
  | {
      type: 'downloading'
      info_hash: string
      progress_pct: number
      speed_bps: number
      eta_secs: number | null
      started_at: string
      candidate_idx: number
      failover_round: number
    }
  | {
      type: 'converting'
      current_idx: number
      total: number
      current_name: string
      started_at: string
    }
  | {
      type: 'placing'
      files_placed: number
      total_files: number
      started_at: string
    }
  | {
      type: 'completed'
      completed_at: string
      stats?: CompletionStats
    }
  | {
      type: 'failed'
      error: string
      retryable?: boolean
      retry_count?: number
      failed_at: string
    }
  | {
      type: 'cancelled'
      cancelled_by: string
      reason: string | null
      cancelled_at: string
    }

export interface Ticket {
  id: string
  created_at: string
  created_by: string
  state: TicketState
  priority: number
  query_context: QueryContext
  dest_path: string
  output_constraints?: OutputConstraints
  updated_at: string
}

export interface TicketListResponse {
  tickets: Ticket[]
  total: number
  limit: number
  offset: number
}

// Audio format options
export type AudioFormat =
  | 'flac'
  | 'mp3'
  | 'aac'
  | 'ogg_vorbis'
  | 'opus'
  | 'wav'
  | 'alac'

// Video codec options
export type VideoCodec = 'h264' | 'h265' | 'vp9' | 'av1'

// Video container options
export type VideoContainer = 'mkv' | 'mp4' | 'webm'

// Audio constraints for conversion
export interface AudioConstraints {
  format: AudioFormat
  bitrate_kbps?: number
  sample_rate_hz?: number
  channels?: number
  compression_level?: number
}

// Video constraints for conversion
export interface VideoConstraints {
  codec: VideoCodec
  container: VideoContainer
  crf?: number
  bitrate_kbps?: number
  width?: number
  height?: number
  fps?: number
  preset?: string
  audio?: AudioConstraints
}

// Output constraints - what format to convert to (or keep original)
export type OutputConstraints =
  | { type: 'original' }
  | ({ type: 'audio' } & AudioConstraints)
  | ({ type: 'video' } & VideoConstraints)

export interface CreateTicketRequest {
  priority?: number
  query_context: {
    tags: string[]
    description: string
  }
  dest_path: string
  output_constraints?: OutputConstraints
}

export interface CancelTicketRequest {
  reason?: string
}

export interface ApiError {
  error: string
}

export type TicketStateType =
  | 'pending'
  | 'acquiring'
  | 'acquisition_failed'
  | 'needs_approval'
  | 'auto_approved'
  | 'approved'
  | 'rejected'
  | 'downloading'
  | 'converting'
  | 'placing'
  | 'completed'
  | 'failed'
  | 'cancelled'

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
  | 'ticket_deleted'
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
  | 'training_query_context'
  | 'training_scoring_context'
  | 'training_file_mapping_context'
  | 'user_correction'

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
  | {
      type: 'ticket_deleted'
      ticket_id: string
      deleted_by: string
      hard_delete: boolean
    }
  | {
      type: 'training_query_context'
      sample_id: string
      ticket_id: string
      input_tags?: string[]
      input_description: string
      input_expected?: string
      output_queries: string[]
      method: string
      confidence: number
      success?: boolean
    }
  | {
      type: 'training_scoring_context'
      sample_id: string
      ticket_id: string
      input_description: string
      input_expected?: string
      input_candidates: Array<{
        title: string
        hash: string
        size_bytes: number
        seeders: number
        category?: string
      }>
      output_recommended_idx: number
      output_scores: number[]
      method: string
    }
  | {
      type: 'training_file_mapping_context'
      sample_id: string
      ticket_id: string
      torrent_title: string
      torrent_hash: string
      expected_content: string
      files: string[]
      mappings: Array<{ file_idx: number; track_idx?: number; role?: string }>
      quality_score: number
    }
  | {
      type: 'user_correction'
      ticket_id: string
      correction_type: string
      original_value: string
      corrected_value: string
      user_id: string
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

// TextBrain types

export interface ExpectedTrack {
  number: number
  title: string
  duration_secs?: number
}

export type ExpectedContent =
  | {
      type: 'album'
      artist?: string
      title: string
      tracks: ExpectedTrack[]
    }
  | {
      type: 'track'
      artist?: string
      title: string
    }
  | {
      type: 'movie'
      title: string
      year?: number
    }
  | {
      type: 'tv_episode'
      series: string
      season: number
      episodes: number[]
    }

export interface QueryContextWithExpected extends QueryContext {
  expected?: ExpectedContent
}

export interface LlmUsage {
  input_tokens: number
  output_tokens: number
  model: string
}

export interface QueryBuildResult {
  queries: string[]
  method: string
  confidence: number
  llm_usage?: LlmUsage
}

export interface FileMapping {
  torrent_file_path: string
  ticket_item_id: string
  confidence: number
}

export interface ScoredCandidate {
  candidate: TorrentCandidate
  score: number
  reasoning: string
  file_mappings: FileMapping[]
}

export interface ScoredCandidateSummary {
  title: string
  info_hash: string
  score: number
  reasoning: string
  file_mapping_count: number
}

export interface MatchResult {
  candidates: ScoredCandidate[]
  method: string
  llm_usage?: LlmUsage
}

export interface AcquisitionResult {
  best_candidate?: ScoredCandidate
  all_candidates: ScoredCandidate[]
  queries_tried: string[]
  candidates_evaluated: number
  query_method: string
  score_method: string
  auto_approved: boolean
  llm_usage?: LlmUsage
  duration_ms: number
}

// TextBrain API request/response types

export interface TextBrainCompleteRequest {
  prompt: string
  max_tokens?: number
  temperature?: number
}

export interface TextBrainCompleteResponse {
  text: string
  usage: LlmUsage
  duration_ms: number
}

export interface TextBrainBuildQueriesRequest {
  context: QueryContextWithExpected
}

export interface TextBrainBuildQueriesResponse {
  result: QueryBuildResult
  duration_ms: number
}

export interface TextBrainScoreRequest {
  context: QueryContextWithExpected
  candidates: TorrentCandidate[]
}

export interface TextBrainScoreResponse {
  result: MatchResult
  duration_ms: number
}

export interface TextBrainAcquireRequest {
  description: string
  tags: string[]
  expected?: ExpectedContent
  auto_approve_threshold?: number
  cache_only?: boolean
}

// Backend response structure (different from AcquisitionResult)
export interface TextBrainAcquireResponse {
  queries_tried: string[]
  candidates_evaluated: number
  candidates: {
    title: string
    info_hash: string
    size_bytes: number
    seeders: number
    score: number
    reasoning: string
  }[]
  best_candidate?: {
    title: string
    info_hash: string
    size_bytes: number
    seeders: number
    score: number
    reasoning: string
  }
  auto_approved: boolean
  query_method: string
  score_method: string
  duration_ms: number
}

export interface TextBrainConfigResponse {
  mode: string
  auto_approve_threshold: number
  llm_configured: boolean
  llm_provider?: string
}
