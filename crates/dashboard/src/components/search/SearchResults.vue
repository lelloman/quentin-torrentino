<script setup lang="ts">
import { ref, computed } from 'vue'
import type { SearchResponse, TorrentCandidate } from '../../api/types'

const props = defineProps<{
  result: SearchResponse
}>()

const emit = defineEmits<{
  copyMagnet: [magnet: string]
  download: [options: { magnet?: string; torrentUrl?: string; title: string }]
}>()

type SortField = 'title' | 'seeders' | 'size' | 'date'
type SortDirection = 'asc' | 'desc'

const sortField = ref<SortField>('seeders')
const sortDirection = ref<SortDirection>('desc')

const sortedCandidates = computed(() => {
  const candidates = [...props.result.candidates]

  candidates.sort((a, b) => {
    let comparison = 0

    switch (sortField.value) {
      case 'title':
        comparison = a.title.localeCompare(b.title)
        break
      case 'seeders':
        comparison = a.seeders - b.seeders
        break
      case 'size':
        comparison = a.size_bytes - b.size_bytes
        break
      case 'date':
        const dateA = a.publish_date ? new Date(a.publish_date).getTime() : 0
        const dateB = b.publish_date ? new Date(b.publish_date).getTime() : 0
        comparison = dateA - dateB
        break
    }

    return sortDirection.value === 'desc' ? -comparison : comparison
  })

  return candidates
})

function toggleSort(field: SortField) {
  if (sortField.value === field) {
    sortDirection.value = sortDirection.value === 'asc' ? 'desc' : 'asc'
  } else {
    sortField.value = field
    sortDirection.value = 'desc'
  }
}

