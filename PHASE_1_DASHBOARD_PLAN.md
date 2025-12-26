# Phase 1 Dashboard: Foundation

## Overview

This phase implements the admin dashboard foundation using Vue 3 + TypeScript + Vite. By the end, you'll have a working web UI to view system health, configuration, and manage tickets - replacing manual curl testing.

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Framework | Vue 3 (Composition API) | Per README spec, good DX |
| Build tool | Vite | Fast, modern, Vue-native |
| Styling | UnoCSS | Tailwind-compatible, Vite-native, per README |
| State | Pinia | Vue's official state management |
| HTTP client | fetch + custom wrapper | Simple, no extra deps needed |
| Component library | None (custom) | Keep it minimal, add later if needed |

## Scope

### In Scope
- Project scaffolding (Vite + Vue 3 + TypeScript)
- UnoCSS setup with sensible defaults
- API client module for backend communication
- Basic layout (header, sidebar, main content)
- Health status display
- Configuration viewer (sanitized config from API)
- Tickets list with filtering (state, pagination)
- Ticket detail view
- Create ticket form
- Cancel ticket action
- Error handling and loading states
- Dev proxy to backend server

### Out of Scope (Future Phases)
- WebSocket real-time updates (Phase 6)
- Authentication UI (when we add real auth)
- Search testing UI (Phase 2)
- Torrent status view (Phase 2)
- Shadow Catalog browser (Phase 2)
- Approval workflow (Phase 5)

## Project Structure

```
crates/dashboard/
├── package.json
├── vite.config.ts
├── tsconfig.json
├── tsconfig.node.json
├── uno.config.ts
├── index.html
├── src/
│   ├── main.ts                 # App entry point
│   ├── App.vue                 # Root component
│   ├── api/
│   │   ├── client.ts           # HTTP client wrapper
│   │   ├── types.ts            # API response types
│   │   ├── health.ts           # Health API
│   │   ├── config.ts           # Config API
│   │   └── tickets.ts          # Tickets API
│   ├── stores/
│   │   └── app.ts              # Global app store (Pinia)
│   ├── composables/
│   │   ├── useHealth.ts        # Health check composable
│   │   └── useTickets.ts       # Tickets management composable
│   ├── components/
│   │   ├── layout/
│   │   │   ├── AppHeader.vue
│   │   │   ├── AppSidebar.vue
│   │   │   └── AppLayout.vue
│   │   ├── common/
│   │   │   ├── LoadingSpinner.vue
│   │   │   ├── ErrorAlert.vue
│   │   │   └── Badge.vue
│   │   └── tickets/
│   │       ├── TicketList.vue
│   │       ├── TicketCard.vue
│   │       ├── TicketDetail.vue
│   │       ├── TicketStateFilter.vue
│   │       └── CreateTicketForm.vue
│   ├── views/
│   │   ├── DashboardView.vue   # Home/overview
│   │   ├── HealthView.vue      # Health & status
│   │   ├── ConfigView.vue      # Config display
│   │   ├── TicketsView.vue     # Ticket list
│   │   └── TicketDetailView.vue # Single ticket
│   ├── router/
│   │   └── index.ts            # Vue Router setup
│   └── styles/
│       └── main.css            # Global styles
└── public/
    └── favicon.ico
```

## API Types (TypeScript)

```typescript
// src/api/types.ts

export interface HealthResponse {
  status: string;
  version: string;
}

export interface SanitizedConfig {
  auth: {
    method: string;
  };
  server: {
    host: string;
    port: number;
  };
  database: {
    path: string;
  };
}

export interface QueryContext {
  tags: string[];
  description: string;
}

export interface TicketState {
  type: 'pending' | 'cancelled' | 'completed' | 'failed';
  // Additional fields depending on type
  cancelled_by?: string;
  reason?: string;
  cancelled_at?: string;
  completed_at?: string;
  error?: string;
  failed_at?: string;
}

export interface Ticket {
  id: string;
  created_at: string;
  created_by: string;
  state: TicketState;
  priority: number;
  query_context: QueryContext;
  dest_path: string;
  updated_at: string;
}

export interface TicketListResponse {
  tickets: Ticket[];
  total: number;
  limit: number;
  offset: number;
}

export interface CreateTicketRequest {
  priority?: number;
  query_context: QueryContext;
  dest_path: string;
}
```

## Views & Routes

| Route | View | Description |
|-------|------|-------------|
| `/` | DashboardView | Overview with health + recent tickets |
| `/health` | HealthView | Detailed health status |
| `/config` | ConfigView | Configuration display |
| `/tickets` | TicketsView | Ticket list with filters |
| `/tickets/:id` | TicketDetailView | Single ticket detail |

## Implementation Tasks

### Task 1: Project Scaffolding
**Files**: `crates/dashboard/*`

- [x] Create package.json with dependencies
- [x] Create vite.config.ts with proxy to backend
- [x] Create tsconfig.json and tsconfig.node.json
- [x] Create uno.config.ts with theme
- [x] Create index.html entry point
- [x] Create src/main.ts and src/App.vue
- [x] Verify dev server starts

### Task 2: API Client
**Files**: `src/api/*`

- [x] Create client.ts with fetch wrapper (base URL, error handling)
- [x] Create types.ts with TypeScript interfaces
- [x] Create health.ts - getHealth()
- [x] Create config.ts - getConfig()
- [x] Create tickets.ts - CRUD operations

