<script setup lang="ts">
import { onMounted } from 'vue'
import { storeToRefs } from 'pinia'
import { useAppStore } from '../stores/app'
import LoadingSpinner from '../components/common/LoadingSpinner.vue'
import ErrorAlert from '../components/common/ErrorAlert.vue'

const store = useAppStore()
const { config, configLoading, configError } = storeToRefs(store)

onMounted(() => {
  store.fetchConfig()
})
</script>

<template>
  <div>
    <div class="flex items-center justify-between mb-6">
      <h1 class="text-2xl font-bold">Configuration</h1>
      <button
        @click="store.fetchConfig()"
        class="btn-secondary"
        :disabled="configLoading"
      >
        Refresh
      </button>
    </div>

    <div v-if="configLoading" class="flex justify-center py-12">
      <LoadingSpinner size="lg" />
    </div>

    <ErrorAlert
      v-else-if="configError"
      :message="configError"
      @dismiss="configError = null"
    />

    <div v-else-if="config" class="space-y-4">
      <div class="card">
        <h2 class="text-lg font-semibold mb-4">Authentication</h2>
        <div class="space-y-2">
          <div class="flex items-center justify-between py-2 border-b border-gray-100">
            <span class="text-gray-600">Method</span>
            <span class="font-mono text-sm">{{ config.auth.method }}</span>
          </div>
        </div>
      </div>

      <div class="card">
        <h2 class="text-lg font-semibold mb-4">Server</h2>
        <div class="space-y-2">
          <div class="flex items-center justify-between py-2 border-b border-gray-100">
            <span class="text-gray-600">Host</span>
            <span class="font-mono text-sm">{{ config.server.host }}</span>
          </div>
          <div class="flex items-center justify-between py-2 border-b border-gray-100">
            <span class="text-gray-600">Port</span>
            <span class="font-mono text-sm">{{ config.server.port }}</span>
          </div>
        </div>
      </div>

      <div class="card">
        <h2 class="text-lg font-semibold mb-4">Database</h2>
        <div class="space-y-2">
          <div class="flex items-center justify-between py-2 border-b border-gray-100">
            <span class="text-gray-600">Path</span>
            <span class="font-mono text-sm">{{ config.database.path }}</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
