//! Ticket system for tracking content acquisition requests.

mod sqlite_store;
mod store;
mod types;

pub use sqlite_store::SqliteTicketStore;
pub use store::{CreateTicketRequest, TicketError, TicketFilter, TicketStore};
pub use types::{
    AcquisitionPhase, AudioSearchConstraints, CatalogReference, CompletionStats, ExpectedContent,
    ExpectedTrack, OutputConstraints, QueryContext, Resolution, SearchConstraints,
    SelectedCandidate, Ticket, TicketState, TmdbMediaType, VideoCodec, VideoSearchConstraints,
    VideoSource,
};
