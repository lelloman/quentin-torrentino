<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import type { CreateTicketRequest, OutputConstraints, AudioFormat, VideoCodec, VideoContainer } from '../../api/types'

const emit = defineEmits<{
  submit: [request: CreateTicketRequest]
  cancel: []
}>()

const description = ref('')
const tagsInput = ref('')
const destPath = ref('')
const priority = ref(0)

// Output constraints
const outputType = ref<'original' | 'audio' | 'video'>('original')
// Audio output options
const audioFormat = ref<AudioFormat>('ogg_vorbis')
const audioBitrate = ref<number | undefined>(320)
// Video output options
const videoFormat = ref<VideoCodec>('h264')
const videoContainer = ref<VideoContainer>('mp4')
const videoQualityMode = ref<'crf' | 'bitrate'>('bitrate')
const videoCrf = ref<number>(23)
const videoBitrateKbps = ref<number>(5000)
const videoMaxHeight = ref<number | undefined>(undefined)

const tags = computed(() => {
  return tagsInput.value
    .split(',')
    .map((t) => t.trim())
    .filter((t) => t.length > 0)
})

const isValid = computed(() => {
  return description.value.trim().length > 0 && destPath.value.trim().length > 0
})

const outputConstraints = computed((): OutputConstraints | undefined => {
  if (outputType.value === 'original') {
    return undefined // No conversion needed
  }
  if (outputType.value === 'audio') {
    return {
      type: 'audio',
      format: audioFormat.value,
      bitrate_kbps: audioBitrate.value,
    }
  }
  if (outputType.value === 'video') {
    return {
      type: 'video',
      format: videoFormat.value,
      container: videoContainer.value,
      crf: videoQualityMode.value === 'crf' ? videoCrf.value : undefined,
      bitrate_kbps: videoQualityMode.value === 'bitrate' ? videoBitrateKbps.value : undefined,
      max_height: videoMaxHeight.value,
    }
  }
  return undefined
})

// Audio format options
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
  const format = audioFormatOptions.find((f) => f.value === audioFormat.value)
  return format?.lossless ?? false
})

// Clear bitrate when switching to lossless format
watch(audioFormat, (newFormat) => {
  const format = audioFormatOptions.find((f) => f.value === newFormat)
  if (format?.lossless) {
    audioBitrate.value = undefined
  } else if (audioBitrate.value === undefined) {
    audioBitrate.value = 320
  }
})

function handleSubmit() {
  if (!isValid.value) return

  emit('submit', {
    priority: priority.value,
    query_context: {
      tags: tags.value,
      description: description.value.trim(),
    },
    dest_path: destPath.value.trim(),
    output_constraints: outputConstraints.value,
  })
}

function handleCancel() {
  description.value = ''
  tagsInput.value = ''
  destPath.value = ''
  priority.value = 0
  outputType.value = 'original'
  // Reset audio options
  audioFormat.value = 'ogg_vorbis'
  audioBitrate.value = 320
  // Reset video options
  videoFormat.value = 'h264'
  videoContainer.value = 'mp4'
  videoQualityMode.value = 'bitrate'
  videoCrf.value = 23
  videoBitrateKbps.value = 5000
  videoMaxHeight.value = undefined
  emit('cancel')
}
</script>

