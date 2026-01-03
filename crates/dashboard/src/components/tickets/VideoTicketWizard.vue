<script setup lang="ts">
import { ref, computed } from 'vue'
import { useTicketWizard } from '../../composables/useTicketWizard'
import LoadingSpinner from '../common/LoadingSpinner.vue'
import ErrorAlert from '../common/ErrorAlert.vue'
import type { CreateTicketWithCatalogRequest, Resolution, VideoSource, VideoSearchCodec, LanguagePreference, LanguagePriority } from '../../api/types'

const emit = defineEmits<{
  submit: [request: CreateTicketWithCatalogRequest]
  cancel: []
}>()

const wizard = useTicketWizard()

// Video mode: movie or tv
const videoMode = ref<'movie' | 'tv'>('movie')

// Track whether a search has been performed
const hasSearched = ref(false)

// Resolution options
const resolutionOptions: { value: Resolution; label: string }[] = [
  { value: 'r720p', label: '720p' },
  { value: 'r1080p', label: '1080p' },
  { value: 'r2160p', label: '4K (2160p)' },
]

// Source options
const sourceOptions: { value: VideoSource; label: string }[] = [
  { value: 'blu_ray', label: 'Blu-Ray' },
  { value: 'remux', label: 'Remux' },
  { value: 'web_dl', label: 'WEB-DL' },
  { value: 'hdtv', label: 'HDTV' },
  { value: 'cam', label: 'CAM' },
]

// Codec options
const codecOptions: { value: VideoSearchCodec; label: string }[] = [
  { value: 'x265', label: 'x265/HEVC' },
  { value: 'x264', label: 'x264' },
  { value: 'av1', label: 'AV1' },
]

// Language options (ISO 639-1 codes)
const languageOptions: { code: string; label: string }[] = [
  { code: 'en', label: 'English' },
  { code: 'it', label: 'Italian' },
  { code: 'de', label: 'German' },
  { code: 'fr', label: 'French' },
  { code: 'es', label: 'Spanish' },
  { code: 'pt', label: 'Portuguese' },
  { code: 'ru', label: 'Russian' },
  { code: 'ja', label: 'Japanese' },
  { code: 'ko', label: 'Korean' },
  { code: 'zh', label: 'Chinese' },
  { code: 'nl', label: 'Dutch' },
  { code: 'pl', label: 'Polish' },
  { code: 'sv', label: 'Swedish' },
  { code: 'no', label: 'Norwegian' },
  { code: 'da', label: 'Danish' },
  { code: 'fi', label: 'Finnish' },
  { code: 'tr', label: 'Turkish' },
  { code: 'ar', label: 'Arabic' },
  { code: 'hi', label: 'Hindi' },
  { code: 'th', label: 'Thai' },
]

// Helper to add a language preference
function addLanguage(type: 'audio' | 'subtitle', code: string) {
  const constraints = wizard.videoConstraints.value
  const list = type === 'audio' ? constraints.audio_languages : constraints.subtitle_languages

  // Check if already added
  if (list?.some(l => l.code === code)) return

  const newPref: LanguagePreference = { code, priority: 'preferred' }
  if (type === 'audio') {
    constraints.audio_languages = [...(list || []), newPref]
  } else {
    constraints.subtitle_languages = [...(list || []), newPref]
  }
}

// Helper to remove a language preference
function removeLanguage(type: 'audio' | 'subtitle', code: string) {
  const constraints = wizard.videoConstraints.value
  if (type === 'audio') {
    constraints.audio_languages = constraints.audio_languages?.filter(l => l.code !== code)
  } else {
    constraints.subtitle_languages = constraints.subtitle_languages?.filter(l => l.code !== code)
  }
}

// Helper to toggle language priority
function togglePriority(type: 'audio' | 'subtitle', code: string) {
  const constraints = wizard.videoConstraints.value
  const list = type === 'audio' ? constraints.audio_languages : constraints.subtitle_languages
  const updated = list?.map(l =>
    l.code === code
      ? { ...l, priority: (l.priority === 'required' ? 'preferred' : 'required') as LanguagePriority }
      : l
  )
  if (type === 'audio') {
    constraints.audio_languages = updated
  } else {
    constraints.subtitle_languages = updated
  }
}

// Helper to get language label from code
function getLanguageLabel(code: string): string {
  return languageOptions.find(l => l.code === code)?.label || code.toUpperCase()
}

// Initialize based on video mode
function setVideoMode(mode: 'movie' | 'tv') {
  videoMode.value = mode
  wizard.setContentType(mode)
  wizard.goToStep('search')
  hasSearched.value = false
}

// Start with movie mode
setVideoMode('movie')

// TMDB image base URL
const tmdbImageBase = 'https://image.tmdb.org/t/p/w185'

// Format year from date string
function formatYear(dateStr?: string): string {
  if (!dateStr) return ''
  return dateStr.substring(0, 4)
}

