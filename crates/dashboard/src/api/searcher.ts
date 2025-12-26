import { get, post, patch } from './client'
import type {
  SearchRequest,
  SearchResponse,
  SearcherStatusResponse,
  IndexersResponse,
  IndexerStatus,
  UpdateIndexerRequest,
} from './types'

export async function search(request: SearchRequest): Promise<SearchResponse> {
  return post<SearchResponse>('/search', request)
}

export async function getSearcherStatus(): Promise<SearcherStatusResponse> {
  return get<SearcherStatusResponse>('/searcher/status')
}

export async function getIndexers(): Promise<IndexersResponse> {
  return get<IndexersResponse>('/searcher/indexers')
}

export async function updateIndexer(
  name: string,
  request: UpdateIndexerRequest
): Promise<IndexerStatus> {
  return patch<IndexerStatus>(`/searcher/indexers/${encodeURIComponent(name)}`, request)
}
