pub mod audit;
pub mod auth;
pub mod config;
pub mod searcher;
pub mod ticket;

pub use audit::{
    create_audit_system, AuditError, AuditEvent, AuditEventEnvelope, AuditFilter, AuditHandle,
    AuditRecord, AuditStore, AuditWriter, SqliteAuditStore,
};
pub use auth::{
    create_authenticator, AuthError, AuthRequest, Authenticator, Identity, NoneAuthenticator,
};
pub use config::{
    load_config, load_config_from_str, validate_config, AuthMethod, Config, ConfigError,
    DatabaseConfig, SanitizedConfig,
};
pub use searcher::{
    IndexerRateLimitConfig, IndexerStatus, RateLimitStatus, RateLimiterPool, RawTorrentResult,
    SearchCategory, SearchError, SearchQuery, SearchResult, Searcher, TokenBucket,
    TorrentCandidate, TorrentFile, TorrentSource,
};
pub use ticket::{
    CreateTicketRequest, QueryContext, SqliteTicketStore, Ticket, TicketError, TicketFilter,
    TicketState, TicketStore,
};
