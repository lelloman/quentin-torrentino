import { get, post, del } from './client'
import type {
  TorrentInfo,
  TorrentListResponse,
  TorrentFilterParams,
  AddMagnetRequest,
  AddFromUrlRequest,
  AddTorrentResponse,
  TorrentClientStatusResponse,
  SetLimitRequest,
  SuccessResponse,
} from './types'

const BASE_URL = '/api/v1'

export async function getTorrentClientStatus(): Promise<TorrentClientStatusResponse> {
  return get<TorrentClientStatusResponse>('/torrents/status')
}

export async function listTorrents(
  filters?: TorrentFilterParams
): Promise<TorrentListResponse> {
  const params = new URLSearchParams()
  if (filters?.state) params.append('state', filters.state)
  if (filters?.category) params.append('category', filters.category)
  if (filters?.search) params.append('search', filters.search)

  const query = params.toString()
  const path = query ? `/torrents?${query}` : '/torrents'
  return get<TorrentListResponse>(path)
}

export async function getTorrent(hash: string): Promise<TorrentInfo> {
  return get<TorrentInfo>(`/torrents/${hash}`)
}

export async function addMagnet(request: AddMagnetRequest): Promise<AddTorrentResponse> {
  return post<AddTorrentResponse>('/torrents/add/magnet', request)
}

export async function addTorrentFile(
  file: File,
  options?: {
    download_path?: string
    category?: string
    paused?: boolean
    ticket_id?: string
  }
): Promise<AddTorrentResponse> {
  const formData = new FormData()
  formData.append('file', file)
  if (options?.download_path) formData.append('download_path', options.download_path)
  if (options?.category) formData.append('category', options.category)
  if (options?.paused) formData.append('paused', options.paused ? 'true' : 'false')
  if (options?.ticket_id) formData.append('ticket_id', options.ticket_id)

  const response = await fetch(`${BASE_URL}/torrents/add/file`, {
    method: 'POST',
    body: formData,
  })

  if (!response.ok) {
    const body = await response.json().catch(() => ({}))
    throw new Error(body.error || `${response.status} ${response.statusText}`)
  }

  return response.json()
}

export async function removeTorrent(
  hash: string,
  deleteFiles = false
): Promise<SuccessResponse> {
  const params = new URLSearchParams()
  if (deleteFiles) params.append('delete_files', 'true')

  const query = params.toString()
  const path = query ? `/torrents/${hash}?${query}` : `/torrents/${hash}`
  return del<SuccessResponse>(path)
}

export async function pauseTorrent(hash: string): Promise<SuccessResponse> {
  return post<SuccessResponse>(`/torrents/${hash}/pause`)
}

export async function resumeTorrent(hash: string): Promise<SuccessResponse> {
  return post<SuccessResponse>(`/torrents/${hash}/resume`)
}

export async function setUploadLimit(
  hash: string,
  limit: number
): Promise<SuccessResponse> {
  return post<SuccessResponse, SetLimitRequest>(`/torrents/${hash}/upload-limit`, {
    limit,
  })
}

export async function setDownloadLimit(
  hash: string,
  limit: number
): Promise<SuccessResponse> {
  return post<SuccessResponse, SetLimitRequest>(`/torrents/${hash}/download-limit`, {
    limit,
  })
}

export async function recheckTorrent(hash: string): Promise<SuccessResponse> {
  return post<SuccessResponse>(`/torrents/${hash}/recheck`)
}

export async function addTorrentFromUrl(
  torrentUrl: string,
  options?: {
    download_path?: string
    category?: string
    paused?: boolean
    ticket_id?: string
  }
): Promise<AddTorrentResponse> {
  // Use backend proxy to fetch the URL (avoids CORS issues with redirects)
  const request: AddFromUrlRequest = {
    url: torrentUrl,
    ...options,
  }
  return post<AddTorrentResponse, AddFromUrlRequest>('/torrents/add/url', request)
}
