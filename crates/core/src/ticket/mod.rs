//! Ticket system for tracking content acquisition requests.

mod sqlite_store;
mod store;
mod types;

pub use sqlite_store::SqliteTicketStore;
pub use store::{CreateTicketRequest, TicketError, TicketFilter, TicketStore};
pub use types::{
    AcquisitionPhase, AudioSearchConstraints, CatalogReference, CompletionStats, ExpectedContent,
    ExpectedTrack, LanguagePreference, LanguagePriority, OutputConstraints, QueryContext,
    Resolution, RetryPhase, SearchConstraints, SelectedCandidate, Ticket, TicketState,
    TmdbMediaType, VideoCodec, VideoSearchConstraints, VideoSource,
};
