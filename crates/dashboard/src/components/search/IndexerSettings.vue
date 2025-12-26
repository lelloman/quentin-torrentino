<script setup lang="ts">
import { ref, watch } from 'vue'
import type { IndexerStatus, UpdateIndexerRequest } from '../../api/types'

const props = defineProps<{
  indexer: IndexerStatus | null
  show: boolean
  saving?: boolean
}>()

const emit = defineEmits<{
  close: []
  save: [name: string, request: UpdateIndexerRequest]
}>()

const rateLimitRpm = ref(10)
const enabled = ref(true)

watch(
  () => props.indexer,
  (indexer) => {
    if (indexer) {
      rateLimitRpm.value = indexer.rate_limit.requests_per_minute
      enabled.value = indexer.enabled
    }
  },
  { immediate: true }
)

function handleSave() {
  if (!props.indexer) return

  const request: UpdateIndexerRequest = {}

  if (rateLimitRpm.value !== props.indexer.rate_limit.requests_per_minute) {
    request.rate_limit_rpm = rateLimitRpm.value
  }

  if (enabled.value !== props.indexer.enabled) {
    request.enabled = enabled.value
  }

  // Only emit if there are changes
  if (Object.keys(request).length > 0) {
    emit('save', props.indexer.name, request)
  } else {
    emit('close')
  }
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="show"
      class="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
      @click.self="emit('close')"
    >
      <div class="bg-white rounded-lg shadow-xl w-full max-w-md mx-4">
        <div class="flex items-center justify-between p-4 border-b">
          <h2 class="text-lg font-semibold">
            Edit Indexer: {{ indexer?.name }}
          </h2>
          <button
            @click="emit('close')"
            class="text-gray-400 hover:text-gray-600 transition-colors"
          >
            <span class="i-carbon-close text-xl"></span>
          </button>
        </div>

        <form @submit.prevent="handleSave" class="p-4 space-y-4">
          <div>
            <label class="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                v-model="enabled"
                class="w-4 h-4 rounded border-gray-300"
                :disabled="saving"
              />
              <span class="text-sm font-medium text-gray-700">Enabled</span>
            </label>
            <p class="mt-1 text-xs text-gray-500">
              Disabled indexers will not be queried during searches.
            </p>
          </div>

          <div>
            <label for="rateLimitRpm" class="block text-sm font-medium text-gray-700 mb-1">
              Rate Limit (requests per minute)
            </label>
            <input
              id="rateLimitRpm"
              v-model.number="rateLimitRpm"
              type="number"
              min="1"
              max="1000"
              class="input w-full"
              :disabled="saving"
            />
            <p class="mt-1 text-xs text-gray-500">
              Higher values allow more requests but may overload the indexer.
            </p>
          </div>

          <div class="flex justify-end gap-3 pt-4 border-t">
            <button
              type="button"
              @click="emit('close')"
              class="btn-secondary"
              :disabled="saving"
            >
              Cancel
            </button>
            <button
              type="submit"
              class="btn-primary"
              :disabled="saving"
            >
              {{ saving ? 'Saving...' : 'Save Changes' }}
            </button>
          </div>
        </form>
      </div>
    </div>
  </Teleport>
</template>
