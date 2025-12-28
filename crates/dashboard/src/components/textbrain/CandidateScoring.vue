<script setup lang="ts">
import { computed } from 'vue'
import {
  formatConfidence,
  formatFileSize,
  getScoreColorClass,
} from '../../composables/useTextBrain'
import type { ScoredCandidate, FileMapping } from '../../api/types'
import Badge from '../common/Badge.vue'

const props = defineProps<{
  candidates: ScoredCandidate[]
  selectedHash?: string
  autoApproveThreshold?: number
}>()

const emit = defineEmits<{
  select: [candidate: ScoredCandidate]
  viewFiles: [candidate: ScoredCandidate]
}>()

const threshold = computed(() => props.autoApproveThreshold ?? 0.85)

function isAutoApproved(score: number): boolean {
  return score >= threshold.value
}

function getFileMappingQuality(mappings: FileMapping[]): string {
  if (mappings.length === 0) return 'No mappings'
  const avgConfidence =
    mappings.reduce((sum, m) => sum + m.confidence, 0) / mappings.length
  return `${mappings.length} files (${formatConfidence(avgConfidence)} avg)`
}
</script>

<template>
  <div class="bg-white rounded-lg shadow">
    <div class="px-6 py-4 border-b border-gray-200">
      <h2 class="text-lg font-semibold">Scored Candidates</h2>
      <p class="text-sm text-gray-500">
        {{ candidates.length }} candidate(s) scored. Auto-approve threshold:
        {{ formatConfidence(threshold) }}
      </p>
    </div>

    <div v-if="candidates.length === 0" class="p-6 text-center text-gray-500">
      No candidates to display
    </div>

    <ul v-else class="divide-y divide-gray-200">
      <li
        v-for="(scored, index) in candidates"
        :key="scored.candidate.info_hash"
        class="p-4 hover:bg-gray-50 cursor-pointer"
        :class="{
          'bg-blue-50': selectedHash === scored.candidate.info_hash,
          'ring-2 ring-green-500 ring-inset': isAutoApproved(scored.score) && index === 0,
        }"
        @click="emit('select', scored)"
      >
        <div class="flex items-start justify-between gap-4">
          <!-- Main info -->
          <div class="flex-1 min-w-0">
            <div class="flex items-center gap-2 mb-1">
              <span class="text-lg font-medium text-gray-500">{{ index + 1 }}.</span>
              <h3 class="font-medium text-gray-900 truncate">
                {{ scored.candidate.title }}
              </h3>
            </div>

            <div class="flex flex-wrap items-center gap-2 text-sm text-gray-500 mb-2">
              <span>{{ formatFileSize(scored.candidate.size_bytes) }}</span>
              <span>·</span>
              <span class="text-green-600">{{ scored.candidate.seeders }} seeders</span>
              <span>·</span>
              <span class="text-red-600">{{ scored.candidate.leechers }} leechers</span>
              <span v-if="scored.candidate.category">·</span>
              <span v-if="scored.candidate.category">{{ scored.candidate.category }}</span>
            </div>

            <p class="text-sm text-gray-600 italic">
              {{ scored.reasoning }}
            </p>

            <!-- File mappings -->
            <div
              v-if="scored.file_mappings.length > 0"
              class="mt-2 flex items-center gap-2"
            >
              <Badge class="text-xs">
                {{ getFileMappingQuality(scored.file_mappings) }}
              </Badge>
              <button
                @click.stop="emit('viewFiles', scored)"
                class="text-xs text-blue-600 hover:text-blue-800"
              >
                View mappings
              </button>
            </div>
          </div>

          <!-- Score -->
          <div class="flex flex-col items-end gap-1">
            <div
              class="text-2xl font-bold"
              :class="getScoreColorClass(scored.score)"
            >
              {{ formatConfidence(scored.score) }}
            </div>
            <Badge
              v-if="isAutoApproved(scored.score) && index === 0"
              class="bg-green-100 text-green-800"
            >
              Auto-approved
            </Badge>
            <Badge
              v-else-if="isAutoApproved(scored.score)"
              class="bg-yellow-100 text-yellow-800"
            >
              High confidence
            </Badge>
          </div>
        </div>

        <!-- Sources -->
        <div class="mt-2 flex flex-wrap gap-1">
          <Badge
            v-for="source in scored.candidate.sources"
            :key="source.indexer"
            class="text-xs"
          >
            {{ source.indexer }}
          </Badge>
        </div>
      </li>
    </ul>
  </div>
</template>