function formatSize(bytes: number): string {
  if (bytes === 0) return 'Unknown'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`
}

function formatDate(dateStr: string | undefined): string {
  if (!dateStr) return 'Unknown'
  return new Date(dateStr).toLocaleDateString()
}

function getMagnet(candidate: TorrentCandidate): string | undefined {
  // First check if any source has a magnet URI
  for (const source of candidate.sources) {
    if (source.magnet_uri) return source.magnet_uri
  }
  // Fallback: construct magnet from info_hash (DHT will find peers)
  if (candidate.info_hash) {
    const encodedTitle = encodeURIComponent(candidate.title)
    return `magnet:?xt=urn:btih:${candidate.info_hash}&dn=${encodedTitle}`
  }
  return undefined
}

function getTorrentUrl(candidate: TorrentCandidate): string | undefined {
  for (const source of candidate.sources) {
    if (source.torrent_url) return source.torrent_url
  }
  return undefined
}

function canDownload(candidate: TorrentCandidate): boolean {
  return !!getMagnet(candidate) || !!getTorrentUrl(candidate)
}

function handleCopyMagnet(candidate: TorrentCandidate) {
  const magnet = getMagnet(candidate)
  if (magnet) {
    emit('copyMagnet', magnet)
  }
}

function handleDownload(candidate: TorrentCandidate) {
  const magnet = getMagnet(candidate)
  const torrentUrl = getTorrentUrl(candidate)
  if (magnet || torrentUrl) {
    emit('download', { magnet, torrentUrl, title: candidate.title })
  }
}

function getSortIcon(field: SortField): string {
  if (sortField.value !== field) return 'i-carbon-arrows-vertical'
  return sortDirection.value === 'asc' ? 'i-carbon-arrow-up' : 'i-carbon-arrow-down'
}
</script>

<template>
  <div class="card">
    <div class="flex items-center justify-between mb-4">
      <h2 class="text-lg font-semibold">
        Results ({{ result.candidates.length }})
        <span class="text-sm font-normal text-gray-500 ml-2">
          <span v-if="result.cache_hits > 0" class="text-purple-600">{{ result.cache_hits }} from cache</span>
          <span v-if="result.cache_hits > 0 && result.external_hits > 0">, </span>
          <span v-if="result.external_hits > 0" class="text-blue-600">{{ result.external_hits }} from external</span>
        </span>
      </h2>
      <span class="text-sm text-gray-500">
        Search took {{ result.duration_ms }}ms
      </span>
    </div>

    <div v-if="result.indexer_errors && Object.keys(result.indexer_errors).length > 0" class="mb-4 p-3 bg-yellow-50 border border-yellow-200 rounded-md">
      <div class="text-sm text-yellow-800">
        <strong>Some indexers had errors:</strong>
        <ul class="list-disc list-inside mt-1">
          <li v-for="(error, indexer) in result.indexer_errors" :key="indexer">
            {{ indexer }}: {{ error }}
          </li>
        </ul>
      </div>
    </div>

    <div v-if="result.candidates.length === 0" class="text-center py-8 text-gray-500">
      No results found
    </div>

    <div v-else class="overflow-x-auto">
      <table class="w-full text-sm">
        <thead class="bg-gray-50 border-b">
          <tr>
            <th class="text-center p-3 font-medium w-12" title="Source: Cache or External">
              Src
            </th>
            <th class="text-left p-3 font-medium">
              <button
                @click="toggleSort('title')"
                class="flex items-center gap-1 hover:text-primary"
              >
                Title
                <span :class="getSortIcon('title')" class="text-xs"></span>
              </button>
            </th>
            <th class="text-right p-3 font-medium">
              <button
                @click="toggleSort('seeders')"
                class="flex items-center gap-1 hover:text-primary ml-auto"
              >
                Seeders
                <span :class="getSortIcon('seeders')" class="text-xs"></span>
              </button>
            </th>
            <th class="text-right p-3 font-medium">
              <button
                @click="toggleSort('size')"
                class="flex items-center gap-1 hover:text-primary ml-auto"
              >
                Size
                <span :class="getSortIcon('size')" class="text-xs"></span>
              </button>
            </th>
            <th class="text-right p-3 font-medium">
              <button
                @click="toggleSort('date')"
                class="flex items-center gap-1 hover:text-primary ml-auto"
              >
                Date
                <span :class="getSortIcon('date')" class="text-xs"></span>
              </button>
            </th>
            <th class="text-center p-3 font-medium">Sources</th>
            <th class="text-center p-3 font-medium">Actions</th>
          </tr>
        </thead>
        <tbody class="divide-y">
          <tr
            v-for="candidate in sortedCandidates"
            :key="candidate.info_hash"
            class="hover:bg-gray-50"
          >
            <td class="p-3 text-center">
              <span
                v-if="candidate.from_cache"
                class="inline-flex items-center justify-center w-6 h-6 rounded-full bg-purple-100 text-purple-600"
                title="From cache"
              >
                <span class="i-carbon-data-base text-sm"></span>
              </span>
              <span
                v-else
                class="inline-flex items-center justify-center w-6 h-6 rounded-full bg-blue-100 text-blue-600"
                title="From external search"
              >
                <span class="i-carbon-globe text-sm"></span>
              </span>
            </td>
            <td class="p-3">
              <div class="max-w-md truncate" :title="candidate.title">
                {{ candidate.title }}
              </div>
              <div class="text-xs text-gray-400 font-mono truncate" :title="candidate.info_hash">
                {{ candidate.info_hash }}
              </div>
            </td>
            <td class="p-3 text-right">
              <span class="text-green-600 font-medium">{{ candidate.seeders }}</span>
              <span class="text-gray-400"> / </span>
              <span class="text-red-600">{{ candidate.leechers }}</span>
            </td>
            <td class="p-3 text-right whitespace-nowrap">
              {{ formatSize(candidate.size_bytes) }}
            </td>
            <td class="p-3 text-right whitespace-nowrap">
              {{ formatDate(candidate.publish_date) }}
            </td>
            <td class="p-3 text-center">
              <span class="text-xs text-gray-500">
                {{ candidate.sources.length }} indexer{{ candidate.sources.length === 1 ? '' : 's' }}
              </span>
            </td>
            <td class="p-3 text-center">
              <div v-if="canDownload(candidate)" class="flex items-center justify-center gap-2">
                <button
                  @click="handleDownload(candidate)"
                  class="text-green-600 hover:text-green-700 transition-colors"
                  title="Download"
                >
                  <span class="i-carbon-download text-lg inline-block"></span>
                </button>
                <button
                  v-if="getMagnet(candidate)"
                  @click="handleCopyMagnet(candidate)"
                  class="text-gray-500 hover:text-gray-700 transition-colors"
                  title="Copy Magnet Link"
                >
                  <span class="i-carbon-copy"></span>
                </button>
              </div>
              <span v-else class="text-gray-300">-</span>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>
