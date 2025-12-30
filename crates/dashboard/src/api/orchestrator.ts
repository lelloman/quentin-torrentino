// Orchestrator API client

import { get, post } from './client'

// Types

export interface OrchestratorStatus {
  available: boolean
  running: boolean
  active_downloads: number
  acquiring_count: number
  pending_count: number
  needs_approval_count: number
  downloading_count: number
}

export interface MessageResponse {
  message: string
}

// API functions

export async function getOrchestratorStatus(): Promise<OrchestratorStatus> {
  return get<OrchestratorStatus>('/orchestrator/status')
}

export async function startOrchestrator(): Promise<MessageResponse> {
  return post<MessageResponse>('/orchestrator/start')
}

export async function stopOrchestrator(): Promise<MessageResponse> {
  return post<MessageResponse>('/orchestrator/stop')
}
