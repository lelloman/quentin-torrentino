import { get } from './client'
import type { SanitizedConfig } from './types'

export async function getConfig(): Promise<SanitizedConfig> {
  return get<SanitizedConfig>('/config')
}
