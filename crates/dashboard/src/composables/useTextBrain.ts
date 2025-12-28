import { ref, computed } from 'vue'
import * as textbrainApi from '../api/textbrain'
import type {
  QueryContextWithExpected,
  QueryBuildResult,
  MatchResult,
  AcquisitionResult,
  TorrentCandidate,
  TextBrainConfigResponse,
  ExpectedContent,
} from '../api/types'

// Module-level state (persists across component mounts)
const loading = ref(false)
const error = ref<string | null>(null)
const config = ref<TextBrainConfigResponse | null>(null)
const queryResult = ref<QueryBuildResult | null>(null)
const scoreResult = ref<MatchResult | null>(null)
const acquisitionResult = ref<AcquisitionResult | null>(null)

export function useTextBrain() {

  // Computed
  const isLlmConfigured = computed(() => config.value?.llm_configured ?? false)
  const mode = computed(() => config.value?.mode ?? 'dumb_only')
  const autoApproveThreshold = computed(
    () => config.value?.auto_approve_threshold ?? 0.85
  )

  const generatedQueries = computed(() => queryResult.value?.queries ?? [])
  const queryConfidence = computed(() => queryResult.value?.confidence ?? 0)
  const queryMethod = computed(() => queryResult.value?.method ?? '')

  const scoredCandidates = computed(() => scoreResult.value?.candidates ?? [])
  const topCandidate = computed(() => scoredCandidates.value[0] ?? null)

  const bestCandidate = computed(() => acquisitionResult.value?.best_candidate ?? null)
  const allCandidates = computed(() => acquisitionResult.value?.all_candidates ?? [])
  const isAutoApproved = computed(() => acquisitionResult.value?.auto_approved ?? false)

  // Actions
  async function fetchConfig() {
    loading.value = true
    error.value = null
    try {
      config.value = await textbrainApi.getConfig()
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch config'
      throw e
    } finally {
      loading.value = false
    }
  }

  async function buildQueries(context: QueryContextWithExpected) {
    loading.value = true
    error.value = null
    try {
      const response = await textbrainApi.buildQueries(context)
      queryResult.value = response.result
      return response
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to build queries'
      throw e
    } finally {
      loading.value = false
    }
  }

  async function scoreCandidates(
    context: QueryContextWithExpected,
    candidates: TorrentCandidate[]
  ) {
    loading.value = true
    error.value = null
    try {
      const response = await textbrainApi.scoreCandidates(context, candidates)
      scoreResult.value = response.result
      return response
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to score candidates'
      throw e
    } finally {
      loading.value = false
    }
  }

  async function acquire(
    context: QueryContextWithExpected,
    cacheOnly?: boolean
  ) {
    loading.value = true
    error.value = null
    try {
      const response = await textbrainApi.acquire(context, cacheOnly)
      // Transform API response to AcquisitionResult format
      acquisitionResult.value = {
        best_candidate: response.best_candidate ? {
          candidate: {
            title: response.best_candidate.title,
            info_hash: response.best_candidate.info_hash,
            size_bytes: response.best_candidate.size_bytes,
            seeders: response.best_candidate.seeders,
            leechers: 0,
            sources: [],
            from_cache: false,
          },
          score: response.best_candidate.score,
          reasoning: response.best_candidate.reasoning,
          file_mappings: [],
        } : undefined,
        all_candidates: response.candidates.map(c => ({
          candidate: {
            title: c.title,
            info_hash: c.info_hash,
            size_bytes: c.size_bytes,
            seeders: c.seeders,
            leechers: 0,
            sources: [],
            from_cache: false,
          },
          score: c.score,
          reasoning: c.reasoning,
          file_mappings: [],
        })),
        queries_tried: response.queries_tried,
        candidates_evaluated: response.candidates_evaluated,
        query_method: response.query_method,
        score_method: response.score_method,
        auto_approved: response.auto_approved,
        duration_ms: response.duration_ms,
      }
      return response
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to acquire'
      throw e
    } finally {
      loading.value = false
    }
  }

  async function acquireForTicket(ticketId: string) {
    loading.value = true
    error.value = null
    try {
      const response = await textbrainApi.acquireForTicket(ticketId)
      // Transform API response to AcquisitionResult format
      acquisitionResult.value = {
        best_candidate: response.best_candidate ? {
          candidate: {
            title: response.best_candidate.title,
            info_hash: response.best_candidate.info_hash,
            size_bytes: response.best_candidate.size_bytes,
            seeders: response.best_candidate.seeders,
            leechers: 0,
            sources: [],
            from_cache: false,
          },
          score: response.best_candidate.score,
          reasoning: response.best_candidate.reasoning,
          file_mappings: [],
        } : undefined,
        all_candidates: response.candidates.map(c => ({
          candidate: {
            title: c.title,
            info_hash: c.info_hash,
            size_bytes: c.size_bytes,
            seeders: c.seeders,
            leechers: 0,
            sources: [],
            from_cache: false,
          },
          score: c.score,
          reasoning: c.reasoning,
          file_mappings: [],
        })),
        queries_tried: response.queries_tried,
        candidates_evaluated: response.candidates_evaluated,
        query_method: response.query_method,
        score_method: response.score_method,
        auto_approved: response.auto_approved,
        duration_ms: response.duration_ms,
      }
      return response
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to acquire for ticket'
      throw e
    } finally {
      loading.value = false
    }
  }

  function reset() {
    queryResult.value = null
    scoreResult.value = null
    acquisitionResult.value = null
    error.value = null
  }

  return {
    // State
    loading,
    error,
    config,
    queryResult,
    scoreResult,
    acquisitionResult,

    // Computed
    isLlmConfigured,
    mode,
    autoApproveThreshold,
    generatedQueries,
    queryConfidence,
    queryMethod,
    scoredCandidates,
    topCandidate,
    bestCandidate,
    allCandidates,
    isAutoApproved,

    // Actions
    fetchConfig,
    buildQueries,
    scoreCandidates,
    acquire,
    acquireForTicket,
    reset,
  }
}

// Helper function to format confidence as percentage
export function formatConfidence(confidence: number): string {
  return `${Math.round(confidence * 100)}%`
}

// Helper function to format file size
export function formatFileSize(bytes: number): string {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
}

// Helper to get color class based on score
export function getScoreColorClass(score: number): string {
  if (score >= 0.85) return 'text-green-600'
  if (score >= 0.70) return 'text-yellow-600'
  if (score >= 0.50) return 'text-orange-600'
  return 'text-red-600'
}

// Helper to get expected content description
export function getExpectedContentDescription(expected: ExpectedContent): string {
  switch (expected.type) {
    case 'album':
      const artistPart = expected.artist ? `${expected.artist} - ` : ''
      return `Album: ${artistPart}${expected.title} (${expected.tracks.length} tracks)`
    case 'track':
      const trackArtist = expected.artist ? `${expected.artist} - ` : ''
      return `Track: ${trackArtist}${expected.title}`
    case 'movie':
      const year = expected.year ? ` (${expected.year})` : ''
      return `Movie: ${expected.title}${year}`
    case 'tv_episode':
      const eps = expected.episodes.join(', ')
      return `TV: ${expected.series} S${expected.season.toString().padStart(2, '0')}E${eps}`
    default:
      return 'Unknown content type'
  }
}
