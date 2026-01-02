import { ref, computed, onMounted } from 'vue'
import { useCatalogLookup } from './useCatalogLookup'
import type {
  SearchConstraints,
  AudioSearchConstraints,
  VideoSearchConstraints,
  AudioFormat,
  OutputConstraints,
  CreateTicketWithCatalogRequest,
} from '@/api/types'

export type ContentType = 'album' | 'movie' | 'tv'
export type WizardStep = 'type' | 'search' | 'constraints' | 'details' | 'review'

export function useTicketWizard() {
  // Catalog lookup integration
  const catalog = useCatalogLookup()

  // Wizard state
  const currentStep = ref<WizardStep>('type')
  const contentType = ref<ContentType | null>(null)

  // Search state
  const searchQuery = ref('')
  const searchYear = ref<number | undefined>(undefined)

  // Constraints state
  const audioConstraints = ref<AudioSearchConstraints>({
    preferred_formats: [],
    min_bitrate_kbps: undefined,
    avoid_compilations: false,
    avoid_live: false,
  })

  const videoConstraints = ref<VideoSearchConstraints>({
    min_resolution: undefined,
    preferred_resolution: undefined,
    preferred_sources: [],
    preferred_codecs: [],
    preferred_language: undefined,
    exclude_hardcoded_subs: false,
  })

  // Ticket details
  const description = ref('')
  const tagsInput = ref('')
  const destPath = ref('')
  const priority = ref(0)
  const outputType = ref<'original' | 'audio'>('original')
  const audioFormat = ref<AudioFormat>('ogg_vorbis')
  const audioBitrate = ref<number | undefined>(320)

  // Computed values
  const tags = computed(() => {
    return tagsInput.value
      .split(',')
      .map((t) => t.trim())
      .filter((t) => t.length > 0)
  })

  const hasSelection = computed(() => {
    return (
      catalog.selectedRelease.value !== null ||
      catalog.selectedMovie.value !== null ||
      (catalog.selectedSeries.value !== null && catalog.selectedSeason.value !== null)
    )
  })

  const selectedItemSummary = computed(() => {
    if (catalog.selectedRelease.value) {
      const r = catalog.selectedRelease.value
      return {
        type: 'album' as const,
        title: r.title,
        subtitle: `${r.artist_credit} - ${r.track_count} tracks`,
      }
    }
    if (catalog.selectedMovie.value) {
      const m = catalog.selectedMovie.value
      return {
        type: 'movie' as const,
        title: m.title,
        subtitle: m.release_date?.substring(0, 4) ?? 'Unknown year',
      }
    }
    if (catalog.selectedSeries.value && catalog.selectedSeason.value) {
      const s = catalog.selectedSeries.value
      const season = catalog.selectedSeason.value
      return {
        type: 'tv' as const,
        title: s.name,
        subtitle: `Season ${season.season_number} - ${season.episode_count} episodes`,
      }
    }
    return null
  })

  const searchConstraints = computed((): SearchConstraints | undefined => {
    if (contentType.value === 'album') {
      const ac = audioConstraints.value
      // Only include if there are actual constraints
      const hasConstraints =
        (ac.preferred_formats?.length ?? 0) > 0 ||
        ac.min_bitrate_kbps !== undefined ||
        ac.avoid_compilations ||
        ac.avoid_live

      if (hasConstraints) {
        return { audio: ac }
      }
    } else if (contentType.value === 'movie' || contentType.value === 'tv') {
      const vc = videoConstraints.value
      const hasConstraints =
        vc.min_resolution !== undefined ||
        vc.preferred_resolution !== undefined ||
        (vc.preferred_sources?.length ?? 0) > 0 ||
        (vc.preferred_codecs?.length ?? 0) > 0 ||
        vc.preferred_language !== undefined ||
        vc.exclude_hardcoded_subs

      if (hasConstraints) {
        return { video: vc }
      }
    }
    return undefined
  })

  const outputConstraints = computed((): OutputConstraints | undefined => {
    if (outputType.value === 'original') {
      return undefined
    }
    if (outputType.value === 'audio') {
      return {
        type: 'audio',
        format: audioFormat.value,
        bitrate_kbps: audioBitrate.value,
      }
    }
    return undefined
  })

  const canProceed = computed(() => {
    switch (currentStep.value) {
      case 'type':
        return contentType.value !== null
      case 'search':
        return hasSelection.value
      case 'constraints':
        return true // Constraints are optional
      case 'details':
        // Description is optional if we have a catalog selection (will be auto-generated)
        const hasDescription = description.value.trim().length > 0 || hasSelection.value
        return hasDescription && destPath.value.trim().length > 0
      case 'review':
        return true
      default:
        return false
    }
  })

  // Actions
  function setContentType(type: ContentType) {
    contentType.value = type
    catalog.clearAll()
    searchQuery.value = ''
    searchYear.value = undefined
  }

  async function performSearch() {
    if (!searchQuery.value.trim()) return

    catalog.error.value = null
    switch (contentType.value) {
      case 'album':
        await catalog.searchReleases(searchQuery.value)
        break
      case 'movie':
        await catalog.searchMovies(searchQuery.value, searchYear.value)
        break
      case 'tv':
        await catalog.searchTvSeries(searchQuery.value)
        break
    }
  }

  function nextStep() {
    const steps: WizardStep[] = ['type', 'search', 'constraints', 'details', 'review']
    const currentIdx = steps.indexOf(currentStep.value)
    if (currentIdx < steps.length - 1 && canProceed.value) {
      currentStep.value = steps[currentIdx + 1]
    }
  }

  function prevStep() {
    const steps: WizardStep[] = ['type', 'search', 'constraints', 'details', 'review']
    const currentIdx = steps.indexOf(currentStep.value)
    if (currentIdx > 0) {
      currentStep.value = steps[currentIdx - 1]
    }
  }

  function goToStep(step: WizardStep) {
    currentStep.value = step
  }

  function buildTicketRequest(): CreateTicketWithCatalogRequest {
    // Auto-generate description from selected item if empty
    let finalDescription = description.value.trim()
    if (!finalDescription && selectedItemSummary.value) {
      const s = selectedItemSummary.value
      finalDescription = `${s.title} - ${s.subtitle}`
    }

    // Auto-generate tags from content type
    const finalTags = [...tags.value]
    if (contentType.value === 'album' && !finalTags.includes('music')) {
      finalTags.push('music')
    }
    if (contentType.value === 'movie' && !finalTags.includes('movie')) {
      finalTags.push('movie')
    }
    if (contentType.value === 'tv' && !finalTags.includes('tv')) {
      finalTags.push('tv')
    }

    return {
      priority: priority.value,
      query_context: {
        tags: finalTags,
        description: finalDescription,
        expected: catalog.buildExpectedContent(),
        catalog_reference: catalog.buildCatalogReference(),
        search_constraints: searchConstraints.value,
      },
      dest_path: destPath.value.trim(),
      output_constraints: outputConstraints.value,
    }
  }

  function reset() {
    currentStep.value = 'type'
    contentType.value = null
    searchQuery.value = ''
    searchYear.value = undefined
    description.value = ''
    tagsInput.value = ''
    destPath.value = ''
    priority.value = 0
    outputType.value = 'original'
    audioFormat.value = 'ogg_vorbis'
    audioBitrate.value = 320

    audioConstraints.value = {
      preferred_formats: [],
      min_bitrate_kbps: undefined,
      avoid_compilations: false,
      avoid_live: false,
    }

    videoConstraints.value = {
      min_resolution: undefined,
      preferred_resolution: undefined,
      preferred_sources: [],
      preferred_codecs: [],
      preferred_language: undefined,
      exclude_hardcoded_subs: false,
    }

    catalog.clearAll()
  }

  // Initialize catalog status on mount
  onMounted(() => {
    catalog.fetchStatus()
  })

  return {
    // Catalog state (exposed for direct access)
    catalogStatus: catalog.status,
    catalogLoading: catalog.loading,
    catalogError: catalog.error,
    isCatalogAvailable: catalog.isAvailable,

    // Search results
    musicBrainzResults: catalog.musicBrainzResults,
    tmdbMovieResults: catalog.tmdbMovieResults,
    tmdbTvResults: catalog.tmdbTvResults,

    // Selected items
    selectedRelease: catalog.selectedRelease,
    selectedMovie: catalog.selectedMovie,
    selectedSeries: catalog.selectedSeries,
    selectedSeason: catalog.selectedSeason,

    // Wizard state
    currentStep,
    contentType,
    searchQuery,
    searchYear,
    hasSelection,
    selectedItemSummary,
    canProceed,

    // Constraints
    audioConstraints,
    videoConstraints,
    searchConstraints,

    // Ticket details
    description,
    tagsInput,
    tags,
    destPath,
    priority,
    outputType,
    audioFormat,
    audioBitrate,
    outputConstraints,

    // Catalog actions
    selectRelease: catalog.selectRelease,
    selectMovie: catalog.selectMovie,
    selectSeries: catalog.selectSeries,
    selectSeason: catalog.selectSeason,

    // Wizard actions
    setContentType,
    performSearch,
    nextStep,
    prevStep,
    goToStep,
    buildTicketRequest,
    reset,
  }
}
