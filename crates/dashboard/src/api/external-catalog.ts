import { get } from './client'
import type {
  ExternalCatalogStatus,
  MusicBrainzRelease,
  TmdbMovie,
  TmdbSeries,
  TmdbSeason,
} from './types'

// Get external catalog availability status
export async function getExternalCatalogStatus(): Promise<ExternalCatalogStatus> {
  return get<ExternalCatalogStatus>('/external-catalog/status')
}

// =============================================================================
// MusicBrainz API
// =============================================================================

export interface MusicBrainzSearchParams {
  query: string
  limit?: number
}

export async function searchMusicBrainz(
  params: MusicBrainzSearchParams
): Promise<MusicBrainzRelease[]> {
  const searchParams = new URLSearchParams({ query: params.query })
  if (params.limit !== undefined) {
    searchParams.set('limit', params.limit.toString())
  }
  return get<MusicBrainzRelease[]>(
    `/external-catalog/musicbrainz/search?${searchParams}`
  )
}

export async function getMusicBrainzRelease(
  mbid: string
): Promise<MusicBrainzRelease> {
  return get<MusicBrainzRelease>(`/external-catalog/musicbrainz/release/${mbid}`)
}

// =============================================================================
// TMDB API
// =============================================================================

export interface TmdbMovieSearchParams {
  query: string
  year?: number
}

export async function searchTmdbMovies(
  params: TmdbMovieSearchParams
): Promise<TmdbMovie[]> {
  const searchParams = new URLSearchParams({ query: params.query })
  if (params.year !== undefined) {
    searchParams.set('year', params.year.toString())
  }
  return get<TmdbMovie[]>(`/external-catalog/tmdb/movies/search?${searchParams}`)
}

export async function getTmdbMovie(id: number): Promise<TmdbMovie> {
  return get<TmdbMovie>(`/external-catalog/tmdb/movies/${id}`)
}

export interface TmdbTvSearchParams {
  query: string
}

export async function searchTmdbTv(
  params: TmdbTvSearchParams
): Promise<TmdbSeries[]> {
  const searchParams = new URLSearchParams({ query: params.query })
  return get<TmdbSeries[]>(`/external-catalog/tmdb/tv/search?${searchParams}`)
}

export async function getTmdbTv(id: number): Promise<TmdbSeries> {
  return get<TmdbSeries>(`/external-catalog/tmdb/tv/${id}`)
}

export async function getTmdbSeason(
  seriesId: number,
  seasonNumber: number
): Promise<TmdbSeason> {
  return get<TmdbSeason>(
    `/external-catalog/tmdb/tv/${seriesId}/season/${seasonNumber}`
  )
}
