import { get } from './client'
import type { HealthResponse } from './types'

export async function getHealth(): Promise<HealthResponse> {
  return get<HealthResponse>('/health')
}