// Search handler
async function handleSearch() {
  if (!wizard.searchQuery.value.trim()) return
  hasSearched.value = true
  await wizard.performSearch()
}

// Select a movie from search results
async function handleSelectMovie(id: number) {
  await wizard.selectMovie(id)
}

// Select a TV series from search results
async function handleSelectSeries(id: number) {
  await wizard.selectSeries(id)
}

// Select a season
async function handleSelectSeason(seasonNumber: number) {
  if (!wizard.selectedSeries.value) return
  await wizard.selectSeason(wizard.selectedSeries.value.id, seasonNumber)
}

// Check if we have a valid selection
const hasValidSelection = computed(() => {
  if (videoMode.value === 'movie') {
    return wizard.selectedMovie.value !== null
  } else {
    return wizard.selectedSeries.value !== null && wizard.selectedSeason.value !== null
  }
})

// Handle wizard completion
function handleSubmit() {
  const request = wizard.buildTicketRequest()
  emit('submit', request)
}

// Handle cancel
function handleCancel() {
  wizard.reset()
  emit('cancel')
}

// Handle previous step
function handlePrevStep() {
  if (wizard.currentStep.value === 'constraints') {
    wizard.goToStep('search')
  } else {
    wizard.prevStep()
  }
}

// Step indicators
const steps = [
  { key: 'search', label: 'Search' },
  { key: 'constraints', label: 'Quality' },
  { key: 'details', label: 'Details' },
  { key: 'review', label: 'Review' },
] as const

const currentStepIdx = computed(() => {
  const idx = steps.findIndex((s) => s.key === wizard.currentStep.value)
  return idx >= 0 ? idx : 0
})

// Can proceed logic - override for video-specific validation
const canProceedVideo = computed(() => {
  if (wizard.currentStep.value === 'search') {
    return hasValidSelection.value
  }
  return wizard.canProceed.value
})
</script>

