<script setup lang="ts">
import { ref } from 'vue'

const emit = defineEmits<{
  submit: [uri: string]
  cancel: []
}>()

const magnetUri = ref('')

function handleSubmit() {
  if (magnetUri.value.trim()) {
    emit('submit', magnetUri.value.trim())
  }
}
</script>

<template>
  <div class="bg-white rounded-lg shadow-sm border p-4">
    <h3 class="font-medium mb-3">Add Torrent</h3>
    <form @submit.prevent="handleSubmit" class="space-y-4">
      <div>
        <label class="block text-sm font-medium text-gray-700 mb-1">
          Magnet URI
        </label>
        <input
          v-model="magnetUri"
          type="text"
          placeholder="magnet:?xt=urn:btih:..."
          class="w-full px-3 py-2 border rounded-md focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
        />
      </div>

      <div class="flex gap-2">
        <button type="submit" class="btn-primary" :disabled="!magnetUri.trim()">
          Add Torrent
        </button>
        <button type="button" @click="$emit('cancel')" class="btn-secondary">
          Cancel
        </button>
      </div>
    </form>
  </div>
</template>
