// Pipeline composable for state management

import { ref, computed } from 'vue'
import type {
  PipelineStatus,
  ConverterInfo,
  PlacerInfo,
  FfmpegValidation,
  PoolStatus,
  TicketProgress,
  ProcessTicketRequest,
  ProcessTicketResponse,
} from '../api/pipeline'
import {
  getPipelineStatus,
  getConverterInfo,
  getPlacerInfo,
  validateFfmpeg,
  processTicket as apiProcessTicket,
  getTicketProgress,
} from '../api/pipeline'

export function usePipeline() {
  const loading = ref(false)
  const error = ref<string | null>(null)
  const status = ref<PipelineStatus | null>(null)
  const converterInfo = ref<ConverterInfo | null>(null)
  const placerInfo = ref<PlacerInfo | null>(null)
  const ffmpegValidation = ref<FfmpegValidation | null>(null)

  // Computed properties
  const isAvailable = computed(() => status.value?.available ?? false)
  const isRunning = computed(() => status.value?.running ?? false)
  const conversionPool = computed(() => status.value?.conversion_pool)
  const placementPool = computed(() => status.value?.placement_pool)
  const activeJobs = computed(() => {
    const conv = conversionPool.value?.active_jobs ?? 0
    const place = placementPool.value?.active_jobs ?? 0
    return conv + place
  })
  const ffmpegReady = computed(() => ffmpegValidation.value?.valid ?? false)

  // Actions
  async function fetchStatus() {
    loading.value = true
    error.value = null
    try {
      status.value = await getPipelineStatus()
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch pipeline status'
    } finally {
      loading.value = false
    }
  }

  async function fetchConverterInfo() {
    loading.value = true
    error.value = null
    try {
      converterInfo.value = await getConverterInfo()
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch converter info'
    } finally {
      loading.value = false
    }
  }

  async function fetchPlacerInfo() {
    loading.value = true
    error.value = null
    try {
      placerInfo.value = await getPlacerInfo()
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch placer info'
    } finally {
      loading.value = false
    }
  }

  async function checkFfmpeg() {
    loading.value = true
    error.value = null
    try {
      ffmpegValidation.value = await validateFfmpeg()
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to validate ffmpeg'
    } finally {
      loading.value = false
    }
  }

  async function fetchAll(showLoading = true) {
    if (showLoading) {
      loading.value = true
    }
    error.value = null
    try {
      const [statusRes, converterRes, placerRes, ffmpegRes] = await Promise.all([
        getPipelineStatus(),
        getConverterInfo(),
        getPlacerInfo(),
        validateFfmpeg(),
      ])
      status.value = statusRes
      converterInfo.value = converterRes
      placerInfo.value = placerRes
      ffmpegValidation.value = ffmpegRes
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch pipeline info'
    } finally {
      if (showLoading) {
        loading.value = false
      }
    }
  }

  async function processTicket(
    ticketId: string,
    request: ProcessTicketRequest
  ): Promise<ProcessTicketResponse | null> {
    loading.value = true
    error.value = null
    try {
      return await apiProcessTicket(ticketId, request)
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to submit pipeline job'
      return null
    } finally {
      loading.value = false
    }
  }

  async function fetchProgress(ticketId: string): Promise<TicketProgress | null> {
    try {
      return await getTicketProgress(ticketId)
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch progress'
      return null
    }
  }

  return {
    // State
    loading,
    error,
    status,
    converterInfo,
    placerInfo,
    ffmpegValidation,
    // Computed
    isAvailable,
    isRunning,
    conversionPool,
    placementPool,
    activeJobs,
    ffmpegReady,
    // Actions
    fetchStatus,
    fetchConverterInfo,
    fetchPlacerInfo,
    checkFfmpeg,
    fetchAll,
    processTicket,
    fetchProgress,
  }
}

// Utility function for formatting pool statistics
export function formatPoolStats(pool: PoolStatus | undefined): string {
  if (!pool) return 'N/A'
  return `${pool.active_jobs}/${pool.max_concurrent} active, ${pool.queued_jobs} queued`
}

// Utility function for calculating pool utilization percentage
export function poolUtilization(pool: PoolStatus | undefined): number {
  if (!pool || pool.max_concurrent === 0) return 0
  return (pool.active_jobs / pool.max_concurrent) * 100
}
