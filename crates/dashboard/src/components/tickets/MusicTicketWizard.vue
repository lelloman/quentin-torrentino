<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useTicketWizard } from '../../composables/useTicketWizard'
import LoadingSpinner from '../common/LoadingSpinner.vue'
import ErrorAlert from '../common/ErrorAlert.vue'
import type { CreateTicketWithCatalogRequest, AudioFormat } from '../../api/types'

const emit = defineEmits<{
  submit: [request: CreateTicketWithCatalogRequest]
  cancel: []
}>()

const wizard = useTicketWizard()

// Initialize content type to album for music wizard and skip to search step
wizard.setContentType('album')
wizard.goToStep('search')

// Track whether a search has been performed
const hasSearched = ref(false)

// Audio format options for output conversion
const audioFormatOptions: { value: AudioFormat; label: string; lossless: boolean }[] = [
  { value: 'flac', label: 'FLAC', lossless: true },
  { value: 'alac', label: 'ALAC', lossless: true },
  { value: 'wav', label: 'WAV', lossless: true },
  { value: 'ogg_vorbis', label: 'Ogg Vorbis', lossless: false },
  { value: 'opus', label: 'Opus', lossless: false },
  { value: 'mp3', label: 'MP3', lossless: false },
  { value: 'aac', label: 'AAC', lossless: false },
]

const isLosslessFormat = computed(() => {
  const format = audioFormatOptions.find((f) => f.value === wizard.audioFormat.value)
  return format?.lossless ?? false
})

// Clear bitrate when switching to lossless format
watch(wizard.audioFormat, (newFormat) => {
  const format = audioFormatOptions.find((f) => f.value === newFormat)
  if (format?.lossless) {
    wizard.audioBitrate.value = undefined
  } else if (wizard.audioBitrate.value === undefined) {
    wizard.audioBitrate.value = 320
  }
})

// Format duration in mm:ss
function formatDuration(ms?: number): string {
  if (!ms) return '--:--'
  const totalSecs = Math.round(ms / 1000)
  const mins = Math.floor(totalSecs / 60)
  const secs = totalSecs % 60
  return `${mins}:${secs.toString().padStart(2, '0')}`
}

// Format total duration as human-readable
function formatTotalDuration(ms?: number): string {
  if (!ms) return ''
  const totalMins = Math.round(ms / 60000)
  if (totalMins < 60) {
    return `${totalMins} min`
  }
  const hours = Math.floor(totalMins / 60)
  const mins = totalMins % 60
  return `${hours}h ${mins}m`
}

// Search handler
async function handleSearch() {
  if (!wizard.searchQuery.value.trim()) return
  hasSearched.value = true
  await wizard.performSearch()
}

// Select a release from search results
async function handleSelectRelease(mbid: string) {
  await wizard.selectRelease(mbid)
}

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

// Handle previous step - prevent going back to 'type' step
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
  { key: 'constraints', label: 'Preferences' },
  { key: 'details', label: 'Details' },
  { key: 'review', label: 'Review' },
] as const

const currentStepIdx = computed(() => {
  const idx = steps.findIndex((s) => s.key === wizard.currentStep.value)
  return idx >= 0 ? idx : 0
})
</script>

