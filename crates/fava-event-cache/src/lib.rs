//! Neutral contract for retaining signed relay-observed event state.

use fava_query::QuerySource;
use fava_state::{
    CacheMutation, CachedEvent, Timestamp, admission_mutations, expiration_mutations,
};
use nostr::event::EventId;
use thiserror::Error;

/// Event-cache provider contract.
pub trait EventCache: QuerySource + Send + Sync {
    /// Admit one verified signed relay event using universal event-state rules.
    ///
    /// # Errors
    ///
    /// Returns [`EventCacheError`] when verification, the current read, or the
    /// atomic mutation cannot complete.
    fn admit(&self, event: CachedEvent, now: Timestamp) -> Result<bool, EventCacheError> {
        event
            .event
            .verify()
            .map_err(|error| EventCacheError::Refused(format!("invalid signed event: {error}")))?;
        let mutations = admission_mutations(&self.events()?, event, now);
        if mutations.is_empty() {
            return Ok(false);
        }
        self.commit(mutations)?;
        Ok(true)
    }

    /// Retract every retained event expired at an exact time.
    ///
    /// # Errors
    ///
    /// Returns [`EventCacheError`] when the current read or atomic mutation
    /// cannot complete.
    fn expire(&self, now: Timestamp) -> Result<usize, EventCacheError> {
        let mutations = expiration_mutations(&self.events()?, now);
        let count = mutations.len();
        if count > 0 {
            self.commit(mutations)?;
        }
        Ok(count)
    }

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

    /// Read all currently retained domain values for deterministic state decisions.
    ///
    /// # Errors
    ///
    /// Returns [`EventCacheError`] when the provider cannot read current state.
    fn events(&self) -> Result<Vec<CachedEvent>, EventCacheError>;

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
