// HTTP client wrapper with error handling and authentication

import type { ApiError } from './types'

const BASE_URL = '/api/v1'
const API_KEY_STORAGE_KEY = 'quentin_api_key'

export class ApiClientError extends Error {
  constructor(
    public status: number,
    public statusText: string,
    public body?: ApiError
  ) {
    super(body?.error ?? `${status} ${statusText}`)
    this.name = 'ApiClientError'
  }
}

// API key management
export function getStoredApiKey(): string | null {
  return localStorage.getItem(API_KEY_STORAGE_KEY)
}

export function setStoredApiKey(key: string): void {
  localStorage.setItem(API_KEY_STORAGE_KEY, key)
}

export function clearStoredApiKey(): void {
  localStorage.removeItem(API_KEY_STORAGE_KEY)
}

function getAuthHeaders(): Record<string, string> {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
  }
  const apiKey = getStoredApiKey()
  if (apiKey) {
    headers['Authorization'] = `Bearer ${apiKey}`
  }
  return headers
}

async function handleResponse<T>(response: Response): Promise<T> {
  if (!response.ok) {
    let body: ApiError | undefined
    try {
      body = await response.json()
    } catch {
      // Response body wasn't JSON
    }
    throw new ApiClientError(response.status, response.statusText, body)
  }
  return response.json()
}

export async function get<T>(path: string): Promise<T> {
  const response = await fetch(`${BASE_URL}${path}`, {
    headers: getAuthHeaders(),
  })
  return handleResponse<T>(response)
}

export async function post<T, B = unknown>(path: string, body?: B): Promise<T> {
  const response = await fetch(`${BASE_URL}${path}`, {
    method: 'POST',
    headers: getAuthHeaders(),
    body: body ? JSON.stringify(body) : undefined,
  })
  return handleResponse<T>(response)
}

export async function del<T, B = unknown>(path: string, body?: B): Promise<T> {
  const response = await fetch(`${BASE_URL}${path}`, {
    method: 'DELETE',
    headers: getAuthHeaders(),
    body: body ? JSON.stringify(body) : undefined,
  })
  return handleResponse<T>(response)
}

export async function patch<T, B = unknown>(path: string, body?: B): Promise<T> {
  const response = await fetch(`${BASE_URL}${path}`, {
    method: 'PATCH',
    headers: getAuthHeaders(),
    body: body ? JSON.stringify(body) : undefined,
  })
  return handleResponse<T>(response)
}
