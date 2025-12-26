// API response types matching backend structures

export interface HealthResponse {
  status: string
}

export interface SanitizedConfig {
  auth: {
    method: string
  }
  server: {
    host: string
    port: number
  }
  database: {
    path: string
  }
}

export interface QueryContext {
  tags: string[]
  description: string
}

// TicketState uses discriminated union with 'type' field
export type TicketState =
  | { type: 'pending' }
  | {
      type: 'cancelled'
      cancelled_by: string
      reason: string | null
      cancelled_at: string
    }
  | {
      type: 'completed'
      completed_at: string
    }
  | {
      type: 'failed'
      error: string
      failed_at: string
    }

export interface Ticket {
  id: string
  created_at: string
  created_by: string
  state: TicketState
  priority: number
  query_context: QueryContext
  dest_path: string
  updated_at: string
}

export interface TicketListResponse {
  tickets: Ticket[]
  total: number
  limit: number
  offset: number
}

export interface CreateTicketRequest {
  priority?: number
  query_context: {
    tags: string[]
    description: string
  }
  dest_path: string
}

export interface CancelTicketRequest {
  reason?: string
}

export interface ApiError {
  error: string
}

export type TicketStateType = 'pending' | 'cancelled' | 'completed' | 'failed'
