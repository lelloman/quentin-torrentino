<script setup lang="ts">
import { onMounted } from 'vue'
import { storeToRefs } from 'pinia'
import { useAppStore } from '../stores/app'
import LoadingSpinner from '../components/common/LoadingSpinner.vue'
import ErrorAlert from '../components/common/ErrorAlert.vue'
import Badge from '../components/common/Badge.vue'

const store = useAppStore()
const { health, healthLoading, healthError } = storeToRefs(store)

onMounted(() => {
  store.fetchHealth()
})
</script>

<template>
  <div>
    <div class="flex items-center justify-between mb-6">
      <h1 class="text-2xl font-bold">Health Status</h1>
      <button
        @click="store.fetchHealth()"
        class="btn-secondary"
        :disabled="healthLoading"
      >
        Refresh
      </button>
    </div>

    <div v-if="healthLoading" class="flex justify-center py-12">
      <LoadingSpinner size="lg" />
    </div>

    <ErrorAlert
      v-else-if="healthError"
      :message="healthError"
      @dismiss="healthError = null"
    />

    <div v-else-if="health" class="card">
      <h2 class="text-lg font-semibold mb-4">System Status</h2>
      <div class="space-y-3">
        <div class="flex items-center justify-between py-2 border-b border-gray-100">
          <span class="text-gray-600">Status</span>
          <Badge :variant="health.status === 'ok' ? 'success' : 'danger'">
            {{ health.status }}
          </Badge>
        </div>
      </div>
    </div>
  </div>
</template>
