<script setup lang="ts">
import { useAuth } from '../../composables/useAuth'

const { hasApiKey, clearAuth } = useAuth()

function handleLogout() {
  clearAuth()
  // Reload to show login page
  window.location.reload()
}
</script>

<template>
  <header class="h-14 bg-white border-b border-gray-200 flex items-center px-4 gap-4">
    <h1 class="text-lg font-semibold text-gray-800 flex-1">Quentin Dashboard</h1>
    <button
      v-if="hasApiKey"
      @click="handleLogout"
      class="text-sm text-gray-600 hover:text-gray-900"
    >
      Logout
    </button>
  </header>
</template>
