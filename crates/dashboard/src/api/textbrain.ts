import { get, post } from './client'
import type {
  TextBrainBuildQueriesRequest,
  TextBrainBuildQueriesResponse,
  TextBrainScoreRequest,
  TextBrainScoreResponse,
  TextBrainAcquireRequest,
  TextBrainAcquireResponse,
  TextBrainConfigResponse,
  QueryContextWithExpected,
  TorrentCandidate,
} from './types'

/**
 * Get TextBrain configuration
 */
export async function getConfig(): Promise<TextBrainConfigResponse> {
  return get<TextBrainConfigResponse>('/textbrain/config')
}

/**
 * Build search queries from context
 */
export async function buildQueries(
  context: QueryContextWithExpected
): Promise<TextBrainBuildQueriesResponse> {
  const request: TextBrainBuildQueriesRequest = { context }
  return post<TextBrainBuildQueriesResponse, TextBrainBuildQueriesRequest>('/textbrain/queries', request)
}

/**
 * Score candidates against context
 */
export async function scoreCandidates(
  context: QueryContextWithExpected,
  candidates: TorrentCandidate[]
): Promise<TextBrainScoreResponse> {
  const request: TextBrainScoreRequest = { context, candidates }
  return post<TextBrainScoreResponse, TextBrainScoreRequest>('/textbrain/score', request)
}

/**
 * Run full acquisition pipeline for a ticket
 */
export async function acquireForTicket(
  ticketId: string
): Promise<TextBrainAcquireResponse> {
  return post<TextBrainAcquireResponse>(`/textbrain/acquire/${ticketId}`)
}

/**
 * Run full acquisition pipeline with custom context
 */
export async function acquire(
  context: QueryContextWithExpected,
  cacheOnly?: boolean
): Promise<TextBrainAcquireResponse> {
  const request: TextBrainAcquireRequest = {
    description: context.description,
    tags: context.tags,
    expected: context.expected,
    cache_only: cacheOnly,
  }
  return post<TextBrainAcquireResponse, TextBrainAcquireRequest>('/textbrain/acquire', request)
}

/**
 * Preview queries without executing search
 */
export async function previewQueries(
  context: QueryContextWithExpected
): Promise<TextBrainBuildQueriesResponse> {
  return buildQueries(context)
}
