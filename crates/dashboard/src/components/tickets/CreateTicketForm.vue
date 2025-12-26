<script setup lang="ts">
import { ref, computed } from 'vue'
import type { CreateTicketRequest } from '../../api/types'

const emit = defineEmits<{
  submit: [request: CreateTicketRequest]
  cancel: []
}>()

const description = ref('')
const tagsInput = ref('')
const destPath = ref('')
const priority = ref(0)

const tags = computed(() => {
  return tagsInput.value
    .split(',')
    .map((t) => t.trim())
    .filter((t) => t.length > 0)
})

const isValid = computed(() => {
  return description.value.trim().length > 0 && destPath.value.trim().length > 0
})

function handleSubmit() {
  if (!isValid.value) return

  emit('submit', {
    priority: priority.value,
    query_context: {
      tags: tags.value,
      description: description.value.trim(),
    },
    dest_path: destPath.value.trim(),
  })
}

function handleCancel() {
  description.value = ''
  tagsInput.value = ''
  destPath.value = ''
  priority.value = 0
  emit('cancel')
}
</script>

<template>
  <form @submit.prevent="handleSubmit" class="card space-y-4">
    <h2 class="text-lg font-semibold">Create New Ticket</h2>

    <div>
      <label for="description" class="block text-sm font-medium text-gray-700 mb-1">
        Description
      </label>
      <textarea
        id="description"
        v-model="description"
        class="input w-full"
        rows="3"
        placeholder="What are you looking for?"
        required
      ></textarea>
    </div>

    <div>
      <label for="tags" class="block text-sm font-medium text-gray-700 mb-1">
        Tags (comma-separated)
      </label>
      <input
        id="tags"
        v-model="tagsInput"
        type="text"
        class="input w-full"
        placeholder="music, flac, album"
      />
    </div>

    <div>
      <label for="destPath" class="block text-sm font-medium text-gray-700 mb-1">
        Destination Path
      </label>
      <input
        id="destPath"
        v-model="destPath"
        type="text"
        class="input w-full"
        placeholder="/media/downloads/..."
        required
      />
    </div>

    <div>
      <label for="priority" class="block text-sm font-medium text-gray-700 mb-1">
        Priority (0-100)
      </label>
      <input
        id="priority"
        v-model.number="priority"
        type="number"
        min="0"
        max="100"
        class="input w-32"
      />
    </div>

    <div class="flex justify-end gap-3 pt-2">
      <button type="button" @click="handleCancel" class="btn-secondary">
        Cancel
      </button>
      <button type="submit" class="btn-primary" :disabled="!isValid">
        Create Ticket
      </button>
    </div>
  </form>
</template>
