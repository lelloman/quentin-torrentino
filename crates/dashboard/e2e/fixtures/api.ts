/**
 * API helpers for E2E tests
 * Direct API calls for test setup/teardown
 */

const API_BASE = process.env.API_URL || 'http://localhost:18080';

export interface Ticket {
  ticket_id: string;
  state: string;
  search: {
    artist?: string;
    album?: string;
    title?: string;
  };
  created_at: string;
}

export interface HealthResponse {
  status: string;
  version: string;
}

export async function checkHealth(): Promise<HealthResponse> {
  const res = await fetch(`${API_BASE}/api/v1/health`);
  if (!res.ok) throw new Error(`Health check failed: ${res.status}`);
  return res.json();
}

export async function createTicket(ticket: {
  search: { artist?: string; album?: string };
  tracks?: Array<{
    catalog_track_id: string;
    disc_number: number;
    track_number: number;
    name: string;
    duration_secs: number;
    dest_path: string;
    requested: boolean;
  }>;
}): Promise<Ticket> {
  const res = await fetch(`${API_BASE}/api/v1/tickets`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(ticket),
  });
  if (!res.ok) throw new Error(`Create ticket failed: ${res.status}`);
  return res.json();
}

export async function getTicket(ticketId: string): Promise<Ticket> {
  const res = await fetch(`${API_BASE}/api/v1/tickets/${ticketId}`);
  if (!res.ok) throw new Error(`Get ticket failed: ${res.status}`);
  return res.json();
}

export async function listTickets(): Promise<Ticket[]> {
  const res = await fetch(`${API_BASE}/api/v1/tickets`);
  if (!res.ok) throw new Error(`List tickets failed: ${res.status}`);
  const data = await res.json();
  return data.tickets || [];
}

export async function cancelTicket(ticketId: string): Promise<void> {
  const res = await fetch(`${API_BASE}/api/v1/tickets/${ticketId}`, {
    method: 'DELETE',
  });
  if (!res.ok) throw new Error(`Cancel ticket failed: ${res.status}`);
}

export async function approveTicket(ticketId: string, candidateIdx?: number): Promise<void> {
  const res = await fetch(`${API_BASE}/api/v1/tickets/${ticketId}/approve`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ candidate_idx: candidateIdx }),
  });
  if (!res.ok) throw new Error(`Approve ticket failed: ${res.status}`);
}

export async function rejectTicket(ticketId: string, reason?: string): Promise<void> {
  const res = await fetch(`${API_BASE}/api/v1/tickets/${ticketId}/reject`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ reason }),
  });
  if (!res.ok) throw new Error(`Reject ticket failed: ${res.status}`);
}

export async function getAuditEvents(params?: {
  ticket_id?: string;
  event_type?: string;
  limit?: number;
}): Promise<unknown[]> {
  const searchParams = new URLSearchParams();
  if (params?.ticket_id) searchParams.set('ticket_id', params.ticket_id);
  if (params?.event_type) searchParams.set('event_type', params.event_type);
  if (params?.limit) searchParams.set('limit', String(params.limit));

  const res = await fetch(`${API_BASE}/api/v1/audit?${searchParams}`);
  if (!res.ok) throw new Error(`Get audit events failed: ${res.status}`);
  const data = await res.json();
  return data.events || [];
}

export async function getConfig(): Promise<unknown> {
  const res = await fetch(`${API_BASE}/api/v1/config`);
  if (!res.ok) throw new Error(`Get config failed: ${res.status}`);
  return res.json();
}
