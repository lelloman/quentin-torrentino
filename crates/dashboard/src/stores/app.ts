import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { HealthResponse, SanitizedConfig } from '../api/types'
import { getHealth } from '../api/health'
import { getConfig } from '../api/config'

export const useAppStore = defineStore('app', () => {
  const health = ref<HealthResponse | null>(null)
  const config = ref<SanitizedConfig | null>(null)
  const healthLoading = ref(false)
  const configLoading = ref(false)
  const healthError = ref<string | null>(null)
  const configError = ref<string | null>(null)

  async function fetchHealth() {
    healthLoading.value = true
    healthError.value = null
    try {
      health.value = await getHealth()
    } catch (e) {
      healthError.value = e instanceof Error ? e.message : 'Failed to fetch health'
    } finally {
      healthLoading.value = false
    }
  }

  async function fetchConfig() {
    configLoading.value = true
    configError.value = null
    try {
      config.value = await getConfig()
    } catch (e) {
      configError.value = e instanceof Error ? e.message : 'Failed to fetch config'
    } finally {
      configLoading.value = false
    }
  }

  return {
    health,
    config,
    healthLoading,
    configLoading,
    healthError,
    configError,
    fetchHealth,
    fetchConfig,
  }
})
