<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useSearcher } from '../composables/useSearcher'
import { addMagnet, addTorrentFromUrl } from '../api/torrents'
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

const downloadStatus = ref<{ type: 'success' | 'error'; message: string } | null>(null)

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
  downloadStatus.value = { type: 'success', message: 'Magnet link copied to clipboard' }
  setTimeout(() => { downloadStatus.value = null }, 3000)
}

async function handleDownload(options: { magnet?: string; torrentUrl?: string; title: string }) {
  try {
    downloadStatus.value = { type: 'success', message: `Starting download: ${options.title}...` }

    if (options.magnet) {
      await addMagnet({ uri: options.magnet })
    } else if (options.torrentUrl) {
      await addTorrentFromUrl(options.torrentUrl)
    } else {
      throw new Error('No magnet or torrent URL available')
    }

    downloadStatus.value = { type: 'success', message: `Added to downloads: ${options.title}` }
    setTimeout(() => { downloadStatus.value = null }, 5000)
  } catch (e) {
    downloadStatus.value = { type: 'error', message: `Failed to add torrent: ${e instanceof Error ? e.message : 'Unknown error'}` }
  }
}

function clearDownloadStatus() {
  downloadStatus.value = null
}
</script>

<template>
  <div>
    <h1 class="text-2xl font-bold mb-6">Search</h1>

    <!-- Download status toast -->
    <div
      v-if="downloadStatus"
      class="fixed top-4 right-4 z-50 max-w-md p-4 rounded-lg shadow-lg flex items-center gap-3"
      :class="downloadStatus.type === 'success' ? 'bg-green-100 text-green-800 border border-green-200' : 'bg-red-100 text-red-800 border border-red-200'"
    >
      <span :class="downloadStatus.type === 'success' ? 'i-carbon-checkmark-filled text-green-600' : 'i-carbon-warning-filled text-red-600'" class="text-xl flex-shrink-0"></span>
      <span class="flex-1">{{ downloadStatus.message }}</span>
      <button @click="clearDownloadStatus" class="text-gray-500 hover:text-gray-700">
        <span class="i-carbon-close"></span>
      </button>
    </div>

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
      @download="handleDownload"
      class="mt-6"
    />
  </div>
</template>
