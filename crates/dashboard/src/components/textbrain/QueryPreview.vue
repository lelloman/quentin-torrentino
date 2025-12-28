<script setup lang="ts">
import { ref, computed } from 'vue'
import { useTextBrain, formatConfidence } from '../../composables/useTextBrain'
import type { QueryContextWithExpected, ExpectedContent } from '../../api/types'
import LoadingSpinner from '../common/LoadingSpinner.vue'
import Badge from '../common/Badge.vue'

const emit = defineEmits<{
  queriesGenerated: [queries: string[]]
}>()

const { loading, error, buildQueries, queryResult, generatedQueries, queryConfidence, queryMethod } =
  useTextBrain()

// Form state
const tags = ref<string>('')
const description = ref('')
const contentType = ref<'none' | 'album' | 'track' | 'movie' | 'tv_episode'>('none')

// Album-specific
const albumArtist = ref('')
const albumTitle = ref('')
const albumTracks = ref<{ number: number; title: string }[]>([{ number: 1, title: '' }])

// Track-specific
const trackArtist = ref('')
const trackTitle = ref('')

// Movie-specific
const movieTitle = ref('')
const movieYear = ref<number | undefined>()

// TV-specific
const tvSeries = ref('')
const tvSeason = ref(1)
const tvEpisodes = ref('1')

function addTrack() {
  const nextNumber = albumTracks.value.length + 1
  albumTracks.value.push({ number: nextNumber, title: '' })
}

function removeTrack(index: number) {
  albumTracks.value.splice(index, 1)
  // Renumber
  albumTracks.value.forEach((t, i) => {
    t.number = i + 1
  })
}

function buildExpectedContent(): ExpectedContent | undefined {
  switch (contentType.value) {
    case 'album':
      if (!albumTitle.value) return undefined
      return {
        type: 'album',
        artist: albumArtist.value || undefined,
        title: albumTitle.value,
        tracks: albumTracks.value
          .filter((t) => t.title)
          .map((t) => ({
            number: t.number,
            title: t.title,
          })),
      }
    case 'track':
      if (!trackTitle.value) return undefined
      return {
        type: 'track',
        artist: trackArtist.value || undefined,
        title: trackTitle.value,
      }
    case 'movie':
      if (!movieTitle.value) return undefined
      return {
        type: 'movie',
        title: movieTitle.value,
        year: movieYear.value,
      }
    case 'tv_episode':
      if (!tvSeries.value) return undefined
      const episodes = tvEpisodes.value
        .split(',')
        .map((e) => parseInt(e.trim()))
        .filter((e) => !isNaN(e))
      return {
        type: 'tv_episode',
        series: tvSeries.value,
        season: tvSeason.value,
        episodes,
      }
    default:
      return undefined
  }
}

async function handleSubmit() {
  const context: QueryContextWithExpected = {
    tags: tags.value
      .split(',')
      .map((t) => t.trim())
      .filter((t) => t),
    description: description.value,
    expected: buildExpectedContent(),
  }

  try {
    await buildQueries(context)
    emit('queriesGenerated', generatedQueries.value)
  } catch (e) {
    // Error is handled by composable
  }
}

const confidenceColor = computed(() => {
  const c = queryConfidence.value
  if (c >= 0.85) return 'bg-green-100 text-green-800'
  if (c >= 0.7) return 'bg-yellow-100 text-yellow-800'
  return 'bg-red-100 text-red-800'
})
</script>

