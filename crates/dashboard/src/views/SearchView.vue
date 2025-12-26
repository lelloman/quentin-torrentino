<script setup lang="ts">
import { onMounted } from 'vue'
import { useSearcher } from '../composables/useSearcher'
import SearchForm from '../components/search/SearchForm.vue'
import SearchResults from '../components/search/SearchResults.vue'
import ErrorAlert from '../components/common/ErrorAlert.vue'
import type { SearchRequest } from '../api/types'

const {
  searchResult,
  enabledIndexers,
  isSearching,
  error,
  search,
  fetchIndexers,
  clearError,
} = useSearcher()

onMounted(() => {
  fetchIndexers()
})

async function handleSearch(request: SearchRequest) {
  try {
    await search(request)
  } catch {
    // Error is handled by composable
  }
}

function copyMagnet(magnet: string) {
  navigator.clipboard.writeText(magnet)
  // TODO: Show toast notification
}
</script>

<template>
  <div>
    <h1 class="text-2xl font-bold mb-6">Search</h1>

    <SearchForm
      :indexers="enabledIndexers"
      :is-searching="isSearching"
      @search="handleSearch"
    />

    <ErrorAlert
      v-if="error"
      :message="error"
      @dismiss="clearError"
      class="mt-4"
    />

    <SearchResults
      v-if="searchResult"
      :result="searchResult"
      @copy-magnet="copyMagnet"
      class="mt-6"
    />
  </div>
</template>
