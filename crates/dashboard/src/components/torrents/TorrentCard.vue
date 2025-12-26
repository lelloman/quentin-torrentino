<script setup lang="ts">
import { computed } from 'vue'
import type { TorrentInfo } from '../../api/types'

const props = defineProps<{
  torrent: TorrentInfo
}>()

defineEmits<{
  pause: []
  resume: []
  remove: [deleteFiles: boolean]
}>()

const stateColor = computed(() => {
  switch (props.torrent.state) {
    case 'downloading':
      return 'bg-blue-100 text-blue-800'
    case 'seeding':
      return 'bg-green-100 text-green-800'
    case 'paused':
      return 'bg-yellow-100 text-yellow-800'
    case 'checking':
      return 'bg-purple-100 text-purple-800'
    case 'queued':
      return 'bg-gray-100 text-gray-800'
    case 'stalled':
      return 'bg-orange-100 text-orange-800'
    case 'error':
      return 'bg-red-100 text-red-800'
    default:
      return 'bg-gray-100 text-gray-600'
  }
})

const progressPercent = computed(() => Math.round(props.torrent.progress * 100))

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`
}

function formatSpeed(bytesPerSec: number): string {
  if (bytesPerSec === 0) return '-'
  if (bytesPerSec < 1024) return `${bytesPerSec} B/s`
  if (bytesPerSec < 1024 * 1024) return `${(bytesPerSec / 1024).toFixed(1)} KB/s`
  return `${(bytesPerSec / (1024 * 1024)).toFixed(1)} MB/s`
}

function formatEta(secs?: number): string {
  if (!secs || secs <= 0) return '-'
  if (secs < 60) return `${secs}s`
  if (secs < 3600) return `${Math.floor(secs / 60)}m`
  if (secs < 86400) return `${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m`
  return `${Math.floor(secs / 86400)}d`
}
</script>

<template>
  <div class="bg-white rounded-lg shadow-sm border p-4">
    <div class="flex items-start justify-between mb-2">
      <div class="flex-1 min-w-0">
        <h3 class="font-medium text-gray-900 truncate" :title="torrent.name">
          {{ torrent.name }}
        </h3>
        <p class="text-xs text-gray-400 font-mono truncate">
          {{ torrent.hash }}
        </p>
      </div>
      <span
        :class="['ml-2 px-2 py-1 text-xs font-medium rounded-full', stateColor]"
      >
        {{ torrent.state }}
      </span>
    </div>

    <!-- Progress bar -->
    <div class="mb-3">
      <div class="flex justify-between text-sm text-gray-600 mb-1">
        <span>{{ formatSize(torrent.downloaded_bytes) }} / {{ formatSize(torrent.size_bytes) }}</span>
        <span>{{ progressPercent }}%</span>
      </div>
      <div class="h-2 bg-gray-200 rounded-full overflow-hidden">
        <div
          class="h-full bg-blue-500 transition-all duration-300"
          :style="{ width: `${progressPercent}%` }"
        />
      </div>
    </div>

    <!-- Stats row -->
    <div class="flex items-center text-sm text-gray-600 gap-4 mb-3">
      <span title="Download speed">↓ {{ formatSpeed(torrent.download_speed) }}</span>
      <span title="Upload speed">↑ {{ formatSpeed(torrent.upload_speed) }}</span>
      <span title="Seeds / Peers">S: {{ torrent.seeders }} | L: {{ torrent.leechers }}</span>
      <span title="Ratio">R: {{ torrent.ratio.toFixed(2) }}</span>
      <span v-if="torrent.state === 'downloading'" title="ETA">
        ETA: {{ formatEta(torrent.eta_secs) }}
      </span>
    </div>

    <!-- Actions -->
    <div class="flex items-center gap-2">
      <button
        v-if="torrent.state === 'paused'"
        @click="$emit('resume')"
        class="btn-sm btn-secondary"
      >
        Resume
      </button>
      <button
        v-else-if="torrent.state === 'downloading' || torrent.state === 'seeding'"
        @click="$emit('pause')"
        class="btn-sm btn-secondary"
      >
        Pause
      </button>
      <button
        @click="$emit('remove', false)"
        class="btn-sm text-red-600 hover:bg-red-50"
      >
        Remove
      </button>
      <button
        @click="$emit('remove', true)"
        class="btn-sm text-red-600 hover:bg-red-50"
      >
        Remove + Delete Files
      </button>
    </div>
  </div>
</template>

<style scoped>
.btn-sm {
  @apply px-2 py-1 text-xs rounded border border-gray-300 bg-white hover:bg-gray-50 transition-colors;
}
</style>
