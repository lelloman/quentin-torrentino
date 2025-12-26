<script setup lang="ts">
import { RouterLink, useRoute } from 'vue-router'

const route = useRoute()

const navItems = [
  { to: '/', label: 'Dashboard', icon: 'i-carbon-dashboard' },
  { to: '/tickets', label: 'Tickets', icon: 'i-carbon-ticket' },
  { to: '/search', label: 'Search', icon: 'i-carbon-search' },
  { to: '/torrents', label: 'Torrents', icon: 'i-carbon-download' },
  { to: '/settings', label: 'Settings', icon: 'i-carbon-settings-adjust' },
  { to: '/health', label: 'Health', icon: 'i-carbon-activity' },
  { to: '/config', label: 'Config', icon: 'i-carbon-settings' },
]

const isActive = (path: string) => {
  if (path === '/') {
    return route.path === '/'
  }
  return route.path.startsWith(path)
}
</script>

<template>
  <aside class="w-56 bg-gray-50 border-r border-gray-200 min-h-screen">
    <nav class="p-4">
      <ul class="space-y-1">
        <li v-for="item in navItems" :key="item.to">
          <RouterLink
            :to="item.to"
            class="flex items-center gap-3 px-3 py-2 rounded-md text-sm transition-colors"
            :class="[
              isActive(item.to)
                ? 'bg-primary text-white'
                : 'text-gray-700 hover:bg-gray-200'
            ]"
          >
            <span :class="item.icon" class="text-lg"></span>
            <span>{{ item.label }}</span>
          </RouterLink>
        </li>
      </ul>
    </nav>
  </aside>
</template>
