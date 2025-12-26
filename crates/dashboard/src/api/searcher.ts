import { get, post } from './client'
import type {
  SearchRequest,
  SearchResponse,
  SearcherStatusResponse,
  IndexersResponse,
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
