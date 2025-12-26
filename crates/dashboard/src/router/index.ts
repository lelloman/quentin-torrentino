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
      ],
    },
  ],
})

export default router
