<script setup lang="ts">
import { ref, computed } from 'vue'
import type { IndexerStatus, SearchCategory, SearchMode, SearchRequest } from '../../api/types'

defineProps<{
  indexers: IndexerStatus[]
  isSearching?: boolean
}>()

const emit = defineEmits<{
  search: [request: SearchRequest]
}>()

const query = ref('')
const selectedIndexers = ref<string[]>([])
const selectedCategories = ref<SearchCategory[]>([])
const limit = ref<number | undefined>(undefined)
const searchMode = ref<SearchMode>('both')

const categories: { value: SearchCategory; label: string }[] = [
  { value: 'movies', label: 'Movies' },
  { value: 'tv', label: 'TV' },
  { value: 'music', label: 'Music' },
  { value: 'audio', label: 'Audio' },
  { value: 'books', label: 'Books' },
  { value: 'software', label: 'Software' },
  { value: 'other', label: 'Other' },
]

const searchModes: { value: SearchMode; label: string }[] = [
  { value: 'both', label: 'Both (cache + external)' },
  { value: 'cache_only', label: 'Cache only' },
  { value: 'external_only', label: 'External only' },
]

const isValid = computed(() => query.value.trim().length > 0)

function handleSubmit() {
  if (!isValid.value) return

  const request: SearchRequest = {
    query: query.value.trim(),
    mode: searchMode.value,
  }

  if (selectedIndexers.value.length > 0) {
    request.indexers = selectedIndexers.value
  }

  if (selectedCategories.value.length > 0) {
    request.categories = selectedCategories.value
  }

  if (limit.value !== undefined && limit.value > 0) {
    request.limit = limit.value
  }

  emit('search', request)
}

function toggleIndexer(name: string) {
  const index = selectedIndexers.value.indexOf(name)
  if (index === -1) {
    selectedIndexers.value.push(name)
  } else {
    selectedIndexers.value.splice(index, 1)
  }
}

function toggleCategory(cat: SearchCategory) {
  const index = selectedCategories.value.indexOf(cat)
  if (index === -1) {
    selectedCategories.value.push(cat)
  } else {
    selectedCategories.value.splice(index, 1)
  }
}

function clearForm() {
  query.value = ''
  selectedIndexers.value = []
  selectedCategories.value = []
  limit.value = undefined
  searchMode.value = 'both'
}
</script>

<template>
  <form @submit.prevent="handleSubmit" class="card space-y-4">
    <div>
      <label for="query" class="block text-sm font-medium text-gray-700 mb-1">
        Search Query
      </label>
      <div class="flex gap-2">
        <input
          id="query"
          v-model="query"
          type="text"
          class="input flex-1"
          placeholder="Enter search query..."
          :disabled="isSearching"
          required
        />
        <button
          type="submit"
          class="btn-primary"
          :disabled="!isValid || isSearching"
        >
          <span v-if="isSearching" class="i-carbon-rotate animate-spin mr-1"></span>
          {{ isSearching ? 'Searching...' : 'Search' }}
        </button>
        <button
          type="button"
          @click="clearForm"
          class="btn-secondary"
          :disabled="isSearching"
        >
          Clear
        </button>
      </div>
    </div>

    <div v-if="indexers.length > 0">
      <label class="block text-sm font-medium text-gray-700 mb-2">
        Indexers (leave empty for all)
      </label>
      <div class="flex flex-wrap gap-2">
        <button
          v-for="indexer in indexers"
          :key="indexer.name"
          type="button"
          @click="toggleIndexer(indexer.name)"
          class="px-3 py-1 text-sm rounded-full border transition-colors"
          :class="selectedIndexers.includes(indexer.name)
            ? 'bg-primary text-white border-primary'
            : 'bg-white text-gray-700 border-gray-300 hover:border-primary'"
          :disabled="isSearching"
        >
          {{ indexer.name }}
        </button>
      </div>
    </div>

    <div>
      <label class="block text-sm font-medium text-gray-700 mb-2">
        Categories (leave empty for all)
      </label>
      <div class="flex flex-wrap gap-2">
        <button
          v-for="cat in categories"
          :key="cat.value"
          type="button"
          @click="toggleCategory(cat.value)"
          class="px-3 py-1 text-sm rounded-full border transition-colors"
          :class="selectedCategories.includes(cat.value)
            ? 'bg-primary text-white border-primary'
            : 'bg-white text-gray-700 border-gray-300 hover:border-primary'"
          :disabled="isSearching"
        >
          {{ cat.label }}
        </button>
      </div>
    </div>

    <div class="flex gap-6">
      <div>
        <label for="limit" class="block text-sm font-medium text-gray-700 mb-1">
          Result Limit (optional)
        </label>
        <input
          id="limit"
          v-model.number="limit"
          type="number"
          min="1"
          max="500"
          class="input w-32"
          placeholder="No limit"
          :disabled="isSearching"
        />
      </div>

      <div>
        <label for="mode" class="block text-sm font-medium text-gray-700 mb-1">
          Search Mode
        </label>
        <select
          id="mode"
          v-model="searchMode"
          class="input w-48"
          :disabled="isSearching"
        >
          <option v-for="mode in searchModes" :key="mode.value" :value="mode.value">
            {{ mode.label }}
          </option>
        </select>
      </div>
    </div>
  </form>
</template>
