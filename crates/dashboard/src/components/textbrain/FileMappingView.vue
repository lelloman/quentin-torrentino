<script setup lang="ts">
import { computed } from 'vue'
import { formatConfidence, formatFileSize } from '../../composables/useTextBrain'
import type { ScoredCandidate } from '../../api/types'
import Badge from '../common/Badge.vue'

const props = defineProps<{
  candidate: ScoredCandidate
}>()

const emit = defineEmits<{
  close: []
}>()

// Group mappings by file
const mappedFiles = computed(() => {
  const files = props.candidate.candidate.files ?? []
  const mappings = props.candidate.file_mappings

  return files.map((file) => {
    const mapping = mappings.find((m) => m.torrent_file_path === file.path)
    return {
      file,
      mapping,
    }
  })
})

// Calculate overall mapping quality
const mappingQuality = computed(() => {
  const mappings = props.candidate.file_mappings
  if (mappings.length === 0) return 0
  return mappings.reduce((sum, m) => sum + m.confidence, 0) / mappings.length
})

const unmappedFiles = computed(() => {
  return mappedFiles.value.filter((f) => !f.mapping).length
})

const mappedCount = computed(() => {
  return mappedFiles.value.filter((f) => f.mapping).length
})

function getConfidenceClass(confidence: number): string {
  if (confidence >= 0.85) return 'bg-green-100 text-green-800'
  if (confidence >= 0.7) return 'bg-yellow-100 text-yellow-800'
  if (confidence >= 0.5) return 'bg-orange-100 text-orange-800'
  return 'bg-red-100 text-red-800'
}

function getFileExtension(path: string): string {
  return path.split('.').pop()?.toLowerCase() ?? ''
}

function getFileIcon(path: string): string {
  const ext = getFileExtension(path)
  const audioExts = ['flac', 'mp3', 'aac', 'ogg', 'wav', 'm4a', 'opus']
  const videoExts = ['mkv', 'mp4', 'avi', 'mov', 'wmv']
  const imageExts = ['jpg', 'jpeg', 'png', 'gif', 'webp']

  if (audioExts.includes(ext)) return '🎵'
  if (videoExts.includes(ext)) return '🎬'
  if (imageExts.includes(ext)) return '🖼️'
  return '📄'
}
</script>

<template>
  <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
    <div class="bg-white rounded-lg shadow-xl max-w-3xl w-full max-h-[80vh] flex flex-col">
      <!-- Header -->
      <div class="px-6 py-4 border-b border-gray-200 flex items-center justify-between">
        <div>
          <h2 class="text-lg font-semibold">File Mappings</h2>
          <p class="text-sm text-gray-500 truncate max-w-md">
            {{ candidate.candidate.title }}
          </p>
        </div>
        <button
          @click="emit('close')"
          class="text-gray-400 hover:text-gray-600 text-2xl"
        >
          ×
        </button>
      </div>

      <!-- Summary -->
      <div class="px-6 py-3 bg-gray-50 border-b border-gray-200">
        <div class="flex items-center justify-between text-sm">
          <div class="flex items-center gap-4">
            <span>
              <strong>{{ mappedCount }}</strong> mapped /
              <strong>{{ mappedFiles.length }}</strong> total files
            </span>
            <span v-if="unmappedFiles > 0" class="text-yellow-600">
              ({{ unmappedFiles }} unmapped)
            </span>
          </div>
          <div class="flex items-center gap-2">
            <span class="text-gray-500">Overall quality:</span>
            <Badge :class="getConfidenceClass(mappingQuality)">
              {{ formatConfidence(mappingQuality) }}
            </Badge>
          </div>
        </div>
      </div>

      <!-- File list -->
      <div class="flex-1 overflow-y-auto">
        <table class="w-full">
          <thead class="bg-gray-50 sticky top-0">
            <tr>
              <th class="px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase">
                File
              </th>
              <th class="px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase w-24">
                Size
              </th>
              <th class="px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase w-40">
                Mapped To
              </th>
              <th class="px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase w-24">
                Confidence
              </th>
            </tr>
          </thead>
          <tbody class="divide-y divide-gray-200">
            <tr
              v-for="{ file, mapping } in mappedFiles"
              :key="file.path"
              :class="{
                'bg-green-50': mapping && mapping.confidence >= 0.85,
                'bg-yellow-50': mapping && mapping.confidence >= 0.5 && mapping.confidence < 0.85,
                'bg-gray-50': !mapping,
              }"
            >
              <td class="px-4 py-2">
                <div class="flex items-center gap-2">
                  <span>{{ getFileIcon(file.path) }}</span>
                  <span class="text-sm font-mono truncate max-w-md" :title="file.path">
                    {{ file.path }}
                  </span>
                </div>
              </td>
              <td class="px-4 py-2 text-sm text-gray-500">
                {{ formatFileSize(file.size_bytes) }}
              </td>
              <td class="px-4 py-2">
                <Badge v-if="mapping" variant="info" class="text-xs">
                  {{ mapping.ticket_item_id }}
                </Badge>
                <span v-else class="text-gray-400 text-sm">—</span>
              </td>
              <td class="px-4 py-2">
                <Badge v-if="mapping" :class="getConfidenceClass(mapping.confidence)">
                  {{ formatConfidence(mapping.confidence) }}
                </Badge>
                <span v-else class="text-gray-400 text-sm">—</span>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- Footer -->
      <div class="px-6 py-4 border-t border-gray-200 flex justify-end">
        <button
          @click="emit('close')"
          class="px-4 py-2 bg-gray-100 text-gray-700 rounded-md hover:bg-gray-200"
        >
          Close
        </button>
      </div>
    </div>
  </div>
</template>
