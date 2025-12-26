import { ref } from 'vue'
import type { HealthResponse } from '../api/types'
import { getHealth as apiGetHealth } from '../api/health'

export function useHealth() {
  const health = ref<HealthResponse | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function fetchHealth() {
    loading.value = true
    error.value = null

    try {
      health.value = await apiGetHealth()
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch health'
    } finally {
      loading.value = false
    }
  }

  function clearError() {
    error.value = null
  }

  return {
    health,
    loading,
    error,
    fetchHealth,
    clearError,
  }
}
