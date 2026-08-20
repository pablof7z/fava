//! Neutral contract for retaining signed relay-observed event state.

use fava_query::QuerySource;
use fava_state::{CacheMutation, CachedEvent};
use nostr::event::EventId;
use thiserror::Error;

/// Event-cache provider contract.
pub trait EventCache: QuerySource + Send + Sync {
    /// Atomically apply one event-state mutation batch.
    ///
    /// # Errors
    ///
    /// Returns [`EventCacheError`] when the complete batch cannot commit.
    fn commit(&self, mutations: Vec<CacheMutation>) -> Result<(), EventCacheError>;

    /// Read one currently retained event.
    ///
    /// # Errors
    ///
    /// Returns [`EventCacheError`] when the provider cannot read current state.
    fn event(&self, id: EventId) -> Result<Option<CachedEvent>, EventCacheError>;

    /// Number of retained signed events.
    ///
    /// # Errors
    ///
    /// Returns [`EventCacheError`] when the provider cannot read current state.
    fn len(&self) -> Result<usize, EventCacheError>;

    /// Whether the cache currently retains no signed events.
    ///
    /// # Errors
    ///
    /// Returns [`EventCacheError`] when the provider cannot read current state.
    fn is_empty(&self) -> Result<bool, EventCacheError> {
        self.len().map(|len| len == 0)
    }
}

/// Scoped cache operation failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum EventCacheError {
    /// Provider has closed.
    #[error("event cache is closed")]
    Closed,
    /// Provider refused a mutation before commit.
    #[error("event cache refused operation: {0}")]
    Refused(String),
}
