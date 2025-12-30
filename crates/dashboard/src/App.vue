<script setup lang="ts">
import { onMounted } from 'vue'
import { RouterView } from 'vue-router'
import { useAuth } from './composables/useAuth'
import ApiKeyLogin from './components/auth/ApiKeyLogin.vue'
import LoadingSpinner from './components/common/LoadingSpinner.vue'

const { isAuthenticated, isChecking, checkAuth } = useAuth()

onMounted(() => {
  checkAuth()
})
</script>

<template>
  <div v-if="isChecking" class="min-h-screen flex items-center justify-center">
    <LoadingSpinner size="lg" />
  </div>
  <ApiKeyLogin v-else-if="!isAuthenticated" />
  <RouterView v-else />
</template>