<template>
  <div class="bg-white rounded-lg shadow p-6">
    <h2 class="text-lg font-semibold mb-4">Query Preview</h2>

    <form @submit.prevent="handleSubmit" class="space-y-4">
      <!-- Basic Context -->
      <div>
        <label class="block text-sm font-medium text-gray-700 mb-1">Description</label>
        <textarea
          v-model="description"
          rows="2"
          class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
          placeholder="e.g., Abbey Road by The Beatles, 2019 remaster"
        />
      </div>

      <div>
        <label class="block text-sm font-medium text-gray-700 mb-1">Tags (comma-separated)</label>
        <input
          v-model="tags"
          type="text"
          class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
          placeholder="e.g., music, flac, album"
        />
      </div>

      <!-- Expected Content Type -->
      <div>
        <label class="block text-sm font-medium text-gray-700 mb-1">Expected Content</label>
        <select
          v-model="contentType"
          class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
        >
          <option value="none">None (text matching only)</option>
          <option value="album">Music Album</option>
          <option value="track">Single Track</option>
          <option value="movie">Movie</option>
          <option value="tv_episode">TV Episode(s)</option>
        </select>
      </div>

      <!-- Album Fields -->
      <div v-if="contentType === 'album'" class="space-y-3 p-4 bg-gray-50 rounded-md">
        <div class="grid grid-cols-2 gap-3">
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">Artist</label>
            <input
              v-model="albumArtist"
              type="text"
              class="w-full px-3 py-2 border border-gray-300 rounded-md"
              placeholder="The Beatles"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">Album Title</label>
            <input
              v-model="albumTitle"
              type="text"
              class="w-full px-3 py-2 border border-gray-300 rounded-md"
              placeholder="Abbey Road"
            />
          </div>
        </div>

        <div>
          <label class="block text-sm font-medium text-gray-700 mb-1">Tracks</label>
          <div class="space-y-2">
            <div
              v-for="(track, index) in albumTracks"
              :key="index"
              class="flex items-center gap-2"
            >
              <span class="w-8 text-gray-500">{{ track.number }}.</span>
              <input
                v-model="track.title"
                type="text"
                class="flex-1 px-3 py-1 border border-gray-300 rounded-md text-sm"
                placeholder="Track title"
              />
              <button
                v-if="albumTracks.length > 1"
                type="button"
                @click="removeTrack(index)"
                class="text-red-500 hover:text-red-700"
              >
                Remove
              </button>
            </div>
          </div>
          <button
            type="button"
            @click="addTrack"
            class="mt-2 text-sm text-blue-600 hover:text-blue-800"
          >
            + Add Track
          </button>
        </div>
      </div>

      <!-- Track Fields -->
      <div v-if="contentType === 'track'" class="space-y-3 p-4 bg-gray-50 rounded-md">
        <div class="grid grid-cols-2 gap-3">
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">Artist</label>
            <input
              v-model="trackArtist"
              type="text"
              class="w-full px-3 py-2 border border-gray-300 rounded-md"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">Track Title</label>
            <input
              v-model="trackTitle"
              type="text"
              class="w-full px-3 py-2 border border-gray-300 rounded-md"
            />
          </div>
        </div>
      </div>

      <!-- Movie Fields -->
      <div v-if="contentType === 'movie'" class="space-y-3 p-4 bg-gray-50 rounded-md">
        <div class="grid grid-cols-2 gap-3">
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">Movie Title</label>
            <input
              v-model="movieTitle"
              type="text"
              class="w-full px-3 py-2 border border-gray-300 rounded-md"
              placeholder="The Matrix"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">Year</label>
            <input
              v-model.number="movieYear"
              type="number"
              class="w-full px-3 py-2 border border-gray-300 rounded-md"
              placeholder="1999"
            />
          </div>
        </div>
      </div>

      <!-- TV Fields -->
      <div v-if="contentType === 'tv_episode'" class="space-y-3 p-4 bg-gray-50 rounded-md">
        <div>
          <label class="block text-sm font-medium text-gray-700 mb-1">Series Name</label>
          <input
            v-model="tvSeries"
            type="text"
            class="w-full px-3 py-2 border border-gray-300 rounded-md"
            placeholder="Breaking Bad"
          />
        </div>
        <div class="grid grid-cols-2 gap-3">
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">Season</label>
            <input
              v-model.number="tvSeason"
              type="number"
              min="1"
              class="w-full px-3 py-2 border border-gray-300 rounded-md"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">Episodes (comma-separated)</label>
            <input
              v-model="tvEpisodes"
              type="text"
              class="w-full px-3 py-2 border border-gray-300 rounded-md"
              placeholder="1, 2, 3"
            />
          </div>
        </div>
      </div>

      <button
        type="submit"
        :disabled="loading || !description"
        class="w-full px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
      >
        <LoadingSpinner v-if="loading" size="sm" />
        <span>{{ loading ? 'Generating...' : 'Generate Queries' }}</span>
      </button>
    </form>

    <!-- Error -->
    <div v-if="error" class="mt-4 p-3 bg-red-50 text-red-700 rounded-md">
      {{ error }}
    </div>

    <!-- Results -->
    <div v-if="queryResult" class="mt-6 space-y-4">
      <div class="flex items-center justify-between">
        <h3 class="font-medium">Generated Queries</h3>
        <div class="flex items-center gap-2">
          <Badge :class="confidenceColor">
            {{ formatConfidence(queryConfidence) }} confidence
          </Badge>
          <Badge>{{ queryMethod }}</Badge>
        </div>
      </div>

      <ul class="space-y-2">
        <li
          v-for="(query, index) in generatedQueries"
          :key="index"
          class="flex items-center gap-2 p-2 bg-gray-50 rounded-md"
        >
          <span class="text-gray-400 text-sm">{{ index + 1 }}.</span>
          <code class="flex-1 text-sm">{{ query }}</code>
          <button
            @click="$emit('queriesGenerated', [query])"
            class="text-sm text-blue-600 hover:text-blue-800"
          >
            Use
          </button>
        </li>
      </ul>

      <div v-if="queryResult.llm_usage" class="text-xs text-gray-500">
        LLM: {{ queryResult.llm_usage.model }} ({{ queryResult.llm_usage.input_tokens }}+{{
          queryResult.llm_usage.output_tokens
        }} tokens)
      </div>
    </div>
  </div>
</template>
