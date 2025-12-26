<script setup lang="ts">
import type { TorrentInfo } from '../../api/types'
import TorrentCard from './TorrentCard.vue'

defineProps<{
  torrents: TorrentInfo[]
  loading: boolean
}>()

defineEmits<{
  pause: [hash: string]
  resume: [hash: string]
  remove: [hash: string, deleteFiles: boolean]
}>()
</script>

<template>
  <div>
    <div v-if="loading && torrents.length === 0" class="text-center py-8">
      <p class="text-gray-500">Loading torrents...</p>
    </div>

    <div v-else-if="torrents.length === 0" class="text-center py-8 bg-gray-50 rounded-lg">
      <p class="text-gray-500">No torrents found</p>
    </div>

    <div v-else class="space-y-3">
      <TorrentCard
        v-for="torrent in torrents"
        :key="torrent.hash"
        :torrent="torrent"
        @pause="$emit('pause', torrent.hash)"
        @resume="$emit('resume', torrent.hash)"
        @remove="(deleteFiles) => $emit('remove', torrent.hash, deleteFiles)"
      />
    </div>
  </div>
</template>
