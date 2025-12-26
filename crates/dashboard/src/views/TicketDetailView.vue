<script setup lang="ts">
import { useRoute } from 'vue-router'

const route = useRoute()
</script>

<template>
  <div>
    <h1 class="text-2xl font-bold mb-4">Ticket Detail</h1>
    <p class="text-gray-600">Viewing ticket: {{ route.params.id }}</p>
  </div>
</template>
