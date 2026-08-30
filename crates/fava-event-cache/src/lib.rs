//! Neutral contract for retaining signed relay-observed event state.

use std::cell::Cell;

use fava_query::{QuerySource, SourceCoverage};
use fava_relay::RelaySessionKey;
use fava_state::{
    EventStateMutation, RelayEvent, event_is_expired, mutations_for_event, mutations_for_expiration,
};
use nostr::event::EventId;
use nostr::filter::Filter;
use nostr::types::Timestamp;
use thiserror::Error;

/// Event-cache provider contract.
pub trait EventCache: QuerySource + Send + Sync {
    /// Return one retained proven completion for this exact source and filter.
    ///
    /// The provider owns retention and coherence. A miss says only that this
    /// provider has no reusable proof for this exact request.
    ///
    /// # Errors
    ///
    /// Returns [`EventCacheError`] when the retained coverage cannot be read.
    fn source_coverage(
        &self,
        session: &RelaySessionKey,
        filter: &Filter,
    ) -> Result<Option<SourceCoverage>, EventCacheError>;

    /// Retain one attributed proven completion.
    ///
    /// Implementations refuse before mutation when their completion bound
    /// would be exceeded, and invalidate retained coverage whenever their
    /// event state can make that completion window stale.
    ///
    /// # Errors
    ///
    /// Returns [`EventCacheError`] when the completion bound is exceeded or
    /// the retention cannot be committed.
    fn retain_source_coverage(&self, coverage: SourceCoverage) -> Result<(), EventCacheError>;

    /// Atomically read current event state, decide a mutation batch, and commit it.
    ///
    /// This is the single serialized event-state writer. Implementations must
    /// hold exclusive write authority across the decision so admission cannot
    /// commit a batch decided from state another writer has already replaced.
    /// An empty batch commits nothing and does not advance the source revision.
    ///
    /// Returns the number of mutations committed.
    ///
    /// # Errors
    ///
    /// Returns [`EventCacheError`] when the read or the atomic commit cannot
    /// complete.
    fn transact(
        &self,
        decide: &dyn Fn(&[RelayEvent]) -> Vec<EventStateMutation>,
    ) -> Result<usize, EventCacheError>;

    /// Admit one verified signed relay event using universal event-state rules.
    ///
    /// The same serialized transaction also sweeps every NIP-40 expiration that
    /// has passed at `now`, so admission is the production owner of expiry.
    ///
    /// # Errors
    ///
    /// Returns [`EventCacheError`] when the current read or atomic mutation
    /// cannot complete.
    fn admit(&self, event: RelayEvent, now: Timestamp) -> Result<bool, EventCacheError> {
        let admitted = Cell::new(false);
        self.transact(&|current| {
            let mut mutations = mutations_for_expiration(current, now);
            let live: Vec<RelayEvent> = current
                .iter()
                .filter(|known| !event_is_expired(known.event().tags.as_slice(), now))
                .cloned()
                .collect();
            let admission = mutations_for_event(&live, event.clone(), now);
            admitted.set(!admission.is_empty());
            mutations.extend(admission);
            mutations
        })?;
        Ok(admitted.get())
    }

    /// Retract every retained event expired at an exact time.
    ///
    /// # Errors
    ///
    /// Returns [`EventCacheError`] when the current read or atomic mutation
    /// cannot complete.
    fn expire(&self, now: Timestamp) -> Result<usize, EventCacheError> {
        self.transact(&|current| mutations_for_expiration(current, now))
    }

    /// Atomically apply one externally decided event-state mutation batch.
    ///
    /// Every [`EventStateMutation::Retract`] in the batch is always applicable: a
    /// provider may refuse an insertion for capacity but never a removal.
    ///
    /// # Errors
    ///
    /// Returns [`EventCacheError`] when the complete batch cannot commit.
    fn commit(&self, mutations: Vec<EventStateMutation>) -> Result<(), EventCacheError>;

    /// Read one currently retained event.
    ///
    /// # Errors
    ///
    /// Returns [`EventCacheError`] when the provider cannot read current state.
    fn event(&self, id: EventId) -> Result<Option<RelayEvent>, EventCacheError>;

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