<template>
  <div class="card">
    <!-- Header -->
    <div class="flex items-center justify-between mb-6">
      <h2 class="text-lg font-semibold flex items-center gap-2">
        <span class="i-carbon-music text-xl"></span>
        New Music Ticket
      </h2>
      <button @click="handleCancel" class="text-gray-400 hover:text-gray-600">
        <span class="i-carbon-close text-xl"></span>
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

    <!-- MusicBrainz Not Available Warning -->
    <div
      v-if="wizard.catalogStatus.value && !wizard.catalogStatus.value.musicbrainz_available"
      class="bg-yellow-50 border border-yellow-200 rounded-lg p-4 mb-4"
    >
      <div class="flex items-start gap-3">
        <span class="i-carbon-warning text-yellow-600 text-xl"></span>
        <div>
          <p class="text-sm font-medium text-yellow-800">MusicBrainz Not Configured</p>
          <p class="text-sm text-yellow-700 mt-1">
            The MusicBrainz catalog is not available. You can still create tickets manually using
            the simple form.
          </p>
        </div>
      </div>
    </div>

    <!-- Step 1: Search & Select -->
    <div v-if="wizard.currentStep.value === 'search'" class="space-y-4">
      <p class="text-sm text-gray-600">
        Search MusicBrainz for the album you want to download.
      </p>

      <!-- Search Form -->
      <form @submit.prevent="handleSearch" class="flex gap-2">
        <input
          v-model="wizard.searchQuery.value"
          type="text"
          class="input flex-1"
          placeholder="Artist name, album title..."
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

      <!-- Search Results -->
      <div
        v-else-if="wizard.musicBrainzResults.value.length > 0"
        class="border rounded-lg divide-y max-h-96 overflow-y-auto"
      >
        <button
          v-for="release in wizard.musicBrainzResults.value"
          :key="release.mbid"
          @click="handleSelectRelease(release.mbid)"
          class="w-full p-3 text-left hover:bg-gray-50 transition-colors flex gap-3"
          :class="{
            'bg-blue-50 border-l-4 border-l-blue-600': wizard.selectedRelease.value?.mbid === release.mbid,
          }"
        >
          <!-- Cover Art -->
          <div class="w-12 h-12 flex-shrink-0 bg-gray-100 rounded overflow-hidden">
            <img
              v-if="release.cover_art_available"
              :src="`https://coverartarchive.org/release/${release.mbid}/front-250`"
              :alt="release.title"
              class="w-full h-full object-cover"
              loading="lazy"
              @error="(e) => (e.target as HTMLImageElement).style.display = 'none'"
            />
            <div v-else class="w-full h-full flex items-center justify-center text-gray-400">
              <span class="i-carbon-music text-xl"></span>
            </div>
          </div>
          <!-- Info -->
          <div class="flex-1 min-w-0">
            <div class="font-medium text-gray-900 truncate">{{ release.title }}</div>
            <div class="text-sm text-gray-600 truncate">{{ release.artist_credit }}</div>
            <div class="text-xs text-gray-500 mt-1 flex items-center gap-3">
              <span v-if="release.release_date">{{ release.release_date.substring(0, 4) }}</span>
              <span v-if="release.country" class="uppercase">{{ release.country }}</span>
            </div>
          </div>
        </button>
      </div>

      <!-- No Results -->
      <div
        v-else-if="hasSearched && !wizard.catalogLoading.value && wizard.musicBrainzResults.value.length === 0"
        class="text-center py-8 text-gray-500"
      >
        <span class="i-carbon-search text-4xl mb-2 block"></span>
        <p>No results found. Try a different search term.</p>
      </div>

      <!-- Selected Release Details -->
      <div v-if="wizard.selectedRelease.value" class="bg-blue-50 rounded-lg p-4 mt-4">
        <div class="flex items-start justify-between">
          <div>
            <h3 class="font-semibold text-gray-900">{{ wizard.selectedRelease.value.title }}</h3>
            <p class="text-sm text-gray-700">{{ wizard.selectedRelease.value.artist_credit }}</p>
          </div>
          <button
            @click="wizard.selectedRelease.value = null"
            class="text-gray-400 hover:text-gray-600"
          >
            <span class="i-carbon-close"></span>
          </button>
        </div>

        <!-- Track List -->
        <div class="mt-3 max-h-48 overflow-y-auto">
          <table class="w-full text-sm">
            <thead class="text-xs text-gray-500 uppercase">
              <tr>
                <th class="text-left py-1 w-8">#</th>
                <th class="text-left py-1">Title</th>
                <th class="text-right py-1 w-16">Length</th>
              </tr>
            </thead>
            <tbody class="text-gray-700">
              <tr v-for="track in wizard.selectedRelease.value.tracks" :key="track.position">
                <td class="py-1 text-gray-500">{{ track.position }}</td>
                <td class="py-1">{{ track.title }}</td>
                <td class="py-1 text-right text-gray-500">{{ formatDuration(track.length_ms) }}</td>
              </tr>
            </tbody>
          </table>
        </div>

        <div class="mt-2 text-xs text-gray-500 flex items-center gap-3">
          <span>{{ wizard.selectedRelease.value.track_count }} tracks</span>
          <span v-if="wizard.selectedRelease.value.total_length_ms">
            {{ formatTotalDuration(wizard.selectedRelease.value.total_length_ms) }}
          </span>
        </div>
      </div>
    </div>

    <!-- Step 2: Search Constraints -->
    <div v-else-if="wizard.currentStep.value === 'constraints'" class="space-y-6">
      <p class="text-sm text-gray-600">
        Set your preferences for the torrent search. These help find the best quality match.
      </p>

      <!-- Preferred Formats -->
      <div>
        <label class="block text-sm font-medium text-gray-700 mb-2">Preferred Audio Formats</label>
        <div class="flex flex-wrap gap-2">
          <label
            v-for="format in ['flac', 'mp3', 'ogg_vorbis', 'opus', 'aac'] as const"
            :key="format"
            class="flex items-center gap-2 px-3 py-2 border rounded-lg cursor-pointer transition-colors"
            :class="{
              'border-blue-600 bg-blue-50': wizard.audioConstraints.value.preferred_formats?.includes(format),
              'border-gray-200 hover:border-gray-300': !wizard.audioConstraints.value.preferred_formats?.includes(format),
            }"
          >
            <input
              type="checkbox"
              :checked="wizard.audioConstraints.value.preferred_formats?.includes(format)"
              @change="(e) => {
                const formats = wizard.audioConstraints.value.preferred_formats || []
                if ((e.target as HTMLInputElement).checked) {
                  wizard.audioConstraints.value.preferred_formats = [...formats, format]
                } else {
                  wizard.audioConstraints.value.preferred_formats = formats.filter(f => f !== format)
                }
              }"
              class="sr-only"
            />
            <span class="text-sm">{{ format.toUpperCase().replace('_', ' ') }}</span>
          </label>
        </div>
        <p class="text-xs text-gray-500 mt-1">Leave empty to accept any format</p>
      </div>

      <!-- Minimum Bitrate -->
      <div>
        <label class="block text-sm font-medium text-gray-700 mb-2">Minimum Bitrate (for lossy)</label>
        <select
          v-model.number="wizard.audioConstraints.value.min_bitrate_kbps"
          class="input w-48"
        >
          <option :value="undefined">No minimum</option>
          <option :value="128">128 kbps</option>
          <option :value="192">192 kbps</option>
          <option :value="256">256 kbps</option>
          <option :value="320">320 kbps (highest)</option>
        </select>
      </div>

      <!-- Avoid Flags -->
      <div class="space-y-3">
        <label class="flex items-center gap-3 cursor-pointer">
          <input
            type="checkbox"
            v-model="wizard.audioConstraints.value.avoid_compilations"
            class="w-4 h-4 text-blue-600 rounded"
          />
          <span class="text-sm">Avoid compilations / "Best Of" releases</span>
        </label>
        <label class="flex items-center gap-3 cursor-pointer">
          <input
            type="checkbox"
            v-model="wizard.audioConstraints.value.avoid_live"
            class="w-4 h-4 text-blue-600 rounded"
          />
          <span class="text-sm">Avoid live recordings</span>
        </label>
      </div>
    </div>

    <!-- Step 3: Ticket Details -->
    <div v-else-if="wizard.currentStep.value === 'details'" class="space-y-4">
      <p class="text-sm text-gray-600">
        Specify where to save the files. Only the destination path is required.
      </p>

      <!-- Selected Album Summary -->
      <div v-if="wizard.selectedRelease.value" class="bg-blue-50 rounded-lg p-3 flex gap-3 items-center">
        <div class="w-10 h-10 flex-shrink-0 bg-gray-200 rounded overflow-hidden">
          <img
            v-if="wizard.selectedRelease.value.cover_art_available"
            :src="`https://coverartarchive.org/release/${wizard.selectedRelease.value.mbid}/front-250`"
            class="w-full h-full object-cover"
          />
          <div v-else class="w-full h-full flex items-center justify-center text-gray-400">
            <span class="i-carbon-music"></span>
          </div>
        </div>
        <div class="min-w-0">
          <div class="font-medium text-gray-900 truncate">{{ wizard.selectedRelease.value.title }}</div>
          <div class="text-sm text-gray-600 truncate">{{ wizard.selectedRelease.value.artist_credit }}</div>
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
          Auto-generated from album if left empty
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
          placeholder="e.g., rock, 2024, vinyl-rip"
        />
        <p class="text-xs text-gray-500 mt-1">
          "music" tag is added automatically
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
          placeholder="/media/music/Artist/Album"
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

      <!-- Output Format -->
      <div class="border-t pt-4">
        <label class="block text-sm font-medium text-gray-700 mb-2">Output Format</label>
        <div class="flex gap-4 mb-3">
          <label class="flex items-center gap-2 cursor-pointer">
            <input
              type="radio"
              v-model="wizard.outputType.value"
              value="original"
              class="w-4 h-4 text-blue-600"
            />
            <span class="text-sm">Keep Original</span>
          </label>
          <label class="flex items-center gap-2 cursor-pointer">
            <input
              type="radio"
              v-model="wizard.outputType.value"
              value="audio"
              class="w-4 h-4 text-blue-600"
            />
            <span class="text-sm">Convert Audio</span>
          </label>
        </div>

        <!-- Audio format options -->
        <div v-if="wizard.outputType.value === 'audio'" class="bg-gray-50 p-3 rounded-lg space-y-3">
          <div class="flex gap-4 items-center">
            <div class="flex-1">
              <label for="audioFormat" class="block text-xs font-medium text-gray-600 mb-1">
                Format
              </label>
              <select id="audioFormat" v-model="wizard.audioFormat.value" class="input w-full text-sm">
                <optgroup label="Lossless">
                  <option
                    v-for="opt in audioFormatOptions.filter((o) => o.lossless)"
                    :key="opt.value"
                    :value="opt.value"
                  >
                    {{ opt.label }}
                  </option>
                </optgroup>
                <optgroup label="Lossy">
                  <option
                    v-for="opt in audioFormatOptions.filter((o) => !o.lossless)"
                    :key="opt.value"
                    :value="opt.value"
                  >
                    {{ opt.label }}
                  </option>
                </optgroup>
              </select>
            </div>
            <div class="w-32" v-if="!isLosslessFormat">
              <label for="audioBitrate" class="block text-xs font-medium text-gray-600 mb-1">
                Bitrate (kbps)
              </label>
              <select
                id="audioBitrate"
                v-model.number="wizard.audioBitrate.value"
                class="input w-full text-sm"
              >
                <option :value="128">128</option>
                <option :value="192">192</option>
                <option :value="256">256</option>
                <option :value="320">320</option>
              </select>
            </div>
          </div>
          <p class="text-xs text-gray-500">
            <template v-if="isLosslessFormat">Lossless format - no quality loss</template>
            <template v-else>Lossy compression at {{ wizard.audioBitrate.value }} kbps</template>
          </p>
        </div>

        <p v-else class="text-xs text-gray-500">Files will be placed as-is without conversion</p>
      </div>
    </div>

    <!-- Step 4: Review -->
    <div v-else-if="wizard.currentStep.value === 'review'" class="space-y-4">
      <p class="text-sm text-gray-600">Review your ticket before creating it.</p>

      <!-- Album Info -->
      <div class="bg-gray-50 rounded-lg p-4">
        <h3 class="font-medium text-gray-900 flex items-center gap-2">
          <span class="i-carbon-music"></span>
          Album
        </h3>
        <div class="mt-2 text-sm">
          <div class="font-medium">{{ wizard.selectedRelease.value?.title }}</div>
          <div class="text-gray-600">{{ wizard.selectedRelease.value?.artist_credit }}</div>
          <div class="text-gray-500 text-xs mt-1">
            {{ wizard.selectedRelease.value?.track_count }} tracks
            <span v-if="wizard.selectedRelease.value?.release_date">
              &middot; {{ wizard.selectedRelease.value.release_date.substring(0, 4) }}
            </span>
          </div>
        </div>
      </div>

      <!-- Search Preferences -->
      <div v-if="wizard.searchConstraints.value?.audio" class="bg-gray-50 rounded-lg p-4">
        <h3 class="font-medium text-gray-900 flex items-center gap-2">
          <span class="i-carbon-settings"></span>
          Search Preferences
        </h3>
        <div class="mt-2 text-sm text-gray-600 space-y-1">
          <div v-if="wizard.audioConstraints.value.preferred_formats?.length">
            Preferred: {{ wizard.audioConstraints.value.preferred_formats.join(', ').toUpperCase() }}
          </div>
          <div v-if="wizard.audioConstraints.value.min_bitrate_kbps">
            Min bitrate: {{ wizard.audioConstraints.value.min_bitrate_kbps }} kbps
          </div>
          <div v-if="wizard.audioConstraints.value.avoid_compilations">
            Avoiding compilations
          </div>
          <div v-if="wizard.audioConstraints.value.avoid_live">Avoiding live recordings</div>
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
            <span class="ml-2">{{ ['music', ...wizard.tags.value].join(', ') }}</span>
          </div>
          <div v-if="wizard.description.value">
            <span class="text-gray-500">Description:</span>
            <span class="ml-2">{{ wizard.description.value }}</span>
          </div>
        </div>
      </div>

      <!-- Output Format -->
      <div class="bg-gray-50 rounded-lg p-4">
        <h3 class="font-medium text-gray-900 flex items-center gap-2">
          <span class="i-carbon-export"></span>
          Output Format
        </h3>
        <div class="mt-2 text-sm text-gray-600">
          <template v-if="wizard.outputType.value === 'original'">
            Keep original format (no conversion)
          </template>
          <template v-else>
            Convert to {{ wizard.audioFormat.value.toUpperCase().replace('_', ' ') }}
            <template v-if="!isLosslessFormat"> at {{ wizard.audioBitrate.value }} kbps</template>
          </template>
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
          :disabled="!wizard.canProceed.value"
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
