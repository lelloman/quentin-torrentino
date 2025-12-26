<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue'
import { useTorrents } from '../composables/useTorrents'
import TorrentList from '../components/torrents/TorrentList.vue'
import TorrentStateFilter from '../components/torrents/TorrentStateFilter.vue'
import AddTorrentForm from '../components/torrents/AddTorrentForm.vue'
import ErrorAlert from '../components/common/ErrorAlert.vue'
import type { TorrentState } from '../api/types'

const {
  torrents,
  status,
  loading,
  error,
  stateFilter,
  totalDownloadSpeed,
  totalUploadSpeed,
  downloadingCount,
  seedingCount,
  fetchStatus,
  fetchTorrents,
  addMagnet,
  removeTorrent,
  pauseTorrent,
  resumeTorrent,
  setStateFilter,
  clearError,
} = useTorrents()

const showAddForm = ref(false)
let refreshInterval: ReturnType<typeof setInterval> | null = null

onMounted(async () => {
  await fetchStatus()
  if (status.value?.configured) {
    await fetchTorrents()
    // Auto-refresh every 3 seconds
    refreshInterval = setInterval(fetchTorrents, 3000)
  }
})

onUnmounted(() => {
  if (refreshInterval) {
    clearInterval(refreshInterval)
  }
})

watch(stateFilter, () => {
  fetchTorrents()
})

function handleFilterChange(value: TorrentState | undefined) {
  setStateFilter(value)
}

async function handleAddMagnet(uri: string) {
  const hash = await addMagnet({ uri })
  if (hash) {
    showAddForm.value = false
  }
}

async function handlePause(hash: string) {
  await pauseTorrent(hash)
}

async function handleResume(hash: string) {
  await resumeTorrent(hash)
}

async function handleRemove(hash: string, deleteFiles: boolean) {
  if (confirm(`Remove this torrent${deleteFiles ? ' and its files' : ''}?`)) {
    await removeTorrent(hash, deleteFiles)
  }
}

function formatSpeed(bytesPerSec: number): string {
  if (bytesPerSec < 1024) return `${bytesPerSec} B/s`
  if (bytesPerSec < 1024 * 1024) return `${(bytesPerSec / 1024).toFixed(1)} KB/s`
  return `${(bytesPerSec / (1024 * 1024)).toFixed(1)} MB/s`
}
</script>

<template>
  <div>
    <div class="flex items-center justify-between mb-6">
      <div>
        <h1 class="text-2xl font-bold">Torrents</h1>
        <p v-if="status?.configured" class="text-sm text-gray-600 mt-1">
          Backend: {{ status.backend }}
        </p>
      </div>
      <button
        v-if="status?.configured && !showAddForm"
        @click="showAddForm = true"
        class="btn-primary"
      >
        Add Torrent
      </button>
    </div>

    <ErrorAlert
      v-if="error"
      :message="error"
      @dismiss="clearError"
      class="mb-4"
    />

    <div
      v-if="!status?.configured"
      class="text-center py-12 bg-gray-50 rounded-lg"
    >
      <p class="text-gray-500 mb-2">Torrent client not configured</p>
      <p class="text-sm text-gray-400">
        Add a [torrent_client] section to your config.toml
      </p>
    </div>

    <template v-else>
      <!-- Stats bar -->
      <div class="grid grid-cols-4 gap-4 mb-6">
        <div class="bg-white p-4 rounded-lg shadow-sm border">
          <p class="text-sm text-gray-500">Total</p>
          <p class="text-2xl font-semibold">{{ torrents.length }}</p>
        </div>
        <div class="bg-white p-4 rounded-lg shadow-sm border">
          <p class="text-sm text-gray-500">Downloading</p>
          <p class="text-2xl font-semibold text-blue-600">{{ downloadingCount }}</p>
        </div>
        <div class="bg-white p-4 rounded-lg shadow-sm border">
          <p class="text-sm text-gray-500">Seeding</p>
          <p class="text-2xl font-semibold text-green-600">{{ seedingCount }}</p>
        </div>
        <div class="bg-white p-4 rounded-lg shadow-sm border">
          <p class="text-sm text-gray-500">Speed</p>
          <p class="text-sm">
            <span class="text-blue-600">↓ {{ formatSpeed(totalDownloadSpeed) }}</span>
            <span class="mx-1">|</span>
            <span class="text-green-600">↑ {{ formatSpeed(totalUploadSpeed) }}</span>
          </p>
        </div>
      </div>

      <AddTorrentForm
        v-if="showAddForm"
        @submit="handleAddMagnet"
        @cancel="showAddForm = false"
        class="mb-6"
      />

      <div class="flex items-center gap-4 mb-4">
        <label class="text-sm text-gray-600">Filter by state:</label>
        <TorrentStateFilter
          :model-value="stateFilter"
          @update:model-value="handleFilterChange"
        />
      </div>

      <TorrentList
        :torrents="torrents"
        :loading="loading"
        @pause="handlePause"
        @resume="handleResume"
        @remove="handleRemove"
      />
    </template>
  </div>
</template>
