import { createRouter, createWebHistory } from 'vue-router'
import AppLayout from '../components/layout/AppLayout.vue'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      component: AppLayout,
      children: [
        {
          path: '',
          name: 'dashboard',
          component: () => import('../views/DashboardView.vue'),
        },
        {
          path: 'health',
          name: 'health',
          component: () => import('../views/HealthView.vue'),
        },
        {
          path: 'config',
          name: 'config',
          component: () => import('../views/ConfigView.vue'),
        },
        {
          path: 'tickets',
          name: 'tickets',
          component: () => import('../views/TicketsView.vue'),
        },
        {
          path: 'tickets/:id',
          name: 'ticket-detail',
          component: () => import('../views/TicketDetailView.vue'),
        },
        {
          path: 'search',
          name: 'search',
          component: () => import('../views/SearchView.vue'),
        },
        {
          path: 'torrents',
          name: 'torrents',
          component: () => import('../views/TorrentsView.vue'),
        },
        {
          path: 'settings',
          name: 'settings',
          component: () => import('../views/SettingsView.vue'),
        },
        {
          path: 'audit',
          name: 'audit',
          component: () => import('../views/AuditLogView.vue'),
        },
        {
          path: 'textbrain',
          name: 'textbrain',
          component: () => import('../views/TextBrainView.vue'),
        },
      ],
    },
  ],
})

export default router
