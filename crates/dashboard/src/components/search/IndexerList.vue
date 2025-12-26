<script setup lang="ts">
import type { IndexerStatus } from '../../api/types'
import Badge from '../common/Badge.vue'

defineProps<{
  indexers: IndexerStatus[]
  loading?: boolean
}>()
</script>

<template>
  <div class="space-y-3">
    <div v-if="indexers.length === 0 && !loading" class="text-center py-8 text-gray-500">
      No indexers configured in Jackett
    </div>

    <div v-for="indexer in indexers" :key="indexer.name" class="card flex items-center gap-4">
      <div class="flex-1">
        <div class="flex items-center gap-2">
          <h3 class="font-medium">{{ indexer.name }}</h3>
          <Badge :variant="indexer.enabled ? 'success' : 'default'">
            {{ indexer.enabled ? 'Enabled' : 'Disabled' }}
          </Badge>
        </div>
      </div>
    </div>
  </div>
</template>
