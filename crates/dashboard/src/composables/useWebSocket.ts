import { ref, onUnmounted, type Ref } from 'vue'

// WebSocket message types matching backend ws.rs
export type WsMessageType =
  | 'ticket_update'
  | 'ticket_deleted'
  | 'torrent_progress'
  | 'pipeline_progress'
  | 'orchestrator_status'
  | 'heartbeat'

export interface TicketUpdateMessage {
  type: 'ticket_update'
  ticket_id: string
  state: string
}

export interface TicketDeletedMessage {
  type: 'ticket_deleted'
  ticket_id: string
}

export interface TorrentProgressMessage {
  type: 'torrent_progress'
  ticket_id: string
  info_hash: string
  progress_pct: number
  speed_bps: number
  eta_secs: number | null
}

export interface PipelineProgressMessage {
  type: 'pipeline_progress'
  ticket_id: string
  phase: string
  current: number
  total: number
  current_name: string
  /** FFmpeg conversion percentage (0.0 - 100.0) for current file */
  percent: number
}

export interface OrchestratorStatusMessage {
  type: 'orchestrator_status'
  running: boolean
}

export interface HeartbeatMessage {
  type: 'heartbeat'
  timestamp: number
}

export type WsMessage =
  | TicketUpdateMessage
  | TicketDeletedMessage
  | TorrentProgressMessage
  | PipelineProgressMessage
  | OrchestratorStatusMessage
  | HeartbeatMessage

export type WsMessageHandler = (message: WsMessage) => void

interface UseWebSocketOptions {
  autoConnect?: boolean
  reconnectInterval?: number
  maxReconnectAttempts?: number
}

export function useWebSocket(options: UseWebSocketOptions = {}) {
  const { autoConnect = true, reconnectInterval = 3000, maxReconnectAttempts = 10 } = options

  const connected = ref(false)
  const connecting = ref(false)
  const error: Ref<string | null> = ref(null)
  const reconnectAttempts = ref(0)

  let ws: WebSocket | null = null
  let reconnectTimeout: ReturnType<typeof setTimeout> | null = null
  const handlers: Set<WsMessageHandler> = new Set()

  function getWsUrl(): string {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const host = window.location.host
    return `${protocol}//${host}/api/v1/ws`
  }

  function connect() {
    if (ws && (ws.readyState === WebSocket.CONNECTING || ws.readyState === WebSocket.OPEN)) {
      return
    }

    connecting.value = true
    error.value = null

    try {
      ws = new WebSocket(getWsUrl())

      ws.onopen = () => {
        connected.value = true
        connecting.value = false
        reconnectAttempts.value = 0
        console.log('[WebSocket] Connected')
      }

      ws.onclose = (event) => {
        connected.value = false
        connecting.value = false
        console.log(`[WebSocket] Disconnected (code: ${event.code})`)

        // Attempt to reconnect if we didn't close intentionally
        if (!event.wasClean && reconnectAttempts.value < maxReconnectAttempts) {
          scheduleReconnect()
        }
      }

      ws.onerror = () => {
        error.value = 'WebSocket connection error'
        connecting.value = false
        console.error('[WebSocket] Error')
      }

      ws.onmessage = (event) => {
        try {
          const message = JSON.parse(event.data) as WsMessage
          // Dispatch to all handlers
          handlers.forEach((handler) => {
            try {
              handler(message)
            } catch (e) {
              console.error('[WebSocket] Handler error:', e)
            }
          })
        } catch (e) {
          console.error('[WebSocket] Failed to parse message:', e)
        }
      }
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to connect'
      connecting.value = false
    }
  }

  function disconnect() {
    if (reconnectTimeout) {
      clearTimeout(reconnectTimeout)
      reconnectTimeout = null
    }

    if (ws) {
      ws.close(1000, 'Client disconnect')
      ws = null
    }

    connected.value = false
    connecting.value = false
  }

  function scheduleReconnect() {
    if (reconnectTimeout) {
      return
    }

    reconnectAttempts.value++
    console.log(`[WebSocket] Reconnecting in ${reconnectInterval}ms (attempt ${reconnectAttempts.value}/${maxReconnectAttempts})`)

    reconnectTimeout = setTimeout(() => {
      reconnectTimeout = null
      connect()
    }, reconnectInterval)
  }

  function addHandler(handler: WsMessageHandler) {
    handlers.add(handler)
  }

  function removeHandler(handler: WsMessageHandler) {
    handlers.delete(handler)
  }

  // Auto-connect if enabled
  if (autoConnect) {
    connect()
  }

  // Cleanup on unmount
  onUnmounted(() => {
    disconnect()
    handlers.clear()
  })

  return {
    connected,
    connecting,
    error,
    reconnectAttempts,
    connect,
    disconnect,
    addHandler,
    removeHandler,
  }
}

// Global WebSocket instance for sharing across components
let globalWs: ReturnType<typeof useWebSocket> | null = null

export function useGlobalWebSocket() {
  if (!globalWs) {
    // Create the global instance without auto-cleanup
    globalWs = useWebSocket({ autoConnect: true })
  }
  return globalWs
}
