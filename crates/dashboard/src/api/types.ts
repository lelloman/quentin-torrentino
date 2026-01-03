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
  expected?: ExpectedContent
  catalog_reference?: CatalogReference
  search_constraints?: SearchConstraints
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
  torrent_url?: string
  size_bytes: number
  score: number
  file_mappings?: FileMapping[]
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
      candidates: SelectedCandidateState[]
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
    expected?: ExpectedContent
    catalog_reference?: CatalogReference
    search_constraints?: SearchConstraints
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
  | 'search_started'
  | 'search_completed'
  | 'indexer_rate_limit_updated'
  | 'indexer_enabled_changed'
  | 'torrent_added'
  | 'torrent_removed'
  | 'torrent_paused'
  | 'torrent_resumed'
  | 'torrent_limit_changed'
  | 'torrent_rechecked'
  | 'acquisition_started'
  | 'acquisition_completed'
  | 'query_building_started'
  | 'query_building_completed'
  | 'scoring_started'
  | 'scoring_completed'
  | 'queries_generated'
  | 'candidates_scored'
  | 'candidate_selected'
  | 'training_query_context'
  | 'training_scoring_context'
  | 'training_file_mapping_context'
  | 'user_correction'
  // LLM events
  | 'llm_call_started'
  | 'llm_call_completed'
  | 'llm_call_failed'
  // Conversion events (Phase 4)
  | 'conversion_started'
  | 'conversion_progress'
  | 'conversion_completed'
  | 'conversion_failed'
  // Placement events (Phase 4)
  | 'placement_started'
  | 'placement_progress'
  | 'placement_completed'
  | 'placement_failed'
  | 'placement_rolled_back'

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
  | {
      type: 'acquisition_started'
      ticket_id: string
      mode: string
      description: string
    }
  | {
      type: 'acquisition_completed'
      ticket_id: string
      result: string
    }
  | {
      type: 'query_building_started'
      ticket_id: string
      method: string
    }
  | {
      type: 'query_building_completed'
      ticket_id: string
      queries: string[]
      method: string
      duration_ms: number
    }
  | {
      type: 'search_started'
      ticket_id: string
      query: string
      query_index: number
      total_queries: number
    }
  | {
      type: 'search_completed'
      ticket_id: string
      query: string
      candidates_found: number
      duration_ms: number
    }
  | {
      type: 'scoring_started'
      ticket_id: string
      candidates_count: number
      method: string
    }
  | {
      type: 'scoring_completed'
      ticket_id: string
      candidates_count: number
      top_candidate_hash: string
      top_candidate_score: number
      method: string
      duration_ms: number
    }
  // LLM events
  | {
      type: 'llm_call_started'
      ticket_id?: string
      purpose: string
      provider: string
      model: string
    }
  | {
      type: 'llm_call_completed'
      ticket_id?: string
      purpose: string
      input_tokens: number
      output_tokens: number
      duration_ms: number
    }
  | {
      type: 'llm_call_failed'
      ticket_id?: string
      purpose: string
      error: string
      duration_ms: number
      is_timeout: boolean
    }
  // Conversion events (Phase 4)
  | {
      type: 'conversion_started'
      ticket_id: string
      job_id: string
      input_path: string
      output_path: string
      target_format: string
      total_files: number
    }
  | {
      type: 'conversion_progress'
      ticket_id: string
      job_id: string
      current_idx: number
      total_files: number
      current_file: string
      percent: number
    }
  | {
      type: 'conversion_completed'
      ticket_id: string
      job_id: string
      files_converted: number
      output_bytes: number
      duration_ms: number
      input_format: string
      output_format: string
    }
  | {
      type: 'conversion_failed'
      ticket_id: string
      job_id: string
      failed_file?: string
      error: string
      files_completed: number
      retryable: boolean
    }
  // Placement events (Phase 4)
  | {
      type: 'placement_started'
      ticket_id: string
      job_id: string
      total_files: number
      total_bytes: number
    }
  | {
      type: 'placement_progress'
      ticket_id: string
      job_id: string
      files_placed: number
      total_files: number
      bytes_placed: number
      current_file: string
    }
  | {
      type: 'placement_completed'
      ticket_id: string
      job_id: string
      files_placed: number
      total_bytes: number
      duration_ms: number
      dest_dir: string
    }
  | {
      type: 'placement_failed'
      ticket_id: string
      job_id: string
      failed_file?: string
      error: string
      files_completed: number
    }
  | {
      type: 'placement_rolled_back'
      ticket_id: string
      job_id: string
      files_removed: number
      directories_removed: number
      success: boolean
      errors: string[]
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
  duration_ms?: number
  disc_number?: number
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

// =============================================================================
// External Catalog Types (MusicBrainz, TMDB)
// =============================================================================

export interface ExternalCatalogStatus {
  musicbrainz_available: boolean
  tmdb_available: boolean
}

// MusicBrainz types

export interface MusicBrainzTrack {
  position: number
  title: string
  length_ms?: number
  disc_number: number
}

export interface MusicBrainzRelease {
  mbid: string
  title: string
  artist_credit: string
  release_date?: string
  country?: string
  track_count?: number
  total_length_ms?: number
  tracks: MusicBrainzTrack[]
  cover_art_available?: boolean
}

// TMDB types

export interface TmdbMovie {
  id: number
  title: string
  original_title: string
  release_date?: string
  overview: string
  poster_path?: string
  runtime_minutes?: number
  genres: string[]
}

export interface TmdbEpisode {
  episode_number: number
  name: string
  air_date?: string
  runtime_minutes?: number
  overview: string
}

export interface TmdbSeason {
  id: number
  season_number: number
  name: string
  air_date?: string
  episode_count: number
  episodes: TmdbEpisode[]
  poster_path?: string
}

export interface TmdbSeasonSummary {
  season_number: number
  episode_count: number
  air_date?: string
}

export interface TmdbSeries {
  id: number
  name: string
  original_name: string
  first_air_date?: string
  overview: string
  poster_path?: string
  number_of_seasons: number
  number_of_episodes: number
  genres: string[]
  seasons: TmdbSeasonSummary[]
}

// =============================================================================
// Catalog Reference and Search Constraints
// =============================================================================

export type TmdbMediaType = 'movie' | 'tv'

// CatalogReference - stores catalog IDs for validation during scoring
export type CatalogReference =
  | {
      type: 'music_brainz'
      release_id: string
      track_count: number
      total_duration_ms?: number
    }
  | {
      type: 'tmdb'
      id: number
      media_type: TmdbMediaType
      runtime_minutes?: number
      episode_count?: number
    }

// Resolution for video constraints
export type Resolution = 'r720p' | 'r1080p' | 'r2160p'

// Video source quality
export type VideoSource = 'cam' | 'hdtv' | 'web_dl' | 'blu_ray' | 'remux'

// Video codec for constraints (different from conversion codec)
export type VideoSearchCodec = 'x264' | 'x265' | 'av1'

// Audio search constraints
export interface AudioSearchConstraints {
  preferred_formats?: AudioFormat[]
  min_bitrate_kbps?: number
  avoid_compilations?: boolean
  avoid_live?: boolean
}

// Language priority for video constraints
export type LanguagePriority = 'required' | 'preferred'

// Language preference with priority
export interface LanguagePreference {
  code: string
  priority: LanguagePriority
}

// Video search constraints
export interface VideoSearchConstraints {
  min_resolution?: Resolution
  preferred_resolution?: Resolution
  preferred_sources?: VideoSource[]
  preferred_codecs?: VideoSearchCodec[]
  audio_languages?: LanguagePreference[]
  subtitle_languages?: LanguagePreference[]
  exclude_hardcoded_subs?: boolean
}

// Combined search constraints
export interface SearchConstraints {
  audio?: AudioSearchConstraints
  video?: VideoSearchConstraints
}

// Extended QueryContext with catalog reference and constraints
export interface QueryContextWithCatalog extends QueryContext {
  expected?: ExpectedContent
  catalog_reference?: CatalogReference
  search_constraints?: SearchConstraints
}

// Extended CreateTicketRequest with catalog and constraints
export interface CreateTicketWithCatalogRequest {
  priority?: number
  query_context: {
    tags: string[]
    description: string
    expected?: ExpectedContent
    catalog_reference?: CatalogReference
    search_constraints?: SearchConstraints
  }
  dest_path: string
  output_constraints?: OutputConstraints
}
