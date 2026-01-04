// Allow some clippy lints that are too noisy for this codebase
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::manual_range_contains)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::collapsible_if)]

pub mod audit;
pub mod auth;
pub mod catalog;
pub mod config;
pub mod content;
pub mod converter;
pub mod external_catalog;
pub mod metrics;
pub mod orchestrator;
pub mod placer;
pub mod processor;
pub mod searcher;
pub mod textbrain;
pub mod ticket;
pub mod torrent_client;

/// Testing utilities and mock implementations for E2E tests.
///
/// This module provides mock implementations of all external service traits,
/// allowing comprehensive E2E testing without real infrastructure.
pub mod testing;

pub use audit::{
    create_audit_system, AuditError, AuditEvent, AuditEventEnvelope, AuditFilter, AuditHandle,
    AuditRecord, AuditStore, AuditWriter, SqliteAuditStore,
};
pub use auth::{
    create_authenticator, ApiKeyAuthenticator, AuthError, AuthRequest, Authenticator, Identity,
    NoneAuthenticator,
};
pub use catalog::{
    CachedTorrent, CachedTorrentFile, CachedTorrentSource, CatalogError, CatalogSearchQuery,
    CatalogStats, SearchMode, SqliteCatalog, TorrentCatalog,
};
pub use config::{
    load_config, load_config_from_str, validate_config, AuthConfig, AuthMethod, Config,
    ConfigError, DatabaseConfig, ExternalCatalogsConfig, JackettConfig, LibrqbitConfig,
    QBittorrentConfig, SanitizedConfig, SearcherBackend, SearcherConfig, ServerConfig,
    TorrentClientBackend, TorrentClientConfig,
};
pub use content::{
    // Error types
    ContentError,
    // Result types
    PostProcessResult,
};
pub use converter::{
    // Types
    AudioConstraints,
    AudioFormat,
    ContainerFormat,
    ConversionConstraints,
    ConversionJob,
    ConversionProgress,
    ConversionResult,
    // Traits
    Converter,
    // Configuration
    ConverterConfig,
    // Error types
    ConverterError,
    EmbeddedMetadata,
    // Capabilities
    EncoderCapabilities,
    // Implementations
    FfmpegConverter,
    MediaInfo,
    VideoConstraints,
    VideoFormat,
};
pub use external_catalog::{
    // Clients
    CombinedCatalogClient,
    // Trait
    ExternalCatalog,
    // Error types
    ExternalCatalogError,
    MusicBrainzClient,
    // Configuration
    MusicBrainzConfig,
    // MusicBrainz types
    MusicBrainzRelease,
    MusicBrainzTrack,
    TmdbClient,
    TmdbConfig,
    // TMDB types
    TmdbEpisode,
    TmdbMovie,
    TmdbSeason,
    TmdbSeasonSummary,
    TmdbSeries,
};
pub use orchestrator::{
    // Types
    ActiveDownload,
    // Configuration
    OrchestratorConfig,
    OrchestratorError,
    OrchestratorStatus,
    // Orchestrator
    TicketOrchestrator,
};
pub use placer::{
    // Types
    ChecksumType,
    FilePlacement,
    // Implementations
    FsPlacer,
    PlacedFile,
    PlacementJob,
    PlacementProgress,
    PlacementResult,
    // Traits
    Placer,
    // Configuration
    PlacerConfig,
    // Error types
    PlacerError,
    RollbackFile,
    RollbackPlan,
    RollbackResult,
};
pub use processor::{
    // Error types
    PipelineError,
    // Types
    PipelineJob,
    PipelineMetadata,
    // Pipeline
    PipelineProcessor,
    PipelineProgress,
    PipelineResult,
    PipelineStatus,
    PlacedFileInfo,
    PoolStatus,
    // Configuration
    ProcessorConfig,
    RetryConfig,
    SourceFile,
};
pub use searcher::{
    deduplicate_results, FileEnricher, FileEnricherConfig, IndexerStatus, JackettSearcher,
    RawTorrentResult, SearchCategory, SearchError, SearchQuery, SearchResult, Searcher,
    TorrentCandidate, TorrentFile, TorrentSource,
};
pub use textbrain::{
    // Result types
    AcquisitionResult,
    // LLM client types
    AnthropicClient,
    // Traits
    CandidateMatcher,
    CompletionRequest,
    CompletionResponse,
    // Dumb implementations
    DumbMatcher,
    DumbMatcherConfig,
    DumbQueryBuilder,
    DumbQueryBuilderConfig,
    FileMapping,
    LlmClient,
    // Configuration
    LlmConfig,
    LlmError,
    LlmProvider,
    LlmUsage,
    MatchResult,
    OllamaClient,
    QueryBuildResult,
    QueryBuilder,
    ScoredCandidate,
    ScoredCandidateSummary,
    // Coordinator
    TextBrain,
    TextBrainConfig,
    TextBrainError,
    TextBrainMode,
};
pub use ticket::{
    AcquisitionPhase, AudioSearchConstraints, CatalogReference, CompletionStats,
    CreateTicketRequest, ExpectedContent, ExpectedTrack, LanguagePreference, LanguagePriority,
    OutputConstraints, QueryContext, Resolution, SearchConstraints, SelectedCandidate,
    SqliteTicketStore, Ticket, TicketError, TicketFilter, TicketState, TicketStore, TmdbMediaType,
    VideoCodec, VideoSearchConstraints, VideoSource,
};
pub use torrent_client::{
    AddTorrentRequest, AddTorrentResult, LibrqbitClient, QBittorrentClient, TorrentClient,
    TorrentClientError, TorrentFilters, TorrentInfo, TorrentState,
};