<template>
  <div class="card">
    <!-- Header -->
    <div class="flex items-center justify-between mb-6">
      <h2 class="text-lg font-semibold flex items-center gap-2">
        <span class="i-carbon-video text-xl"></span>
        New Video Ticket
      </h2>
      <button @click="handleCancel" class="text-gray-400 hover:text-gray-600">
        <span class="i-carbon-close text-xl"></span>
      </button>
    </div>

    <!-- Video Mode Toggle -->
    <div class="flex gap-2 mb-6">
      <button
        @click="setVideoMode('movie')"
        class="flex-1 py-2 px-4 rounded-lg border-2 transition-colors flex items-center justify-center gap-2"
        :class="{
          'border-blue-600 bg-blue-50 text-blue-700': videoMode === 'movie',
          'border-gray-200 hover:border-gray-300': videoMode !== 'movie',
        }"
      >
        <span class="i-carbon-media-library"></span>
        Movie
      </button>
      <button
        @click="setVideoMode('tv')"
        class="flex-1 py-2 px-4 rounded-lg border-2 transition-colors flex items-center justify-center gap-2"
        :class="{
          'border-blue-600 bg-blue-50 text-blue-700': videoMode === 'tv',
          'border-gray-200 hover:border-gray-300': videoMode !== 'tv',
        }"
      >
        <span class="i-carbon-tv"></span>
        TV Series
      </button>
    </div>

    <!-- Step Indicator -->
    <div class="flex items-center gap-2 mb-6">
      <template v-for="(step, idx) in steps" :key="step.key">
        <div
          class="flex items-center gap-2"
          :class="{
            'text-blue-600': idx === currentStepIdx,
            'text-gray-400': idx !== currentStepIdx,
          }"
        >
          <div
            class="w-6 h-6 rounded-full flex items-center justify-center text-xs font-medium"
            :class="{
              'bg-blue-600 text-white': idx === currentStepIdx,
              'bg-blue-100 text-blue-600': idx < currentStepIdx,
              'bg-gray-200 text-gray-500': idx > currentStepIdx,
            }"
          >
            <span v-if="idx < currentStepIdx" class="i-carbon-checkmark"></span>
            <span v-else>{{ idx + 1 }}</span>
          </div>
          <span class="text-sm font-medium hidden sm:inline">{{ step.label }}</span>
        </div>
        <div
          v-if="idx < steps.length - 1"
          class="flex-1 h-0.5 mx-2"
          :class="{
            'bg-blue-600': idx < currentStepIdx,
            'bg-gray-200': idx >= currentStepIdx,
          }"
        ></div>
      </template>
    </div>

    <!-- Error Display -->
    <ErrorAlert
      v-if="wizard.catalogError.value"
      :message="wizard.catalogError.value"
      @dismiss="wizard.catalogError.value = null"
      class="mb-4"
    />

    <!-- TMDB Not Available Warning -->
    <div
      v-if="wizard.catalogStatus.value && !wizard.catalogStatus.value.tmdb_available"
      class="bg-yellow-50 border border-yellow-200 rounded-lg p-4 mb-4"
    >
      <div class="flex items-start gap-3">
        <span class="i-carbon-warning text-yellow-600 text-xl"></span>
        <div>
          <p class="text-sm font-medium text-yellow-800">TMDB Not Configured</p>
          <p class="text-sm text-yellow-700 mt-1">
            The TMDB catalog is not available. Please configure a TMDB API key in the server config
            to use video search.
          </p>
        </div>
      </div>
    </div>

    <!-- Step 1: Search & Select -->
    <div v-if="wizard.currentStep.value === 'search'" class="space-y-4">
      <p class="text-sm text-gray-600">
        Search TMDB for the {{ videoMode === 'movie' ? 'movie' : 'TV series' }} you want to download.
      </p>

      <!-- Search Form -->
      <form @submit.prevent="handleSearch" class="flex gap-2">
        <input
          v-model="wizard.searchQuery.value"
          type="text"
          class="input flex-1"
          :placeholder="videoMode === 'movie' ? 'Movie title...' : 'TV series name...'"
          :disabled="wizard.catalogLoading.value"
        />
        <button
          type="submit"
          class="btn-primary"
          :disabled="!wizard.searchQuery.value.trim() || wizard.catalogLoading.value"
        >
          <span v-if="wizard.catalogLoading.value" class="i-carbon-circle-dash animate-spin"></span>
          <span v-else class="i-carbon-search"></span>
          Search
        </button>
      </form>

      <!-- Loading State -->
      <div v-if="wizard.catalogLoading.value" class="flex justify-center py-8">
        <LoadingSpinner size="lg" />
      </div>

      <!-- Movie Search Results -->
      <template v-else-if="videoMode === 'movie'">
        <div
          v-if="wizard.tmdbMovieResults.value.length > 0"
          class="border rounded-lg divide-y max-h-96 overflow-y-auto"
        >
          <button
            v-for="movie in wizard.tmdbMovieResults.value"
            :key="movie.id"
            @click="handleSelectMovie(movie.id)"
            class="w-full p-3 text-left hover:bg-gray-50 transition-colors flex gap-3"
            :class="{
              'bg-blue-50 border-l-4 border-l-blue-600': wizard.selectedMovie.value?.id === movie.id,
            }"
          >
            <!-- Poster -->
            <div class="w-12 h-18 flex-shrink-0 bg-gray-100 rounded overflow-hidden">
              <img
                v-if="movie.poster_path"
                :src="`${tmdbImageBase}${movie.poster_path}`"
                :alt="movie.title"
                class="w-full h-full object-cover"
                loading="lazy"
              />
              <div v-else class="w-full h-full flex items-center justify-center text-gray-400">
                <span class="i-carbon-media-library text-xl"></span>
              </div>
            </div>
            <!-- Info -->
            <div class="flex-1 min-w-0">
              <div class="font-medium text-gray-900 truncate">{{ movie.title }}</div>
              <div class="text-sm text-gray-600 truncate">{{ movie.original_title }}</div>
              <div class="text-xs text-gray-500 mt-1 flex items-center gap-3">
                <span v-if="movie.release_date">{{ formatYear(movie.release_date) }}</span>
                <span v-if="movie.genres?.length" class="truncate">{{ movie.genres.slice(0, 2).join(', ') }}</span>
              </div>
            </div>
          </button>
        </div>

        <!-- No Results -->
        <div
          v-else-if="hasSearched && !wizard.catalogLoading.value"
          class="text-center py-8 text-gray-500"
        >
          <span class="i-carbon-search text-4xl mb-2 block"></span>
          <p>No movies found. Try a different search term.</p>
        </div>

        <!-- Selected Movie Details -->
        <div v-if="wizard.selectedMovie.value" class="bg-blue-50 rounded-lg p-4 mt-4">
          <div class="flex items-start gap-4">
            <!-- Poster -->
            <div class="w-20 h-30 flex-shrink-0 bg-gray-200 rounded overflow-hidden">
              <img
                v-if="wizard.selectedMovie.value.poster_path"
                :src="`${tmdbImageBase}${wizard.selectedMovie.value.poster_path}`"
                :alt="wizard.selectedMovie.value.title"
                class="w-full h-full object-cover"
              />
              <div v-else class="w-full h-full flex items-center justify-center text-gray-400">
                <span class="i-carbon-media-library text-2xl"></span>
              </div>
            </div>
            <div class="flex-1 min-w-0">
              <div class="flex items-start justify-between">
                <div>
                  <h3 class="font-semibold text-gray-900">{{ wizard.selectedMovie.value.title }}</h3>
                  <p class="text-sm text-gray-600">{{ formatYear(wizard.selectedMovie.value.release_date) }}</p>
                </div>
                <button
                  @click="wizard.selectedMovie.value = null"
                  class="text-gray-400 hover:text-gray-600"
                >
                  <span class="i-carbon-close"></span>
                </button>
              </div>
              <p class="text-sm text-gray-600 mt-2 line-clamp-3">{{ wizard.selectedMovie.value.overview }}</p>
              <div class="text-xs text-gray-500 mt-2 flex items-center gap-3">
                <span v-if="wizard.selectedMovie.value.runtime_minutes">
                  {{ wizard.selectedMovie.value.runtime_minutes }} min
                </span>
                <span v-if="wizard.selectedMovie.value.genres?.length">
                  {{ wizard.selectedMovie.value.genres.join(', ') }}
                </span>
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- TV Series Search Results -->
      <template v-else>
        <div
          v-if="wizard.tmdbTvResults.value.length > 0 && !wizard.selectedSeries.value"
          class="border rounded-lg divide-y max-h-96 overflow-y-auto"
        >
          <button
            v-for="series in wizard.tmdbTvResults.value"
            :key="series.id"
            @click="handleSelectSeries(series.id)"
            class="w-full p-3 text-left hover:bg-gray-50 transition-colors flex gap-3"
          >
            <!-- Poster -->
            <div class="w-12 h-18 flex-shrink-0 bg-gray-100 rounded overflow-hidden">
              <img
                v-if="series.poster_path"
                :src="`${tmdbImageBase}${series.poster_path}`"
                :alt="series.name"
                class="w-full h-full object-cover"
                loading="lazy"
              />
              <div v-else class="w-full h-full flex items-center justify-center text-gray-400">
                <span class="i-carbon-tv text-xl"></span>
              </div>
            </div>
            <!-- Info -->
            <div class="flex-1 min-w-0">
              <div class="font-medium text-gray-900 truncate">{{ series.name }}</div>
              <div class="text-sm text-gray-600 truncate">{{ series.original_name }}</div>
              <div class="text-xs text-gray-500 mt-1 flex items-center gap-3">
                <span v-if="series.first_air_date">{{ formatYear(series.first_air_date) }}</span>
                <span>{{ series.number_of_seasons }} seasons</span>
              </div>
            </div>
          </button>
        </div>

        <!-- No Results -->
        <div
          v-else-if="hasSearched && !wizard.catalogLoading.value && !wizard.selectedSeries.value && wizard.tmdbTvResults.value.length === 0"
          class="text-center py-8 text-gray-500"
        >
          <span class="i-carbon-search text-4xl mb-2 block"></span>
          <p>No TV series found. Try a different search term.</p>
        </div>

        <!-- Selected Series + Season Picker -->
        <div v-if="wizard.selectedSeries.value" class="bg-blue-50 rounded-lg p-4 mt-4">
          <div class="flex items-start gap-4">
            <!-- Poster -->
            <div class="w-20 h-30 flex-shrink-0 bg-gray-200 rounded overflow-hidden">
              <img
                v-if="wizard.selectedSeries.value.poster_path"
                :src="`${tmdbImageBase}${wizard.selectedSeries.value.poster_path}`"
                :alt="wizard.selectedSeries.value.name"
                class="w-full h-full object-cover"
              />
              <div v-else class="w-full h-full flex items-center justify-center text-gray-400">
                <span class="i-carbon-tv text-2xl"></span>
              </div>
            </div>
            <div class="flex-1 min-w-0">
              <div class="flex items-start justify-between">
                <div>
                  <h3 class="font-semibold text-gray-900">{{ wizard.selectedSeries.value.name }}</h3>
                  <p class="text-sm text-gray-600">
                    {{ formatYear(wizard.selectedSeries.value.first_air_date) }} &middot;
                    {{ wizard.selectedSeries.value.number_of_seasons }} seasons
                  </p>
                </div>
                <button
                  @click="wizard.selectedSeries.value = null; wizard.selectedSeason.value = null"
                  class="text-gray-400 hover:text-gray-600"
                >
                  <span class="i-carbon-close"></span>
                </button>
              </div>
              <p class="text-sm text-gray-600 mt-2 line-clamp-2">{{ wizard.selectedSeries.value.overview }}</p>
            </div>
          </div>

          <!-- Season Picker -->
          <div class="mt-4 border-t border-blue-200 pt-4">
            <label class="block text-sm font-medium text-gray-700 mb-2">Select Season</label>
            <div class="flex flex-wrap gap-2">
              <button
                v-for="season in wizard.selectedSeries.value.seasons"
                :key="season.season_number"
                @click="handleSelectSeason(season.season_number)"
                class="px-3 py-2 rounded-lg border-2 text-sm transition-colors"
                :class="{
                  'border-blue-600 bg-blue-600 text-white': wizard.selectedSeason.value?.season_number === season.season_number,
                  'border-gray-300 hover:border-blue-400': wizard.selectedSeason.value?.season_number !== season.season_number,
                }"
              >
                <span v-if="season.season_number === 0">Specials</span>
                <span v-else>S{{ season.season_number.toString().padStart(2, '0') }}</span>
                <span class="text-xs opacity-75 ml-1">({{ season.episode_count }} ep)</span>
              </button>
            </div>
          </div>

          <!-- Selected Season Episodes -->
          <div v-if="wizard.selectedSeason.value" class="mt-4 border-t border-blue-200 pt-4">
            <h4 class="text-sm font-medium text-gray-700 mb-2">
              {{ wizard.selectedSeason.value.name }} &middot; {{ wizard.selectedSeason.value.episode_count }} episodes
            </h4>
            <div class="max-h-48 overflow-y-auto">
              <table class="w-full text-sm">
                <thead class="text-xs text-gray-500 uppercase sticky top-0 bg-blue-50">
                  <tr>
                    <th class="text-left py-1 w-12">Ep</th>
                    <th class="text-left py-1">Title</th>
                    <th class="text-right py-1 w-16">Runtime</th>
                  </tr>
                </thead>
                <tbody class="text-gray-700">
                  <tr v-for="episode in wizard.selectedSeason.value.episodes" :key="episode.episode_number">
                    <td class="py-1 text-gray-500">{{ episode.episode_number }}</td>
                    <td class="py-1 truncate max-w-xs">{{ episode.name }}</td>
                    <td class="py-1 text-right text-gray-500">
                      {{ episode.runtime_minutes ? `${episode.runtime_minutes}m` : '--' }}
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>
        </div>
      </template>
    </div>

    <!-- Step 2: Quality Constraints -->
    <div v-else-if="wizard.currentStep.value === 'constraints'" class="space-y-6">
      <p class="text-sm text-gray-600">
        Set your quality preferences for the torrent search.
      </p>

      <!-- Resolution -->
      <div>
        <label class="block text-sm font-medium text-gray-700 mb-2">Preferred Resolution</label>
        <div class="flex flex-wrap gap-2">
          <label
            v-for="res in resolutionOptions"
            :key="res.value"
            class="flex items-center gap-2 px-3 py-2 border rounded-lg cursor-pointer transition-colors"
            :class="{
              'border-blue-600 bg-blue-50': wizard.videoConstraints.value.preferred_resolution === res.value,
              'border-gray-200 hover:border-gray-300': wizard.videoConstraints.value.preferred_resolution !== res.value,
            }"
          >
            <input
              type="radio"
              :value="res.value"
              v-model="wizard.videoConstraints.value.preferred_resolution"
              class="sr-only"
            />
            <span class="text-sm">{{ res.label }}</span>
          </label>
          <button
            v-if="wizard.videoConstraints.value.preferred_resolution"
            @click="wizard.videoConstraints.value.preferred_resolution = undefined"
            class="text-xs text-gray-500 hover:text-gray-700 px-2"
          >
            Clear
          </button>
        </div>
      </div>

      <!-- Minimum Resolution -->
      <div>
        <label class="block text-sm font-medium text-gray-700 mb-2">Minimum Resolution</label>
        <select v-model="wizard.videoConstraints.value.min_resolution" class="input w-48">
          <option :value="undefined">No minimum</option>
          <option v-for="res in resolutionOptions" :key="res.value" :value="res.value">
            {{ res.label }}
          </option>
        </select>
      </div>

      <!-- Preferred Sources -->
      <div>
        <label class="block text-sm font-medium text-gray-700 mb-2">Preferred Sources</label>
        <div class="flex flex-wrap gap-2">
          <label
            v-for="source in sourceOptions"
            :key="source.value"
            class="flex items-center gap-2 px-3 py-2 border rounded-lg cursor-pointer transition-colors"
            :class="{
              'border-blue-600 bg-blue-50': wizard.videoConstraints.value.preferred_sources?.includes(source.value),
              'border-gray-200 hover:border-gray-300': !wizard.videoConstraints.value.preferred_sources?.includes(source.value),
            }"
          >
            <input
              type="checkbox"
              :checked="wizard.videoConstraints.value.preferred_sources?.includes(source.value)"
              @change="(e) => {
                const sources = wizard.videoConstraints.value.preferred_sources || []
                if ((e.target as HTMLInputElement).checked) {
                  wizard.videoConstraints.value.preferred_sources = [...sources, source.value]
                } else {
                  wizard.videoConstraints.value.preferred_sources = sources.filter(s => s !== source.value)
                }
              }"
              class="sr-only"
            />
            <span class="text-sm">{{ source.label }}</span>
          </label>
        </div>
        <p class="text-xs text-gray-500 mt-1">Leave empty to accept any source</p>
      </div>

      <!-- Preferred Codecs -->
      <div>
        <label class="block text-sm font-medium text-gray-700 mb-2">Preferred Codecs</label>
        <div class="flex flex-wrap gap-2">
          <label
            v-for="codec in codecOptions"
            :key="codec.value"
            class="flex items-center gap-2 px-3 py-2 border rounded-lg cursor-pointer transition-colors"
            :class="{
              'border-blue-600 bg-blue-50': wizard.videoConstraints.value.preferred_codecs?.includes(codec.value),
              'border-gray-200 hover:border-gray-300': !wizard.videoConstraints.value.preferred_codecs?.includes(codec.value),
            }"
          >
            <input
              type="checkbox"
              :checked="wizard.videoConstraints.value.preferred_codecs?.includes(codec.value)"
              @change="(e) => {
                const codecs = wizard.videoConstraints.value.preferred_codecs || []
                if ((e.target as HTMLInputElement).checked) {
                  wizard.videoConstraints.value.preferred_codecs = [...codecs, codec.value]
                } else {
                  wizard.videoConstraints.value.preferred_codecs = codecs.filter(c => c !== codec.value)
                }
              }"
              class="sr-only"
            />
            <span class="text-sm">{{ codec.label }}</span>
          </label>
        </div>
      </div>

      <!-- Audio Languages -->
      <div>
        <label class="block text-sm font-medium text-gray-700 mb-2">Audio Languages</label>
        <div class="flex flex-wrap gap-2 mb-2">
          <div
            v-for="lang in wizard.videoConstraints.value.audio_languages"
            :key="lang.code"
            class="flex items-center gap-1 px-2 py-1 rounded-lg text-sm"
            :class="{
              'bg-blue-100 text-blue-800': lang.priority === 'required',
              'bg-gray-100 text-gray-700': lang.priority === 'preferred',
            }"
          >
            <span>{{ getLanguageLabel(lang.code) }}</span>
            <button
              @click="togglePriority('audio', lang.code)"
              class="px-1.5 py-0.5 text-xs rounded"
              :class="{
                'bg-blue-200 hover:bg-blue-300': lang.priority === 'required',
                'bg-gray-200 hover:bg-gray-300': lang.priority === 'preferred',
              }"
              :title="lang.priority === 'required' ? 'Click for preferred' : 'Click for required'"
            >
              {{ lang.priority === 'required' ? 'REQ' : 'PREF' }}
            </button>
            <button
              @click="removeLanguage('audio', lang.code)"
              class="ml-1 text-gray-500 hover:text-gray-700"
            >
              <span class="i-carbon-close text-xs"></span>
            </button>
          </div>
        </div>
        <select
          @change="(e) => { addLanguage('audio', (e.target as HTMLSelectElement).value); (e.target as HTMLSelectElement).value = '' }"
          class="input w-48"
        >
          <option value="">Add audio language...</option>
          <option
            v-for="lang in languageOptions.filter(l => !wizard.videoConstraints.value.audio_languages?.some(a => a.code === l.code))"
            :key="lang.code"
            :value="lang.code"
          >
            {{ lang.label }}
          </option>
        </select>
        <p class="text-xs text-gray-500 mt-1">Required = stronger boost, Preferred = moderate boost</p>
      </div>

      <!-- Subtitle Languages -->
      <div>
        <label class="block text-sm font-medium text-gray-700 mb-2">Subtitle Languages</label>
        <div class="flex flex-wrap gap-2 mb-2">
          <div
            v-for="lang in wizard.videoConstraints.value.subtitle_languages"
            :key="lang.code"
            class="flex items-center gap-1 px-2 py-1 rounded-lg text-sm"
            :class="{
              'bg-green-100 text-green-800': lang.priority === 'required',
              'bg-gray-100 text-gray-700': lang.priority === 'preferred',
            }"
          >
            <span>{{ getLanguageLabel(lang.code) }}</span>
            <button
              @click="togglePriority('subtitle', lang.code)"
              class="px-1.5 py-0.5 text-xs rounded"
              :class="{
                'bg-green-200 hover:bg-green-300': lang.priority === 'required',
                'bg-gray-200 hover:bg-gray-300': lang.priority === 'preferred',
              }"
              :title="lang.priority === 'required' ? 'Click for preferred' : 'Click for required'"
            >
              {{ lang.priority === 'required' ? 'REQ' : 'PREF' }}
            </button>
            <button
              @click="removeLanguage('subtitle', lang.code)"
              class="ml-1 text-gray-500 hover:text-gray-700"
            >
              <span class="i-carbon-close text-xs"></span>
            </button>
          </div>
        </div>
        <select
          @change="(e) => { addLanguage('subtitle', (e.target as HTMLSelectElement).value); (e.target as HTMLSelectElement).value = '' }"
          class="input w-48"
        >
          <option value="">Add subtitle language...</option>
          <option
            v-for="lang in languageOptions.filter(l => !wizard.videoConstraints.value.subtitle_languages?.some(s => s.code === l.code))"
            :key="lang.code"
            :value="lang.code"
          >
            {{ lang.label }}
          </option>
        </select>
        <p class="text-xs text-gray-500 mt-1">Required = stronger boost, Preferred = moderate boost</p>
      </div>

      <!-- Exclude Hardcoded Subs -->
      <div class="space-y-3">
        <label class="flex items-center gap-3 cursor-pointer">
          <input
            type="checkbox"
            v-model="wizard.videoConstraints.value.exclude_hardcoded_subs"
            class="w-4 h-4 text-blue-600 rounded"
          />
          <span class="text-sm">Exclude releases with hardcoded subtitles</span>
        </label>
      </div>
    </div>

    <!-- Step 3: Ticket Details -->
    <div v-else-if="wizard.currentStep.value === 'details'" class="space-y-4">
      <p class="text-sm text-gray-600">
        Specify where to save the files. Only the destination path is required.
      </p>

      <!-- Selected Item Summary -->
      <div class="bg-blue-50 rounded-lg p-3 flex gap-3 items-center">
        <div class="w-10 h-15 flex-shrink-0 bg-gray-200 rounded overflow-hidden">
          <template v-if="videoMode === 'movie' && wizard.selectedMovie.value">
            <img
              v-if="wizard.selectedMovie.value.poster_path"
              :src="`${tmdbImageBase}${wizard.selectedMovie.value.poster_path}`"
              class="w-full h-full object-cover"
            />
            <div v-else class="w-full h-full flex items-center justify-center text-gray-400">
              <span class="i-carbon-media-library"></span>
            </div>
          </template>
          <template v-else-if="wizard.selectedSeries.value">
            <img
              v-if="wizard.selectedSeries.value.poster_path"
              :src="`${tmdbImageBase}${wizard.selectedSeries.value.poster_path}`"
              class="w-full h-full object-cover"
            />
            <div v-else class="w-full h-full flex items-center justify-center text-gray-400">
              <span class="i-carbon-tv"></span>
            </div>
          </template>
        </div>
        <div class="min-w-0">
          <template v-if="videoMode === 'movie' && wizard.selectedMovie.value">
            <div class="font-medium text-gray-900 truncate">{{ wizard.selectedMovie.value.title }}</div>
            <div class="text-sm text-gray-600">{{ formatYear(wizard.selectedMovie.value.release_date) }}</div>
          </template>
          <template v-else-if="wizard.selectedSeries.value && wizard.selectedSeason.value">
            <div class="font-medium text-gray-900 truncate">{{ wizard.selectedSeries.value.name }}</div>
            <div class="text-sm text-gray-600">
              Season {{ wizard.selectedSeason.value.season_number }} &middot;
              {{ wizard.selectedSeason.value.episode_count }} episodes
            </div>
          </template>
        </div>
      </div>

      <!-- Description (optional) -->
      <div>
        <label for="description" class="block text-sm font-medium text-gray-700 mb-1">
          Description <span class="text-gray-400 font-normal">(optional)</span>
        </label>
        <textarea
          id="description"
          v-model="wizard.description.value"
          class="input w-full"
          rows="2"
          placeholder="Additional notes or search hints..."
        ></textarea>
        <p class="text-xs text-gray-500 mt-1">
          Auto-generated from selection if left empty
        </p>
      </div>

      <!-- Tags -->
      <div>
        <label for="tags" class="block text-sm font-medium text-gray-700 mb-1">
          Tags (comma-separated)
        </label>
        <input
          id="tags"
          v-model="wizard.tagsInput.value"
          type="text"
          class="input w-full"
          placeholder="e.g., action, 2024, hdr"
        />
        <p class="text-xs text-gray-500 mt-1">
          "{{ videoMode }}" tag is added automatically
        </p>
      </div>

      <!-- Destination Path -->
      <div>
        <label for="destPath" class="block text-sm font-medium text-gray-700 mb-1">
          Destination Path <span class="text-red-500">*</span>
        </label>
        <input
          id="destPath"
          v-model="wizard.destPath.value"
          type="text"
          class="input w-full"
          :placeholder="videoMode === 'movie' ? '/media/movies/Movie Name (Year)' : '/media/tv/Series Name/Season 01'"
          required
        />
      </div>

      <!-- Priority -->
      <div>
        <label for="priority" class="block text-sm font-medium text-gray-700 mb-1">
          Priority (0-100)
        </label>
        <input
          id="priority"
          v-model.number="wizard.priority.value"
          type="number"
          min="0"
          max="100"
          class="input w-32"
        />
        <p class="text-xs text-gray-500 mt-1">Higher priority tickets are processed first</p>
      </div>
    </div>

    <!-- Step 4: Review -->
    <div v-else-if="wizard.currentStep.value === 'review'" class="space-y-4">
      <p class="text-sm text-gray-600">Review your ticket before creating it.</p>

      <!-- Content Info -->
      <div class="bg-gray-50 rounded-lg p-4">
        <h3 class="font-medium text-gray-900 flex items-center gap-2">
          <span :class="videoMode === 'movie' ? 'i-carbon-media-library' : 'i-carbon-tv'"></span>
          {{ videoMode === 'movie' ? 'Movie' : 'TV Series' }}
        </h3>
        <div class="mt-2 text-sm">
          <template v-if="videoMode === 'movie' && wizard.selectedMovie.value">
            <div class="font-medium">{{ wizard.selectedMovie.value.title }}</div>
            <div class="text-gray-600">{{ formatYear(wizard.selectedMovie.value.release_date) }}</div>
            <div class="text-gray-500 text-xs mt-1">
              <span v-if="wizard.selectedMovie.value.runtime_minutes">
                {{ wizard.selectedMovie.value.runtime_minutes }} min
              </span>
            </div>
          </template>
          <template v-else-if="wizard.selectedSeries.value && wizard.selectedSeason.value">
            <div class="font-medium">{{ wizard.selectedSeries.value.name }}</div>
            <div class="text-gray-600">
              Season {{ wizard.selectedSeason.value.season_number }}
            </div>
            <div class="text-gray-500 text-xs mt-1">
              {{ wizard.selectedSeason.value.episode_count }} episodes
            </div>
          </template>
        </div>
      </div>

      <!-- Quality Preferences -->
      <div v-if="wizard.searchConstraints.value?.video" class="bg-gray-50 rounded-lg p-4">
        <h3 class="font-medium text-gray-900 flex items-center gap-2">
          <span class="i-carbon-settings"></span>
          Quality Preferences
        </h3>
        <div class="mt-2 text-sm text-gray-600 space-y-1">
          <div v-if="wizard.videoConstraints.value.preferred_resolution">
            Preferred: {{ wizard.videoConstraints.value.preferred_resolution.replace('r', '') }}
          </div>
          <div v-if="wizard.videoConstraints.value.min_resolution">
            Minimum: {{ wizard.videoConstraints.value.min_resolution.replace('r', '') }}
          </div>
          <div v-if="wizard.videoConstraints.value.preferred_sources?.length">
            Sources: {{ wizard.videoConstraints.value.preferred_sources.map(s => s.replace('_', '-').toUpperCase()).join(', ') }}
          </div>
          <div v-if="wizard.videoConstraints.value.preferred_codecs?.length">
            Codecs: {{ wizard.videoConstraints.value.preferred_codecs.join(', ').toUpperCase() }}
          </div>
          <div v-if="wizard.videoConstraints.value.audio_languages?.length">
            Audio: {{ wizard.videoConstraints.value.audio_languages.map(l => `${getLanguageLabel(l.code)} (${l.priority})`).join(', ') }}
          </div>
          <div v-if="wizard.videoConstraints.value.subtitle_languages?.length">
            Subtitles: {{ wizard.videoConstraints.value.subtitle_languages.map(l => `${getLanguageLabel(l.code)} (${l.priority})`).join(', ') }}
          </div>
          <div v-if="wizard.videoConstraints.value.exclude_hardcoded_subs">
            Excluding hardcoded subs
          </div>
        </div>
      </div>

      <!-- Ticket Details -->
      <div class="bg-gray-50 rounded-lg p-4">
        <h3 class="font-medium text-gray-900 flex items-center gap-2">
          <span class="i-carbon-document"></span>
          Ticket Details
        </h3>
        <div class="mt-2 text-sm space-y-2">
          <div>
            <span class="text-gray-500">Destination:</span>
            <span class="ml-2 font-mono text-xs">{{ wizard.destPath.value || '(not set)' }}</span>
          </div>
          <div>
            <span class="text-gray-500">Priority:</span>
            <span class="ml-2">{{ wizard.priority.value }}</span>
          </div>
          <div v-if="wizard.tags.value.length">
            <span class="text-gray-500">Tags:</span>
            <span class="ml-2">{{ [videoMode, ...wizard.tags.value].join(', ') }}</span>
          </div>
          <div v-if="wizard.description.value">
            <span class="text-gray-500">Description:</span>
            <span class="ml-2">{{ wizard.description.value }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- Navigation Buttons -->
    <div class="flex justify-between mt-6 pt-4 border-t">
      <button
        v-if="wizard.currentStep.value !== 'search'"
        @click="handlePrevStep"
        class="btn-secondary flex items-center gap-2"
      >
        <span class="i-carbon-arrow-left"></span>
        Back
      </button>
      <div v-else></div>

      <div class="flex gap-3">
        <button @click="handleCancel" class="btn-secondary">Cancel</button>

        <button
          v-if="wizard.currentStep.value !== 'review'"
          @click="wizard.nextStep()"
          class="btn-primary flex items-center gap-2"
          :disabled="!canProceedVideo"
        >
          Next
          <span class="i-carbon-arrow-right"></span>
        </button>

        <button
          v-else
          @click="handleSubmit"
          class="btn-primary"
          :disabled="!wizard.destPath.value.trim()"
        >
          <span class="i-carbon-add"></span>
          Create Ticket
        </button>
      </div>
    </div>
  </div>
</template>
