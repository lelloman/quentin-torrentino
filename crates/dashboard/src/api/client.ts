// HTTP client wrapper with error handling

import type { ApiError } from './types'

const BASE_URL = '/api/v1'

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
  const response = await fetch(`${BASE_URL}${path}`)
  return handleResponse<T>(response)
}

export async function post<T, B = unknown>(path: string, body?: B): Promise<T> {
  const response = await fetch(`${BASE_URL}${path}`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: body ? JSON.stringify(body) : undefined,
  })
  return handleResponse<T>(response)
}

export async function del<T, B = unknown>(path: string, body?: B): Promise<T> {
  const response = await fetch(`${BASE_URL}${path}`, {
    method: 'DELETE',
    headers: {
      'Content-Type': 'application/json',
    },
    body: body ? JSON.stringify(body) : undefined,
  })
  return handleResponse<T>(response)
}
