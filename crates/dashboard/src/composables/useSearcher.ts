import { ref, computed } from 'vue'
import type { SearchRequest, SearchResponse, IndexerStatus, SearcherStatusResponse } from '../api/types'
import {
  search as apiSearch,
  getSearcherStatus as apiGetSearcherStatus,
  getIndexers as apiGetIndexers,
} from '../api/searcher'

export function useSearcher() {
  const searchResult = ref<SearchResponse | null>(null)
  const indexers = ref<IndexerStatus[]>([])
  const status = ref<SearcherStatusResponse | null>(null)
  const isSearching = ref(false)
  const isLoading = ref(false)
  const error = ref<string | null>(null)

  async function search(request: SearchRequest) {
    isSearching.value = true
    error.value = null
    try {
      searchResult.value = await apiSearch(request)
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Search failed'
      throw e
    } finally {
      isSearching.value = false
    }
  }

  async function fetchStatus() {
    isLoading.value = true
    error.value = null
    try {
      status.value = await apiGetSearcherStatus()
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch status'
    } finally {
      isLoading.value = false
    }
  }

  async function fetchIndexers() {
    isLoading.value = true
    error.value = null
    try {
      const response = await apiGetIndexers()
      indexers.value = response.indexers
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch indexers'
    } finally {
      isLoading.value = false
    }
  }

  const enabledIndexers = computed(() => indexers.value.filter((i) => i.enabled))

  function clearError() {
    error.value = null
  }

  function clearSearch() {
    searchResult.value = null
  }

  return {
    searchResult,
    indexers,
    status,
    isSearching,
    isLoading,
    error,
    enabledIndexers,
    search,
    fetchStatus,
    fetchIndexers,
    clearError,
    clearSearch,
  }
}
