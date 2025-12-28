<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useTextBrain, formatConfidence } from '../composables/useTextBrain'
import type { ScoredCandidate } from '../api/types'
import QueryPreview from '../components/textbrain/QueryPreview.vue'
import CandidateScoring from '../components/textbrain/CandidateScoring.vue'
import FileMappingView from '../components/textbrain/FileMappingView.vue'
import LoadingSpinner from '../components/common/LoadingSpinner.vue'
import Badge from '../components/common/Badge.vue'
import ErrorAlert from '../components/common/ErrorAlert.vue'

const {
  loading,
  error,
  config,
  acquisitionResult,
  fetchConfig,
  allCandidates,
  bestCandidate,
  isAutoApproved,
} = useTextBrain()

// UI state
const selectedCandidate = ref<ScoredCandidate | null>(null)
const fileMappingCandidate = ref<ScoredCandidate | null>(null)

onMounted(async () => {
  try {
    await fetchConfig()
  } catch (e) {
    // Error handled by composable
  }
})

async function handleQueriesGenerated(queries: string[]) {
  // Could use these for search, but for now just log
  console.log('Generated queries:', queries)
}

function handleSelectCandidate(candidate: ScoredCandidate) {
  selectedCandidate.value = candidate
}

function handleViewFiles(candidate: ScoredCandidate) {
  fileMappingCandidate.value = candidate
}

function closeFileMapping() {
  fileMappingCandidate.value = null
}
</script>

<template>
  <div class="p-6 max-w-6xl mx-auto">
    <div class="mb-6">
      <h1 class="text-2xl font-bold text-gray-900">TextBrain</h1>
      <p class="text-gray-600">
        Query building, candidate scoring, and file mapping intelligence
      </p>
    </div>

    <!-- Config Status -->
    <div v-if="config" class="mb-6 flex items-center gap-4 text-sm">
      <Badge>Mode: {{ config.mode }}</Badge>
      <Badge>
        Auto-approve: {{ formatConfidence(config.auto_approve_threshold) }}
      </Badge>
      <Badge
        :class="
          config.llm_configured
            ? 'bg-green-100 text-green-800'
            : 'bg-gray-100 text-gray-800'
        "
      >
        LLM: {{ config.llm_configured ? config.llm_provider : 'Not configured' }}
      </Badge>
    </div>

    <ErrorAlert v-if="error" :message="error" class="mb-6" />

    <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
      <!-- Query Preview Panel -->
      <QueryPreview @queries-generated="handleQueriesGenerated" />

      <!-- Results Panel -->
      <div class="space-y-6">
        <!-- Loading state -->
        <div
          v-if="loading"
          class="bg-white rounded-lg shadow p-6 flex items-center justify-center"
        >
          <LoadingSpinner />
          <span class="ml-2">Processing...</span>
        </div>

        <!-- Acquisition Result Summary -->
        <div
          v-if="acquisitionResult && !loading"
          class="bg-white rounded-lg shadow p-6"
        >
          <h2 class="text-lg font-semibold mb-4">Acquisition Result</h2>

          <div class="space-y-3">
            <div class="flex items-center justify-between">
              <span class="text-gray-600">Queries tried:</span>
              <span class="font-medium">{{ acquisitionResult.queries_tried.length }}</span>
            </div>
            <div class="flex items-center justify-between">
              <span class="text-gray-600">Candidates evaluated:</span>
              <span class="font-medium">{{ acquisitionResult.candidates_evaluated }}</span>
            </div>
            <div class="flex items-center justify-between">
              <span class="text-gray-600">Duration:</span>
              <span class="font-medium">{{ acquisitionResult.duration_ms }}ms</span>
            </div>
            <div class="flex items-center justify-between">
              <span class="text-gray-600">Methods:</span>
              <span class="font-medium">
                {{ acquisitionResult.query_method }} / {{ acquisitionResult.score_method }}
              </span>
            </div>
            <div class="flex items-center justify-between">
              <span class="text-gray-600">Auto-approved:</span>
              <Badge
                :class="
                  isAutoApproved
                    ? 'bg-green-100 text-green-800'
                    : 'bg-yellow-100 text-yellow-800'
                "
              >
                {{ isAutoApproved ? 'Yes' : 'No' }}
              </Badge>
            </div>
          </div>

          <!-- Best candidate highlight -->
          <div v-if="bestCandidate" class="mt-4 p-4 bg-gray-50 rounded-md">
            <h3 class="font-medium mb-2">Best Match</h3>
            <p class="text-sm font-medium">{{ bestCandidate.candidate.title }}</p>
            <p class="text-sm text-gray-600">{{ bestCandidate.reasoning }}</p>
            <div class="mt-2 flex items-center gap-2">
              <Badge
                :class="
                  bestCandidate.score >= 0.85
                    ? 'bg-green-100 text-green-800'
                    : 'bg-yellow-100 text-yellow-800'
                "
              >
                {{ formatConfidence(bestCandidate.score) }}
              </Badge>
              <span class="text-xs text-gray-500">
                {{ bestCandidate.file_mappings.length }} file mappings
              </span>
            </div>
          </div>

          <!-- Queries tried -->
          <div class="mt-4">
            <h3 class="font-medium mb-2 text-sm text-gray-600">Queries Tried</h3>
            <ul class="text-sm space-y-1">
              <li
                v-for="(query, idx) in acquisitionResult.queries_tried"
                :key="idx"
                class="font-mono text-gray-700"
              >
                {{ idx + 1 }}. {{ query }}
              </li>
            </ul>
          </div>
        </div>

        <!-- Candidate Scoring -->
        <CandidateScoring
          v-if="allCandidates.length > 0"
          :candidates="allCandidates"
          :selected-hash="selectedCandidate?.candidate.info_hash"
          :auto-approve-threshold="config?.auto_approve_threshold"
          @select="handleSelectCandidate"
          @view-files="handleViewFiles"
        />
      </div>
    </div>

    <!-- File Mapping Modal -->
    <FileMappingView
      v-if="fileMappingCandidate"
      :candidate="fileMappingCandidate"
      @close="closeFileMapping"
    />
  </div>
</template>
