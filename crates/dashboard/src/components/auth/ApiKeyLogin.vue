<script setup lang="ts">
import { ref } from 'vue'
import { useAuth } from '../../composables/useAuth'

const { authError, isChecking, setApiKey } = useAuth()

const apiKeyInput = ref('')
const showKey = ref(false)

async function handleSubmit() {
  if (!apiKeyInput.value.trim()) return
  await setApiKey(apiKeyInput.value.trim())
}
</script>

<template>
  <div class="min-h-screen flex items-center justify-center bg-gray-100">
    <div class="card w-full max-w-md mx-4">
      <div class="text-center mb-6">
        <h1 class="text-2xl font-bold text-gray-900">Quentin Torrentino</h1>
        <p class="text-gray-600 mt-2">Enter your API key to continue</p>
      </div>

      <form @submit.prevent="handleSubmit" class="space-y-4">
        <div>
          <label for="apiKey" class="block text-sm font-medium text-gray-700 mb-1">
            API Key
          </label>
          <div class="relative">
            <input
              id="apiKey"
              v-model="apiKeyInput"
              :type="showKey ? 'text' : 'password'"
              class="input w-full pr-10"
              placeholder="Enter your API key"
              autocomplete="off"
            />
            <button
              type="button"
              @click="showKey = !showKey"
              class="absolute right-2 top-1/2 -translate-y-1/2 text-gray-500 hover:text-gray-700"
            >
              <span v-if="showKey">Hide</span>
              <span v-else>Show</span>
            </button>
          </div>
        </div>

        <div v-if="authError" class="text-red-600 text-sm">
          {{ authError }}
        </div>

        <button
          type="submit"
          :disabled="isChecking || !apiKeyInput.trim()"
          class="btn-primary w-full"
        >
          <span v-if="isChecking">Verifying...</span>
          <span v-else>Sign In</span>
        </button>
      </form>

      <div class="mt-6 text-center text-sm text-gray-500">
        <p>
          The API key is configured in your server's <code class="bg-gray-100 px-1 rounded">config.toml</code>
        </p>
      </div>
    </div>
  </div>
</template>
