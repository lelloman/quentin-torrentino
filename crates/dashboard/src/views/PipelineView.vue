<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { usePipeline, poolUtilization } from '../composables/usePipeline'
import LoadingSpinner from '../components/common/LoadingSpinner.vue'
import ErrorAlert from '../components/common/ErrorAlert.vue'
import Badge from '../components/common/Badge.vue'

const {
  loading,
  error,
  status,
  converterInfo,
  placerInfo,
  ffmpegValidation,
  isRunning,
  conversionPool,
  placementPool,
  activeJobs,
  ffmpegReady,
  fetchAll,
  checkFfmpeg,
} = usePipeline()

// Auto-refresh interval
const refreshInterval = ref<number | null>(null)
const autoRefresh = ref(true)

function startAutoRefresh() {
  if (refreshInterval.value) return
  refreshInterval.value = window.setInterval(() => {
    if (autoRefresh.value) {
      fetchAll()
    }
  }, 5000)
}

function stopAutoRefresh() {
  if (refreshInterval.value) {
    clearInterval(refreshInterval.value)
    refreshInterval.value = null
  }
}

onMounted(() => {
  fetchAll()
  startAutoRefresh()
})

onUnmounted(() => {
  stopAutoRefresh()
})

function getUtilizationColor(percent: number): string {
  if (percent < 50) return 'bg-green-500'
  if (percent < 80) return 'bg-yellow-500'
  return 'bg-red-500'
}
</script>

