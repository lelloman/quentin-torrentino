pub mod audit;
pub mod auth;
pub mod catalog;
pub mod config;
pub mod searcher;
pub mod textbrain;
pub mod ticket;
pub mod torrent_client;

pub use audit::{
    create_audit_system, AuditError, AuditEvent, AuditEventEnvelope, AuditFilter, AuditHandle,
    AuditRecord, AuditStore, AuditWriter, SqliteAuditStore,
};
pub use auth::{
    create_authenticator, AuthError, AuthRequest, Authenticator, Identity, NoneAuthenticator,
};
pub use config::{
    load_config, load_config_from_str, validate_config, AuthMethod, Config, ConfigError,
    DatabaseConfig, JackettConfig, LibrqbitConfig, QBittorrentConfig, SanitizedConfig,
    SearcherBackend, SearcherConfig, TorrentClientBackend, TorrentClientConfig,
};
pub use searcher::{
    deduplicate_results, IndexerStatus, JackettSearcher, RawTorrentResult, SearchCategory,
    SearchError, SearchQuery, SearchResult, Searcher, TorrentCandidate, TorrentFile, TorrentSource,
};
pub use ticket::{
    AcquisitionPhase, CompletionStats, CreateTicketRequest, QueryContext, SelectedCandidate,
    SqliteTicketStore, Ticket, TicketError, TicketFilter, TicketState, TicketStore,
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
