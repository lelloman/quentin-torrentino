<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import {
  getOrchestratorStatus,
  startOrchestrator,
  stopOrchestrator,
  type OrchestratorStatus,
} from '../../api/orchestrator'
import Badge from '../common/Badge.vue'
import LoadingSpinner from '../common/LoadingSpinner.vue'

const status = ref<OrchestratorStatus | null>(null)
const loading = ref(true)
const error = ref<string | null>(null)
const actionLoading = ref(false)

let pollInterval: ReturnType<typeof setInterval> | null = null

const isAvailable = computed(() => status.value?.available ?? false)
const isRunning = computed(() => status.value?.running ?? false)

async function fetchStatus() {
  try {
    status.value = await getOrchestratorStatus()
    error.value = null
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Failed to fetch status'
  } finally {
    loading.value = false
  }
}

async function handleStart() {
  actionLoading.value = true
  try {
    await startOrchestrator()
    await fetchStatus()
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Failed to start orchestrator'
  } finally {
    actionLoading.value = false
  }
}

async function handleStop() {
  actionLoading.value = true
  try {
    await stopOrchestrator()
    await fetchStatus()
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Failed to stop orchestrator'
  } finally {
    actionLoading.value = false
  }
}

onMounted(() => {
  fetchStatus()
  // Poll every 5 seconds
  pollInterval = setInterval(fetchStatus, 5000)
})

onUnmounted(() => {
  if (pollInterval) {
    clearInterval(pollInterval)
  }
})
</script>

<template>
  <div class="card">
    <div class="flex items-center justify-between mb-4">
      <h3 class="text-lg font-semibold">Orchestrator</h3>
      <div v-if="loading">
        <LoadingSpinner size="sm" />
      </div>
      <Badge v-else-if="!isAvailable" variant="warning">Not Available</Badge>
      <Badge v-else-if="isRunning" variant="success">Running</Badge>
      <Badge v-else variant="default">Stopped</Badge>
    </div>

    <div v-if="error" class="text-red-600 text-sm mb-4">
      {{ error }}
    </div>

    <div v-if="!loading && status" class="space-y-4">
      <!-- Stats Grid -->
      <div class="grid grid-cols-2 gap-3">
        <div class="bg-gray-50 rounded-lg p-3">
          <p class="text-xs text-gray-500 uppercase tracking-wide">Pending</p>
          <p class="text-2xl font-bold text-blue-600">{{ status.pending_count }}</p>
        </div>
        <div class="bg-gray-50 rounded-lg p-3">
          <p class="text-xs text-gray-500 uppercase tracking-wide">Acquiring</p>
          <p class="text-2xl font-bold text-purple-600">{{ status.acquiring_count }}</p>
        </div>
        <div class="bg-gray-50 rounded-lg p-3">
          <p class="text-xs text-gray-500 uppercase tracking-wide">Needs Approval</p>
          <p class="text-2xl font-bold text-orange-600">{{ status.needs_approval_count }}</p>
        </div>
        <div class="bg-gray-50 rounded-lg p-3">
          <p class="text-xs text-gray-500 uppercase tracking-wide">Downloading</p>
          <p class="text-2xl font-bold text-green-600">{{ status.downloading_count }}</p>
        </div>
      </div>

      <!-- Active Downloads Detail -->
      <div v-if="status.active_downloads > 0" class="text-sm text-gray-600">
        <span class="font-medium">{{ status.active_downloads }}</span> active download(s) being tracked
      </div>

      <!-- Control Buttons -->
      <div v-if="isAvailable" class="flex gap-2 pt-2">
        <button
          v-if="!isRunning"
          @click="handleStart"
          :disabled="actionLoading"
          class="btn-primary flex-1"
        >
          <span v-if="actionLoading">Starting...</span>
          <span v-else>Start</span>
        </button>
        <button
          v-else
          @click="handleStop"
          :disabled="actionLoading"
          class="btn-secondary flex-1"
        >
          <span v-if="actionLoading">Stopping...</span>
          <span v-else>Stop</span>
        </button>
      </div>

      <!-- Not Available Message -->
      <div v-if="!isAvailable" class="text-sm text-gray-500 bg-gray-50 rounded-lg p-3">
        <p class="font-medium text-gray-700 mb-1">Orchestrator not available</p>
        <p>Configure <code class="bg-gray-200 px-1 rounded">searcher</code> and <code class="bg-gray-200 px-1 rounded">torrent_client</code> in config.toml, then set <code class="bg-gray-200 px-1 rounded">[orchestrator] enabled = true</code>.</p>
      </div>
    </div>
  </div>
</template>