<template>
  <div>
    <div class="flex items-center justify-between mb-6">
      <h1 class="text-2xl font-bold">Pipeline Status</h1>
      <div class="flex items-center gap-4">
        <label class="flex items-center gap-2 text-sm text-gray-600">
          <input
            type="checkbox"
            v-model="autoRefresh"
            class="rounded border-gray-300"
          />
          Auto-refresh
        </label>
        <button
          @click="fetchAll()"
          class="btn-secondary"
          :disabled="loading"
        >
          Refresh
        </button>
      </div>
    </div>

    <div v-if="loading && !status" class="flex justify-center py-12">
      <LoadingSpinner size="lg" />
    </div>

    <ErrorAlert
      v-else-if="error"
      :message="error"
    />

    <div v-else class="space-y-6">
      <!-- Pipeline Status Overview -->
      <div class="card">
        <h2 class="text-lg font-semibold mb-4">Pipeline Overview</h2>
        <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
          <div class="text-center p-4 bg-gray-50 rounded-lg">
            <div class="text-3xl font-bold" :class="isRunning ? 'text-green-600' : 'text-gray-400'">
              {{ isRunning ? '●' : '○' }}
            </div>
            <div class="text-sm text-gray-600 mt-1">Pipeline Status</div>
            <Badge :variant="isRunning ? 'success' : 'warning'" class="mt-2">
              {{ isRunning ? 'Running' : 'Stopped' }}
            </Badge>
          </div>
          <div class="text-center p-4 bg-gray-50 rounded-lg">
            <div class="text-3xl font-bold text-primary">{{ activeJobs }}</div>
            <div class="text-sm text-gray-600 mt-1">Active Jobs</div>
          </div>
          <div class="text-center p-4 bg-gray-50 rounded-lg">
            <div class="text-3xl font-bold" :class="ffmpegReady ? 'text-green-600' : 'text-red-600'">
              {{ ffmpegReady ? '✓' : '✗' }}
            </div>
            <div class="text-sm text-gray-600 mt-1">FFmpeg</div>
            <Badge :variant="ffmpegReady ? 'success' : 'danger'" class="mt-2">
              {{ ffmpegReady ? 'Ready' : 'Not Found' }}
            </Badge>
          </div>
          <div class="text-center p-4 bg-gray-50 rounded-lg">
            <div class="text-3xl font-bold text-primary">
              {{ (status?.converting_tickets?.length ?? 0) + (status?.placing_tickets?.length ?? 0) }}
            </div>
            <div class="text-sm text-gray-600 mt-1">Processing Tickets</div>
          </div>
        </div>
        <div v-if="status?.message" class="mt-4 p-3 bg-blue-50 text-blue-700 rounded-md text-sm">
          {{ status.message }}
        </div>
      </div>

      <!-- Pool Status -->
      <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
        <!-- Conversion Pool -->
        <div class="card">
          <h3 class="text-lg font-semibold mb-4">Conversion Pool</h3>
          <div v-if="conversionPool" class="space-y-4">
            <div class="flex justify-between text-sm">
              <span class="text-gray-600">Active / Max</span>
              <span class="font-medium">{{ conversionPool.active_jobs }} / {{ conversionPool.max_concurrent }}</span>
            </div>
            <div class="w-full bg-gray-200 rounded-full h-3">
              <div
                class="h-3 rounded-full transition-all"
                :class="getUtilizationColor(poolUtilization(conversionPool))"
                :style="{ width: `${poolUtilization(conversionPool)}%` }"
              ></div>
            </div>
            <div class="grid grid-cols-2 gap-4 text-sm">
              <div>
                <span class="text-gray-600">Queued:</span>
                <span class="ml-2 font-medium">{{ conversionPool.queued_jobs }}</span>
              </div>
              <div>
                <span class="text-gray-600">Processed:</span>
                <span class="ml-2 font-medium">{{ conversionPool.total_processed }}</span>
              </div>
              <div>
                <span class="text-gray-600">Failed:</span>
                <span class="ml-2 font-medium text-red-600">{{ conversionPool.total_failed }}</span>
              </div>
            </div>
          </div>
          <div v-else class="text-gray-500 text-sm">Pool not initialized</div>
        </div>

        <!-- Placement Pool -->
        <div class="card">
          <h3 class="text-lg font-semibold mb-4">Placement Pool</h3>
          <div v-if="placementPool" class="space-y-4">
            <div class="flex justify-between text-sm">
              <span class="text-gray-600">Active / Max</span>
              <span class="font-medium">{{ placementPool.active_jobs }} / {{ placementPool.max_concurrent }}</span>
            </div>
            <div class="w-full bg-gray-200 rounded-full h-3">
              <div
                class="h-3 rounded-full transition-all"
                :class="getUtilizationColor(poolUtilization(placementPool))"
                :style="{ width: `${poolUtilization(placementPool)}%` }"
              ></div>
            </div>
            <div class="grid grid-cols-2 gap-4 text-sm">
              <div>
                <span class="text-gray-600">Queued:</span>
                <span class="ml-2 font-medium">{{ placementPool.queued_jobs }}</span>
              </div>
              <div>
                <span class="text-gray-600">Processed:</span>
                <span class="ml-2 font-medium">{{ placementPool.total_processed }}</span>
              </div>
              <div>
                <span class="text-gray-600">Failed:</span>
                <span class="ml-2 font-medium text-red-600">{{ placementPool.total_failed }}</span>
              </div>
            </div>
          </div>
          <div v-else class="text-gray-500 text-sm">Pool not initialized</div>
        </div>
      </div>

      <!-- Active Tickets -->
      <div class="card" v-if="(status?.converting_tickets?.length ?? 0) > 0 || (status?.placing_tickets?.length ?? 0) > 0">
        <h3 class="text-lg font-semibold mb-4">Active Processing</h3>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div v-if="(status?.converting_tickets?.length ?? 0) > 0">
            <h4 class="text-sm font-medium text-gray-600 mb-2">Converting</h4>
            <ul class="space-y-1">
              <li
                v-for="ticket in status?.converting_tickets"
                :key="ticket"
                class="text-sm font-mono bg-yellow-50 text-yellow-700 px-2 py-1 rounded"
              >
                {{ ticket }}
              </li>
            </ul>
          </div>
          <div v-if="(status?.placing_tickets?.length ?? 0) > 0">
            <h4 class="text-sm font-medium text-gray-600 mb-2">Placing</h4>
            <ul class="space-y-1">
              <li
                v-for="ticket in status?.placing_tickets"
                :key="ticket"
                class="text-sm font-mono bg-blue-50 text-blue-700 px-2 py-1 rounded"
              >
                {{ ticket }}
              </li>
            </ul>
          </div>
        </div>
      </div>

      <!-- Converter Info -->
      <div class="card" v-if="converterInfo">
        <h3 class="text-lg font-semibold mb-4">Converter Configuration</h3>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
          <div>
            <h4 class="text-sm font-medium text-gray-600 mb-2">Details</h4>
            <div class="space-y-2 text-sm">
              <div class="flex justify-between">
                <span class="text-gray-600">Name:</span>
                <span class="font-medium">{{ converterInfo.name }}</span>
              </div>
              <div class="flex justify-between">
                <span class="text-gray-600">Available:</span>
                <Badge :variant="converterInfo.available ? 'success' : 'danger'">
                  {{ converterInfo.available ? 'Yes' : 'No' }}
                </Badge>
              </div>
              <div class="flex justify-between">
                <span class="text-gray-600">Max Parallel:</span>
                <span class="font-medium">{{ converterInfo.config.max_parallel_conversions }}</span>
              </div>
              <div class="flex justify-between">
                <span class="text-gray-600">Timeout:</span>
                <span class="font-medium">{{ converterInfo.config.timeout_secs }}s</span>
              </div>
            </div>
          </div>
          <div>
            <h4 class="text-sm font-medium text-gray-600 mb-2">Supported Formats</h4>
            <div class="space-y-2">
              <div>
                <span class="text-xs text-gray-500">Input:</span>
                <div class="flex flex-wrap gap-1 mt-1">
                  <span
                    v-for="fmt in converterInfo.supported_input_formats"
                    :key="fmt"
                    class="text-xs bg-gray-100 px-2 py-0.5 rounded"
                  >
                    {{ fmt }}
                  </span>
                </div>
              </div>
              <div>
                <span class="text-xs text-gray-500">Output:</span>
                <div class="flex flex-wrap gap-1 mt-1">
                  <span
                    v-for="fmt in converterInfo.supported_output_formats"
                    :key="fmt"
                    class="text-xs bg-gray-100 px-2 py-0.5 rounded"
                  >
                    {{ fmt }}
                  </span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Placer Info -->
      <div class="card" v-if="placerInfo">
        <h3 class="text-lg font-semibold mb-4">Placer Configuration</h3>
        <div class="space-y-2 text-sm">
          <div class="flex justify-between">
            <span class="text-gray-600">Name:</span>
            <span class="font-medium">{{ placerInfo.name }}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-gray-600">Available:</span>
            <Badge :variant="placerInfo.available ? 'success' : 'danger'">
              {{ placerInfo.available ? 'Yes' : 'No' }}
            </Badge>
          </div>
          <div class="flex justify-between">
            <span class="text-gray-600">Atomic Moves:</span>
            <Badge :variant="placerInfo.config.prefer_atomic_moves ? 'success' : 'default'">
              {{ placerInfo.config.prefer_atomic_moves ? 'Preferred' : 'Disabled' }}
            </Badge>
          </div>
          <div class="flex justify-between">
            <span class="text-gray-600">Verify Checksums:</span>
            <Badge :variant="placerInfo.config.verify_checksums ? 'success' : 'default'">
              {{ placerInfo.config.verify_checksums ? 'Enabled' : 'Disabled' }}
            </Badge>
          </div>
          <div class="flex justify-between">
            <span class="text-gray-600">Max Parallel Operations:</span>
            <span class="font-medium">{{ placerInfo.config.max_parallel_operations }}</span>
          </div>
        </div>
      </div>

      <!-- FFmpeg Validation -->
      <div class="card" v-if="ffmpegValidation">
        <div class="flex items-center justify-between mb-4">
          <h3 class="text-lg font-semibold">FFmpeg Validation</h3>
          <button @click="checkFfmpeg()" class="btn-secondary text-sm" :disabled="loading">
            Re-check
          </button>
        </div>
        <div class="space-y-2 text-sm">
          <div class="flex justify-between">
            <span class="text-gray-600">Status:</span>
            <Badge :variant="ffmpegValidation.valid ? 'success' : 'danger'">
              {{ ffmpegValidation.valid ? 'Valid' : 'Invalid' }}
            </Badge>
          </div>
          <div class="flex justify-between">
            <span class="text-gray-600">FFmpeg:</span>
            <Badge :variant="ffmpegValidation.ffmpeg_available ? 'success' : 'danger'">
              {{ ffmpegValidation.ffmpeg_available ? 'Found' : 'Not Found' }}
            </Badge>
          </div>
          <div class="flex justify-between">
            <span class="text-gray-600">FFprobe:</span>
            <Badge :variant="ffmpegValidation.ffprobe_available ? 'success' : 'danger'">
              {{ ffmpegValidation.ffprobe_available ? 'Found' : 'Not Found' }}
            </Badge>
          </div>
          <div class="mt-3 p-3 rounded-md text-sm" :class="ffmpegValidation.valid ? 'bg-green-50 text-green-700' : 'bg-red-50 text-red-700'">
            {{ ffmpegValidation.message }}
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
