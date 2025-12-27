import { get, del } from './client'
import type { CatalogStats } from './types'

export async function getCatalogStats(): Promise<CatalogStats> {
  return get<CatalogStats>('/catalog/stats')
}

export async function clearCatalog(): Promise<{ message: string }> {
  return del<{ message: string }>('/catalog')
}