### Task 3: Layout Components
**Files**: `src/components/layout/*`

- [x] Create AppHeader.vue (logo, nav links)
- [x] Create AppSidebar.vue (navigation menu)
- [x] Create AppLayout.vue (combines header + sidebar + slot)

### Task 4: Common Components
**Files**: `src/components/common/*`

- [x] Create LoadingSpinner.vue
- [x] Create ErrorAlert.vue
- [x] Create Badge.vue (for ticket states)

### Task 5: Router Setup
**Files**: `src/router/index.ts`

- [x] Install and configure vue-router
- [x] Define routes for all views
- [x] Set up route guards if needed

### Task 6: Pinia Store
**Files**: `src/stores/app.ts`

- [x] Create app store with global state
- [x] Health status
- [x] Loading states
- [x] Error state

### Task 7: Health & Config Views
**Files**: `src/views/HealthView.vue`, `src/views/ConfigView.vue`

- [x] Create HealthView with health check display
- [x] Create ConfigView with config JSON display
- [x] Add refresh functionality

### Task 8: Tickets Composable
**Files**: `src/composables/useTickets.ts`

- [x] Implement useTickets composable
- [x] List tickets with filters
- [x] Get single ticket
- [x] Create ticket
- [x] Cancel ticket
- [x] Loading/error states

### Task 9: Ticket Components
**Files**: `src/components/tickets/*`

- [x] Create TicketList.vue - list display
- [x] Create TicketCard.vue - single ticket card
- [x] Create TicketStateFilter.vue - filter dropdown
- [x] Create CreateTicketForm.vue - creation form
- [x] Create TicketDetail.vue - full ticket view

### Task 10: Tickets Views
**Files**: `src/views/TicketsView.vue`, `src/views/TicketDetailView.vue`

- [x] Create TicketsView with list + filters + create button
- [x] Create TicketDetailView with full info + cancel action

### Task 11: Dashboard Overview
**Files**: `src/views/DashboardView.vue`

- [x] Create overview page
- [x] Health status card
- [x] Recent tickets summary
- [x] Quick stats (total, pending, etc.)

### Task 12: Styles & Polish
**Files**: `src/styles/main.css`

- [x] Global reset/normalize
- [x] Dark theme (optional, can default to light)
- [x] Responsive layout adjustments
- [x] Transitions and animations

## Development Workflow

### Running the Dashboard

```bash
# Terminal 1: Start backend
cargo run -p torrentino-server

# Terminal 2: Start dashboard dev server
cd crates/dashboard
npm install
npm run dev
```

The Vite dev server will proxy API requests to the backend.

### Vite Proxy Config

```typescript
// vite.config.ts
export default defineConfig({
  server: {
    proxy: {
      '/api': {
        target: 'http://localhost:8080',
        changeOrigin: true,
      },
    },
  },
});
```

## Manual Testing Guide

After implementation, verify these flows:

### 1. Dashboard loads
- Navigate to `http://localhost:5173`
- Should see overview with health status

### 2. Health check
- Click "Health" in sidebar
- Should show "ok" status and version

### 3. View config
- Click "Config" in sidebar
- Should display sanitized configuration

### 4. Create a ticket
- Navigate to Tickets
- Click "Create Ticket"
- Fill form: tags, description, dest_path, priority
- Submit → should appear in list

### 5. View ticket details
- Click on a ticket in the list
- Should see full ticket info

### 6. Filter tickets
- Use state filter dropdown
- Should filter list appropriately

### 7. Cancel a ticket
- On ticket detail page, click "Cancel"
- Confirm action
- State should change to "cancelled"

### 8. Try to cancel again
- Should show error (already cancelled)

## Success Criteria

- [x] `npm run dev` starts without errors
- [x] `npm run build` produces production build
- [x] `npm run type-check` passes
- [x] All views render correctly
- [x] API calls work through proxy
- [x] Create ticket works
- [x] Cancel ticket works
- [x] Filters work
- [x] Error states display properly
- [x] Loading states display properly

## Dependencies

```json
{
  "dependencies": {
    "vue": "^3.4",
    "vue-router": "^4.2",
    "pinia": "^2.1"
  },
  "devDependencies": {
    "@vitejs/plugin-vue": "^5.0",
    "typescript": "^5.3",
    "vite": "^5.0",
    "vue-tsc": "^1.8",
    "unocss": "^0.58",
    "@unocss/reset": "^0.58"
  }
}
```

## Estimated Complexity

| Task | Complexity | Notes |
|------|------------|-------|
| 1. Project Scaffolding | Medium | Boilerplate setup |
| 2. API Client | Low | Simple fetch wrapper |
| 3. Layout Components | Low | Basic structure |
| 4. Common Components | Low | Reusable UI bits |
| 5. Router Setup | Low | Standard vue-router |
| 6. Pinia Store | Low | Minimal global state |
| 7. Health & Config Views | Low | Simple displays |
| 8. Tickets Composable | Medium | Business logic |
| 9. Ticket Components | Medium | Multiple components |
| 10. Tickets Views | Medium | Wire everything up |
| 11. Dashboard Overview | Low | Combine existing |
| 12. Styles & Polish | Low | Final touches |
