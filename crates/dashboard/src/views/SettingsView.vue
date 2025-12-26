<script setup lang="ts">
import { onMounted } from 'vue'
import { useSearcher } from '../composables/useSearcher'
import IndexerList from '../components/search/IndexerList.vue'
import ErrorAlert from '../components/common/ErrorAlert.vue'
import LoadingSpinner from '../components/common/LoadingSpinner.vue'

const { indexers, status, isLoading, error, fetchIndexers, fetchStatus, clearError } = useSearcher()

onMounted(async () => {
  await Promise.all([fetchStatus(), fetchIndexers()])
})
</script>

<template>
  <div>
    <h1 class="text-2xl font-bold mb-6">Settings</h1>

    <ErrorAlert v-if="error" :message="error" @dismiss="clearError" class="mb-4" />

    <div class="mb-6">
      <h2 class="text-lg font-semibold mb-3">Search Backend</h2>
      <div v-if="status" class="card">
        <div class="grid grid-cols-2 gap-4 text-sm">
          <div>
            <span class="text-gray-500">Backend:</span>
            <span class="ml-2 font-medium">{{ status.backend }}</span>
          </div>
          <div>
            <span class="text-gray-500">Status:</span>
            <span
              class="ml-2 font-medium"
              :class="status.configured ? 'text-green-600' : 'text-red-600'"
            >
              {{ status.configured ? 'Configured' : 'Not Configured' }}
            </span>
          </div>
          <div>
            <span class="text-gray-500">Total Indexers:</span>
            <span class="ml-2 font-medium">{{ status.indexers_count }}</span>
          </div>
          <div>
            <span class="text-gray-500">Enabled Indexers:</span>
            <span class="ml-2 font-medium">{{ status.indexers_enabled }}</span>
          </div>
        </div>
      </div>
      <div v-else-if="isLoading" class="flex justify-center py-4">
        <LoadingSpinner />
      </div>
    </div>

    <div>
      <h2 class="text-lg font-semibold mb-3">Indexers</h2>
      <p class="text-sm text-gray-500 mb-3">
        Indexers are configured in Jackett. Changes must be made there.
      </p>
      <div v-if="isLoading && indexers.length === 0" class="flex justify-center py-4">
        <LoadingSpinner />
      </div>
      <IndexerList v-else :indexers="indexers" :loading="isLoading" />
    </div>
  </div>
</template>
