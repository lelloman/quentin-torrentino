import { ref, computed } from 'vue'
import { getStoredApiKey, setStoredApiKey, clearStoredApiKey, ApiClientError } from '../api/client'
import { getHealth } from '../api/health'

const apiKey = ref<string | null>(getStoredApiKey())
const isAuthenticated = ref<boolean>(false)
const authError = ref<string | null>(null)
const isChecking = ref<boolean>(false)

export function useAuth() {
  const hasApiKey = computed(() => apiKey.value !== null && apiKey.value.length > 0)

  async function checkAuth(): Promise<boolean> {
    isChecking.value = true
    authError.value = null

    try {
      await getHealth()
      isAuthenticated.value = true
      return true
    } catch (error) {
      if (error instanceof ApiClientError && error.status === 401) {
        isAuthenticated.value = false
        // Different message depending on whether user has entered a key
        authError.value = hasApiKey.value ? 'Invalid API key' : null
        return false
      }
      // Other errors (network, server down) - assume auth is OK if we have a key
      if (hasApiKey.value) {
        isAuthenticated.value = true
        return true
      }
      isAuthenticated.value = false
      authError.value = 'Unable to connect to server'
      return false
    } finally {
      isChecking.value = false
    }
  }

  async function setApiKey(key: string): Promise<boolean> {
    apiKey.value = key
    setStoredApiKey(key)
    return checkAuth()
  }

  function clearAuth(): void {
    apiKey.value = null
    isAuthenticated.value = false
    clearStoredApiKey()
  }

  return {
    apiKey: computed(() => apiKey.value),
    hasApiKey,
    isAuthenticated: computed(() => isAuthenticated.value),
    authError: computed(() => authError.value),
    isChecking: computed(() => isChecking.value),
    checkAuth,
    setApiKey,
    clearAuth,
  }
}
