<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import type { CreateTicketRequest, OutputConstraints, AudioFormat } from '../../api/types'

const emit = defineEmits<{
  submit: [request: CreateTicketRequest]
  cancel: []
}>()

const description = ref('')
const tagsInput = ref('')
const destPath = ref('')
const priority = ref(0)

// Output constraints
const outputType = ref<'original' | 'audio'>('original')
const audioFormat = ref<AudioFormat>('ogg_vorbis')
const audioBitrate = ref<number | undefined>(320)

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
  audioFormat.value = 'ogg_vorbis'
  audioBitrate.value = 320
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
