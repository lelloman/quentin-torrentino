pub mod audit;
pub mod auth;
pub mod catalog;
pub mod config;
pub mod content;
pub mod converter;
pub mod external_catalog;
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
pub use config::{
    load_config, load_config_from_str, validate_config, AuthConfig, AuthMethod, Config,
    ConfigError, DatabaseConfig, ExternalCatalogsConfig, JackettConfig, LibrqbitConfig,
    QBittorrentConfig, SanitizedConfig, SearcherBackend, SearcherConfig, TorrentClientBackend,
    TorrentClientConfig,
};
pub use searcher::{
    deduplicate_results, FileEnricher, FileEnricherConfig, IndexerStatus, JackettSearcher,
    RawTorrentResult, SearchCategory, SearchError, SearchQuery, SearchResult, Searcher,
    TorrentCandidate, TorrentFile, TorrentSource,
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
pub use catalog::{
    CachedTorrent, CachedTorrentFile, CachedTorrentSource, CatalogError, CatalogSearchQuery,
    CatalogStats, SearchMode, SqliteCatalog, TorrentCatalog,
};
pub use textbrain::{
    // LLM client types
    AnthropicClient, CompletionRequest, CompletionResponse, LlmClient, LlmError, LlmUsage,
    OllamaClient,
    // Configuration
    LlmConfig, LlmProvider, TextBrainConfig, TextBrainMode,
    // Traits
    CandidateMatcher, QueryBuilder, TextBrainError,
    // Dumb implementations
    DumbMatcher, DumbMatcherConfig, DumbQueryBuilder, DumbQueryBuilderConfig,
    // Result types
    AcquisitionResult, FileMapping, MatchResult, QueryBuildResult, ScoredCandidate,
    ScoredCandidateSummary,
    // Coordinator
    TextBrain,
};
pub use converter::{
    // Traits
    Converter,
    // Configuration
    ConverterConfig,
    // Error types
    ConverterError,
    // Implementations
    FfmpegConverter,
    // Capabilities
    EncoderCapabilities,
    // Types
    AudioConstraints, AudioFormat, ContainerFormat, ConversionConstraints, ConversionJob,
    ConversionProgress, ConversionResult, EmbeddedMetadata, MediaInfo, VideoConstraints,
    VideoFormat,
};
pub use placer::{
    // Traits
    Placer,
    // Configuration
    PlacerConfig,
    // Error types
    PlacerError,
    // Implementations
    FsPlacer,
    // Types
    ChecksumType, FilePlacement, PlacedFile, PlacementJob, PlacementProgress, PlacementResult,
    RollbackFile, RollbackPlan, RollbackResult,
};
pub use processor::{
    // Configuration
    ProcessorConfig, RetryConfig,
    // Error types
    PipelineError,
    // Pipeline
    PipelineProcessor,
    // Types
    PipelineJob, PipelineMetadata, PipelineProgress, PipelineResult, PipelineStatus,
    PlacedFileInfo, PoolStatus, SourceFile,
};
pub use orchestrator::{
    // Configuration
    OrchestratorConfig,
    // Orchestrator
    TicketOrchestrator,
    // Types
    ActiveDownload, OrchestratorError, OrchestratorStatus,
};
pub use content::{
    // Error types
    ContentError,
    // Result types
    PostProcessResult,
};
pub use external_catalog::{
    // Trait
    ExternalCatalog,
    // Error types
    ExternalCatalogError,
    // Clients
    CombinedCatalogClient, MusicBrainzClient, TmdbClient,
    // Configuration
    MusicBrainzConfig, TmdbConfig,
    // MusicBrainz types
    MusicBrainzRelease, MusicBrainzTrack,
    // TMDB types
    TmdbEpisode, TmdbMovie, TmdbSeason, TmdbSeasonSummary, TmdbSeries,
};
