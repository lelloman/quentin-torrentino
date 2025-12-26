import { ref, computed } from 'vue'
import type {
  TorrentInfo,
  TorrentFilterParams,
  TorrentClientStatusResponse,
  AddMagnetRequest,
  TorrentState,
} from '../api/types'
import {
  getTorrentClientStatus as apiGetStatus,
  listTorrents as apiListTorrents,
  getTorrent as apiGetTorrent,
  addMagnet as apiAddMagnet,
  addTorrentFile as apiAddTorrentFile,
  removeTorrent as apiRemoveTorrent,
  pauseTorrent as apiPauseTorrent,
  resumeTorrent as apiResumeTorrent,
  setUploadLimit as apiSetUploadLimit,
  setDownloadLimit as apiSetDownloadLimit,
  recheckTorrent as apiRecheckTorrent,
} from '../api/torrents'

export function useTorrents() {
  const torrents = ref<TorrentInfo[]>([])
  const currentTorrent = ref<TorrentInfo | null>(null)
  const status = ref<TorrentClientStatusResponse | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)

  // Filter state
  const stateFilter = ref<TorrentState | undefined>(undefined)
  const categoryFilter = ref<string | undefined>(undefined)
  const searchFilter = ref<string | undefined>(undefined)

  // Computed stats
  const totalCount = computed(() => torrents.value.length)
  const downloadingCount = computed(
    () => torrents.value.filter((t) => t.state === 'downloading').length
  )
  const seedingCount = computed(
    () => torrents.value.filter((t) => t.state === 'seeding').length
  )
  const pausedCount = computed(
    () => torrents.value.filter((t) => t.state === 'paused').length
  )

  const totalDownloadSpeed = computed(() =>
    torrents.value.reduce((sum, t) => sum + t.download_speed, 0)
  )
  const totalUploadSpeed = computed(() =>
    torrents.value.reduce((sum, t) => sum + t.upload_speed, 0)
  )

  async function fetchStatus() {
    try {
      status.value = await apiGetStatus()
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch status'
    }
  }

  async function fetchTorrents() {
    loading.value = true
    error.value = null

    const filters: TorrentFilterParams = {
      state: stateFilter.value,
      category: categoryFilter.value,
      search: searchFilter.value,
    }

    try {
      const response = await apiListTorrents(filters)
      torrents.value = response.torrents
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch torrents'
    } finally {
      loading.value = false
    }
  }

  async function fetchTorrent(hash: string) {
    loading.value = true
    error.value = null
    currentTorrent.value = null

    try {
      currentTorrent.value = await apiGetTorrent(hash)
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch torrent'
    } finally {
      loading.value = false
    }
  }

  async function addMagnet(request: AddMagnetRequest): Promise<string | null> {
    loading.value = true
    error.value = null

    try {
      const response = await apiAddMagnet(request)
      // Refresh the list
      await fetchTorrents()
      return response.hash
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to add torrent'
      return null
    } finally {
      loading.value = false
    }
  }

  async function addTorrentFile(
    file: File,
    options?: {
      download_path?: string
      category?: string
      paused?: boolean
      ticket_id?: string
    }
  ): Promise<string | null> {
    loading.value = true
    error.value = null

    try {
      const response = await apiAddTorrentFile(file, options)
      // Refresh the list
      await fetchTorrents()
      return response.hash
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to add torrent file'
      return null
    } finally {
      loading.value = false
    }
  }

  async function removeTorrent(hash: string, deleteFiles = false): Promise<boolean> {
    loading.value = true
    error.value = null

    try {
      await apiRemoveTorrent(hash, deleteFiles)
      // Remove from list
      torrents.value = torrents.value.filter((t) => t.hash !== hash)
      if (currentTorrent.value?.hash === hash) {
        currentTorrent.value = null
      }
      return true
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to remove torrent'
      return false
    } finally {
      loading.value = false
    }
  }

  async function pauseTorrent(hash: string): Promise<boolean> {
    error.value = null

    try {
      await apiPauseTorrent(hash)
      // Update local state
      const torrent = torrents.value.find((t) => t.hash === hash)
      if (torrent) {
        torrent.state = 'paused'
      }
      if (currentTorrent.value?.hash === hash) {
        currentTorrent.value.state = 'paused'
      }
      return true
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to pause torrent'
      return false
    }
  }

  async function resumeTorrent(hash: string): Promise<boolean> {
    error.value = null

    try {
      await apiResumeTorrent(hash)
      // Refresh to get correct state
      await fetchTorrents()
      return true
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to resume torrent'
      return false
    }
  }

  async function setUploadLimit(hash: string, limit: number): Promise<boolean> {
    error.value = null

    try {
      await apiSetUploadLimit(hash, limit)
      // Update local state
      const torrent = torrents.value.find((t) => t.hash === hash)
      if (torrent) {
        torrent.upload_limit = limit
      }
      if (currentTorrent.value?.hash === hash) {
        currentTorrent.value.upload_limit = limit
      }
      return true
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to set upload limit'
      return false
    }
  }

  async function setDownloadLimit(hash: string, limit: number): Promise<boolean> {
    error.value = null

    try {
      await apiSetDownloadLimit(hash, limit)
      // Update local state
      const torrent = torrents.value.find((t) => t.hash === hash)
      if (torrent) {
        torrent.download_limit = limit
      }
      if (currentTorrent.value?.hash === hash) {
        currentTorrent.value.download_limit = limit
      }
      return true
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to set download limit'
      return false
    }
  }

  async function recheckTorrent(hash: string): Promise<boolean> {
    error.value = null

    try {
      await apiRecheckTorrent(hash)
      // Update local state
      const torrent = torrents.value.find((t) => t.hash === hash)
      if (torrent) {
        torrent.state = 'checking'
      }
      if (currentTorrent.value?.hash === hash) {
        currentTorrent.value.state = 'checking'
      }
      return true
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to recheck torrent'
      return false
    }
  }

  function setStateFilter(state: TorrentState | undefined) {
    stateFilter.value = state
  }

  function setCategoryFilter(category: string | undefined) {
    categoryFilter.value = category
  }

  function setSearchFilter(search: string | undefined) {
    searchFilter.value = search
  }

  function clearFilters() {
    stateFilter.value = undefined
    categoryFilter.value = undefined
    searchFilter.value = undefined
  }

  function clearError() {
    error.value = null
  }

  return {
    // State
    torrents,
    currentTorrent,
    status,
    loading,
    error,

    // Filters
    stateFilter,
    categoryFilter,
    searchFilter,

    // Computed stats
    totalCount,
    downloadingCount,
    seedingCount,
    pausedCount,
    totalDownloadSpeed,
    totalUploadSpeed,

    // Actions
    fetchStatus,
    fetchTorrents,
    fetchTorrent,
    addMagnet,
    addTorrentFile,
    removeTorrent,
    pauseTorrent,
    resumeTorrent,
    setUploadLimit,
    setDownloadLimit,
    recheckTorrent,

    // Filter setters
    setStateFilter,
    setCategoryFilter,
    setSearchFilter,
    clearFilters,
    clearError,
  }
}
