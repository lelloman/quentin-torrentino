import { ref, computed } from 'vue'
import {
  getExternalCatalogStatus,
  searchMusicBrainz,
  getMusicBrainzRelease,
  searchTmdbMovies,
  getTmdbMovie,
  searchTmdbTv,
  getTmdbTv,
  getTmdbSeason,
} from '@/api/external-catalog'
import type {
  ExternalCatalogStatus,
  MusicBrainzRelease,
  TmdbMovie,
  TmdbSeries,
  TmdbSeason,
  CatalogReference,
  ExpectedContent,
  ExpectedTrack,
} from '@/api/types'

export function useCatalogLookup() {
  const status = ref<ExternalCatalogStatus | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)

  // Search results
  const musicBrainzResults = ref<MusicBrainzRelease[]>([])
  const tmdbMovieResults = ref<TmdbMovie[]>([])
  const tmdbTvResults = ref<TmdbSeries[]>([])

  // Selected items
  const selectedRelease = ref<MusicBrainzRelease | null>(null)
  const selectedMovie = ref<TmdbMovie | null>(null)
  const selectedSeries = ref<TmdbSeries | null>(null)
  const selectedSeason = ref<TmdbSeason | null>(null)

  const isAvailable = computed(() => {
    return (
      status.value?.musicbrainz_available || status.value?.tmdb_available
    )
  })

  // Fetch catalog status
  async function fetchStatus() {
    try {
      status.value = await getExternalCatalogStatus()
    } catch (e) {
      console.error('Failed to fetch external catalog status:', e)
      status.value = { musicbrainz_available: false, tmdb_available: false }
    }
  }

  // Search MusicBrainz releases
  async function searchReleases(query: string, limit = 10) {
    if (!status.value?.musicbrainz_available) {
      error.value = 'MusicBrainz not configured'
      return
    }

    loading.value = true
    error.value = null
    try {
      musicBrainzResults.value = await searchMusicBrainz({ query, limit })
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Search failed'
      musicBrainzResults.value = []
    } finally {
      loading.value = false
    }
  }

  // Get full release details and select it
  async function selectRelease(mbid: string) {
    loading.value = true
    error.value = null
    try {
      selectedRelease.value = await getMusicBrainzRelease(mbid)
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch release'
      selectedRelease.value = null
    } finally {
      loading.value = false
    }
  }

  // Search TMDB movies
  async function searchMovies(query: string, year?: number) {
    if (!status.value?.tmdb_available) {
      error.value = 'TMDB not configured'
      return
    }

    loading.value = true
    error.value = null
    try {
      tmdbMovieResults.value = await searchTmdbMovies({ query, year })
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Search failed'
      tmdbMovieResults.value = []
    } finally {
      loading.value = false
    }
  }

  // Get and select a movie
  async function selectMovie(id: number) {
    loading.value = true
    error.value = null
    try {
      selectedMovie.value = await getTmdbMovie(id)
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch movie'
      selectedMovie.value = null
    } finally {
      loading.value = false
    }
  }

  // Search TMDB TV series
  async function searchTvSeries(query: string) {
    if (!status.value?.tmdb_available) {
      error.value = 'TMDB not configured'
      return
    }

    loading.value = true
    error.value = null
    try {
      tmdbTvResults.value = await searchTmdbTv({ query })
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Search failed'
      tmdbTvResults.value = []
    } finally {
      loading.value = false
    }
  }

  // Get and select a TV series
  async function selectSeries(id: number) {
    loading.value = true
    error.value = null
    try {
      selectedSeries.value = await getTmdbTv(id)
      selectedSeason.value = null
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch series'
      selectedSeries.value = null
    } finally {
      loading.value = false
    }
  }

  // Get and select a season
  async function selectSeason(seriesId: number, seasonNumber: number) {
    loading.value = true
    error.value = null
    try {
      selectedSeason.value = await getTmdbSeason(seriesId, seasonNumber)
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch season'
      selectedSeason.value = null
    } finally {
      loading.value = false
    }
  }

  // Build CatalogReference from selected item
  function buildCatalogReference(): CatalogReference | undefined {
    if (selectedRelease.value) {
      return {
        type: 'music_brainz',
        release_id: selectedRelease.value.mbid,
        track_count: selectedRelease.value.track_count ?? selectedRelease.value.tracks.length,
        total_duration_ms: selectedRelease.value.total_length_ms,
      }
    }

    if (selectedMovie.value) {
      return {
        type: 'tmdb',
        id: selectedMovie.value.id,
        media_type: 'movie',
        runtime_minutes: selectedMovie.value.runtime_minutes,
      }
    }

    if (selectedSeries.value && selectedSeason.value) {
      return {
        type: 'tmdb',
        id: selectedSeries.value.id,
        media_type: 'tv',
        episode_count: selectedSeason.value.episode_count,
      }
    }

    return undefined
  }

  // Build ExpectedContent from selected item
  function buildExpectedContent(): ExpectedContent | undefined {
    if (selectedRelease.value) {
      const tracks: ExpectedTrack[] = selectedRelease.value.tracks.map((t) => ({
        number: t.position,
        title: t.title,
        duration_secs: t.length_ms ? Math.round(t.length_ms / 1000) : undefined,
      }))

      return {
        type: 'album',
        artist: selectedRelease.value.artist_credit,
        title: selectedRelease.value.title,
        tracks,
      }
    }

    if (selectedMovie.value) {
      const year = selectedMovie.value.release_date
        ? parseInt(selectedMovie.value.release_date.substring(0, 4), 10)
        : undefined
      return {
        type: 'movie',
        title: selectedMovie.value.title,
        year,
      }
    }

    if (selectedSeries.value && selectedSeason.value) {
      return {
        type: 'tv_episode',
        series: selectedSeries.value.name,
        season: selectedSeason.value.season_number,
        episodes: selectedSeason.value.episodes.map((e) => e.episode_number),
      }
    }

    return undefined
  }

  // Clear selection
  function clearSelection() {
    selectedRelease.value = null
    selectedMovie.value = null
    selectedSeries.value = null
    selectedSeason.value = null
    error.value = null
  }

  // Clear all
  function clearAll() {
    clearSelection()
    musicBrainzResults.value = []
    tmdbMovieResults.value = []
    tmdbTvResults.value = []
  }

  return {
    // State
    status,
    loading,
    error,
    isAvailable,

    // Search results
    musicBrainzResults,
    tmdbMovieResults,
    tmdbTvResults,

    // Selected items
    selectedRelease,
    selectedMovie,
    selectedSeries,
    selectedSeason,

    // Actions
    fetchStatus,
    searchReleases,
    selectRelease,
    searchMovies,
    selectMovie,
    searchTvSeries,
    selectSeries,
    selectSeason,
    clearSelection,
    clearAll,

    // Builders
    buildCatalogReference,
    buildExpectedContent,
  }
}
