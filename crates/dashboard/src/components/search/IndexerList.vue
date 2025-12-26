<script setup lang="ts">
import type { IndexerStatus } from '../../api/types'
import Badge from '../common/Badge.vue'

defineProps<{
  indexers: IndexerStatus[]
  loading?: boolean
}>()

const emit = defineEmits<{
  edit: [indexer: IndexerStatus]
  toggle: [name: string, enabled: boolean]
}>()

function formatDate(dateStr: string | undefined): string {
  if (!dateStr) return 'Never'
  return new Date(dateStr).toLocaleString()
}
</script>

<template>
  <div class="space-y-3">
    <div v-if="indexers.length === 0 && !loading" class="text-center py-8 text-gray-500">
      No indexers configured
    </div>

    <div
      v-for="indexer in indexers"
      :key="indexer.name"
      class="card flex items-start gap-4"
    >
      <div class="flex-1">
        <div class="flex items-center gap-2">
          <h3 class="font-medium">{{ indexer.name }}</h3>
          <Badge :variant="indexer.enabled ? 'success' : 'default'">
            {{ indexer.enabled ? 'Enabled' : 'Disabled' }}
          </Badge>
        </div>

        <div class="mt-2 text-sm text-gray-500 space-y-1">
          <div class="flex items-center gap-4">
            <span>
              Rate limit: <strong>{{ indexer.rate_limit.requests_per_minute }}</strong> rpm
            </span>
            <span>
              Tokens: <strong>{{ indexer.rate_limit.tokens_available.toFixed(1) }}</strong>
            </span>
          </div>
          <div>
            Last used: {{ formatDate(indexer.last_used) }}
          </div>
          <div v-if="indexer.last_error" class="text-red-600">
            Last error: {{ indexer.last_error }}
          </div>
        </div>
      </div>

      <div class="flex items-center gap-2">
        <button
          @click="emit('toggle', indexer.name, !indexer.enabled)"
          class="btn-secondary text-sm"
          :disabled="loading"
        >
          {{ indexer.enabled ? 'Disable' : 'Enable' }}
        </button>
        <button
          @click="emit('edit', indexer)"
          class="btn-secondary text-sm"
          :disabled="loading"
        >
          Edit
        </button>
      </div>
    </div>
  </div>
</template>