<template>
  <form @submit.prevent="handleSubmit" class="card space-y-4">
    <h2 class="text-lg font-semibold">Create New Ticket</h2>

    <div>
      <label for="description" class="block text-sm font-medium text-gray-700 mb-1">
        Description
      </label>
      <textarea
        id="description"
        v-model="description"
        class="input w-full"
        rows="3"
        placeholder="What are you looking for?"
        required
      ></textarea>
    </div>

    <div>
      <label for="tags" class="block text-sm font-medium text-gray-700 mb-1">
        Tags (comma-separated)
      </label>
      <input
        id="tags"
        v-model="tagsInput"
        type="text"
        class="input w-full"
        placeholder="music, flac, album"
      />
    </div>

    <div>
      <label for="destPath" class="block text-sm font-medium text-gray-700 mb-1">
        Destination Path
      </label>
      <input
        id="destPath"
        v-model="destPath"
        type="text"
        class="input w-full"
        placeholder="/media/downloads/..."
        required
      />
    </div>

    <div>
      <label for="priority" class="block text-sm font-medium text-gray-700 mb-1">
        Priority (0-100)
      </label>
      <input
        id="priority"
        v-model.number="priority"
        type="number"
        min="0"
        max="100"
        class="input w-32"
      />
    </div>

    <!-- Output Constraints -->
    <div class="border-t pt-4">
      <label class="block text-sm font-medium text-gray-700 mb-2">
        Output Format
      </label>
      <div class="flex gap-4 mb-3">
        <label class="flex items-center gap-2 cursor-pointer">
          <input
            type="radio"
            v-model="outputType"
            value="original"
            class="w-4 h-4 text-blue-600"
          />
          <span class="text-sm">Keep Original</span>
        </label>
        <label class="flex items-center gap-2 cursor-pointer">
          <input
            type="radio"
            v-model="outputType"
            value="audio"
            class="w-4 h-4 text-blue-600"
          />
          <span class="text-sm">Convert Audio</span>
        </label>
        <label class="flex items-center gap-2 cursor-pointer">
          <input
            type="radio"
            v-model="outputType"
            value="video"
            class="w-4 h-4 text-blue-600"
          />
          <span class="text-sm">Convert Video</span>
        </label>
      </div>

      <!-- Audio format options (shown when audio conversion selected) -->
      <div v-if="outputType === 'audio'" class="bg-gray-50 p-3 rounded-lg space-y-3">
        <div class="flex gap-4 items-center">
          <div class="flex-1">
            <label for="audioFormat" class="block text-xs font-medium text-gray-600 mb-1">
              Format
            </label>
            <select id="audioFormat" v-model="audioFormat" class="input w-full text-sm">
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
            <select id="audioBitrate" v-model.number="audioBitrate" class="input w-full text-sm">
              <option :value="128">128</option>
              <option :value="192">192</option>
              <option :value="256">256</option>
              <option :value="320">320</option>
            </select>
          </div>
        </div>
        <p class="text-xs text-gray-500">
          <template v-if="isLosslessFormat">
            Lossless format - no quality loss
          </template>
          <template v-else>
            Lossy compression at {{ audioBitrate }} kbps
          </template>
        </p>
      </div>

      <!-- Video format options (shown when video conversion selected) -->
      <div v-else-if="outputType === 'video'" class="bg-gray-50 p-3 rounded-lg space-y-3">
        <div class="grid grid-cols-2 gap-4">
          <div>
            <label for="videoFormat" class="block text-xs font-medium text-gray-600 mb-1">
              Codec
            </label>
            <select id="videoFormat" v-model="videoFormat" class="input w-full text-sm">
              <option value="h264">H.264 (AVC)</option>
              <option value="h265">H.265 (HEVC)</option>
              <option value="vp9">VP9</option>
              <option value="av1">AV1</option>
            </select>
          </div>
          <div>
            <label for="videoContainer" class="block text-xs font-medium text-gray-600 mb-1">
              Container
            </label>
            <select id="videoContainer" v-model="videoContainer" class="input w-full text-sm">
              <option value="mp4">MP4</option>
              <option value="mkv">MKV</option>
              <option value="webm">WebM</option>
            </select>
          </div>
        </div>
        <!-- Quality Mode Toggle -->
        <div>
          <label class="block text-xs font-medium text-gray-600 mb-1">Quality Mode</label>
          <div class="flex gap-3">
            <label class="flex items-center gap-1.5 cursor-pointer">
              <input type="radio" v-model="videoQualityMode" value="bitrate" class="w-3.5 h-3.5" />
              <span class="text-sm">Bitrate</span>
            </label>
            <label class="flex items-center gap-1.5 cursor-pointer">
              <input type="radio" v-model="videoQualityMode" value="crf" class="w-3.5 h-3.5" />
              <span class="text-sm">CRF (variable)</span>
            </label>
          </div>
        </div>
        <div class="grid grid-cols-2 gap-4">
          <!-- Bitrate mode -->
          <div v-if="videoQualityMode === 'bitrate'">
            <label for="videoBitrate" class="block text-xs font-medium text-gray-600 mb-1">
              Bitrate (kbps)
            </label>
            <select id="videoBitrate" v-model.number="videoBitrateKbps" class="input w-full text-sm">
              <option :value="1000">1,000 kbps (~650 MB/1.5h)</option>
              <option :value="2500">2,500 kbps (~1.6 GB/1.5h)</option>
              <option :value="5000">5,000 kbps (~3.3 GB/1.5h)</option>
              <option :value="8000">8,000 kbps (~5.3 GB/1.5h)</option>
              <option :value="10000">10,000 kbps (~6.6 GB/1.5h)</option>
              <option :value="15000">15,000 kbps (~10 GB/1.5h)</option>
              <option :value="20000">20,000 kbps (~13 GB/1.5h)</option>
              <option :value="30000">30,000 kbps (~20 GB/1.5h)</option>
              <option :value="50000">50,000 kbps (~33 GB/1.5h)</option>
            </select>
          </div>
          <!-- CRF mode -->
          <div v-else>
            <label for="videoCrf" class="block text-xs font-medium text-gray-600 mb-1">
              CRF: {{ videoCrf }} ({{ videoCrf <= 18 ? 'high' : videoCrf <= 23 ? 'medium' : 'low' }} quality)
            </label>
            <input
              id="videoCrf"
              type="range"
              v-model.number="videoCrf"
              min="0"
              max="51"
              class="w-full"
            />
            <p class="text-xs text-gray-500">Lower = better quality, larger file</p>
          </div>
          <div>
            <label for="videoMaxHeight" class="block text-xs font-medium text-gray-600 mb-1">
              Max Resolution
            </label>
            <select id="videoMaxHeight" v-model="videoMaxHeight" class="input w-full text-sm">
              <option :value="undefined">Original</option>
              <option :value="2160">4K (2160p)</option>
              <option :value="1080">1080p</option>
              <option :value="720">720p</option>
              <option :value="480">480p</option>
            </select>
          </div>
        </div>
      </div>

      <p v-else class="text-xs text-gray-500">
        Files will be placed as-is without conversion
      </p>
    </div>

    <div class="flex justify-end gap-3 pt-2">
      <button type="button" @click="handleCancel" class="btn-secondary">
        Cancel
      </button>
      <button type="submit" class="btn-primary" :disabled="!isValid">
        Create Ticket
      </button>
    </div>
  </form>
</template>
