//! Bounded current-process event-cache provider.

use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use fava_event_cache::{EventCache, EventCacheError};
use fava_query::{
    OpenedQuerySource, Query, QuerySource, QuerySourceClosed, QuerySourceError, SourceChangeFuture,
    SourceChanges, SourceCoverage, SourceEvent, SourceKind, SourceRetraction, SourceRevision,
    SourceSnapshot, SourceStatus,
};
use fava_state::{EventStateMutation, RelayEvent, RetractionCause};
use nostr::event::{EventId, Kind};
use nostr::filter::Filter;
use nostr::types::RelayUrl;
use tokio::sync::watch;

/// Bounded in-memory cache with coherent latest-state observations.
pub struct MemoryEventCache {
    capacity: NonZeroUsize,
    state: Mutex<CacheState>,
    latest: watch::Sender<Arc<SourceSnapshot>>,
}

/// The events this cache holds, keyed by event and relay session, and the
/// revision they reached.
#[derive(Clone, Debug, Default)]
struct CacheState {
    revision: u64,
    events: BTreeMap<(EventId, RelayUrl), RelayEvent>,
    /// Retractions applied to reach `revision`. Reset by every commit, so a
    /// snapshot reports exactly what its own revision removed.
    retractions: Vec<SourceRetraction>,
    /// Proven EOSE facts that remain coherent with the retained event state.
    coverage: Vec<SourceCoverage>,
}

impl Default for MemoryEventCache {
    fn default() -> Self {
        Self::bounded(NonZeroUsize::new(10_000).expect("constant is non-zero"))
    }
}

impl MemoryEventCache {
    /// Create an empty cache with an exact maximum retained-event count.
    #[must_use]
    pub fn bounded(capacity: NonZeroUsize) -> Self {
        let (latest, _) = watch::channel(Arc::new(SourceSnapshot::empty(SourceKind::EventCache)));
        Self {
            capacity,
            state: Mutex::new(CacheState::default()),
            latest,
        }
    }

    fn snapshot(state: &CacheState) -> SourceSnapshot {
        SourceSnapshot {
            kind: SourceKind::EventCache,
            revision: SourceRevision(state.revision),
            status: SourceStatus::Open,
            events: state
                .events
                .values()
                .cloned()
                .map(SourceEvent::Relay)
                .collect(),
            retractions: state.retractions.clone(),
        }
    }

    /// Apply one batch to a working copy of current state.
    ///
    /// Retractions apply first and unconditionally: a bounded cache refuses
    /// insertions, never removals, so removing state can never be blocked by
    /// the capacity that removing state would relieve. A NIP-09 deletion event
    /// is admitted even at capacity by evicting the oldest retained
    /// non-deletion event, because losing the tombstone would allow the deleted
    /// event to be resurrected.
    fn apply(
        &self,
        current: &CacheState,
        mutations: Vec<EventStateMutation>,
    ) -> Result<CacheState, EventCacheError> {
        let mut next = current.clone();
        next.retractions = Vec::new();
        let mut upserts = Vec::new();
        for mutation in mutations {
            match mutation {
                EventStateMutation::Retract {
                    event_id,
                    session,
                    cause,
                } => {
                    // Report the removal only when this revision actually
                    // removed something; a retraction for an id the cache never
                    // retained is not a fact about this cache's state.
                    if next.events.remove(&(event_id, session)).is_some() {
                        next.retractions
                            .push(SourceRetraction::new(event_id, cause));
                    }
                }
                EventStateMutation::Upsert(incoming) => upserts.push(incoming),
            }
        }

        for incoming in upserts {
            incoming.event().verify().map_err(|error| {
                EventCacheError::Refused(format!("invalid signed event: {error}"))
            })?;
            let key = (incoming.event().id, incoming.occurrence().session.clone());
            if let Some(retained) = next.events.get(&key) {
                if incoming.occurrence().observed_at < retained.occurrence().observed_at {
                    next.events.insert(key, incoming);
                }
                continue;
            }
            if next.events.len() >= self.capacity.get() {
                if incoming.event().kind != Kind::EventDeletion {
                    return Err(EventCacheError::Refused(format!(
                        "bounded event cache capacity {} reached",
                        self.capacity
                    )));
                }
                let evicted = next
                    .events
                    .iter()
                    .filter(|(_, retained)| retained.event().kind != Kind::EventDeletion)
                    .min_by_key(|(_, retained)| (retained.event().created_at, retained.event().id))
                    .map(|(key, _)| key.clone())
                    .ok_or_else(|| {
                        EventCacheError::Refused(format!(
                            "bounded event cache capacity {} holds only deletions",
                            self.capacity
                        ))
                    })?;
                next.events.remove(&evicted);
                // The provider removed retained state under its own bound, not
                // under a Nostr rule. An application may still re-acquire this
                // event from a relay, which is exactly what it must not do for
                // a NIP-09 deletion, so the two can never be reported alike.
                next.retractions
                    .push(SourceRetraction::new(evicted.0, RetractionCause::Evicted));
            }
            next.events.insert(key, incoming);
        }
        // An event-state transition can change the answer to an arbitrary
        // retained filter.  Forgetting coverage is conservative and keeps the
        // cache's reusable proof coherent without a second invalidation owner.
        if next.events != current.events {
            next.coverage.clear();
        }
        Ok(next)
    }

    fn publish(
        guard: &mut CacheState,
        next: CacheState,
        sender: &watch::Sender<Arc<SourceSnapshot>>,
    ) -> Result<(), EventCacheError> {
        let mut next = next;
        next.revision = next
            .revision
            .checked_add(1)
            .ok_or_else(|| EventCacheError::Refused("source revision exhausted".to_owned()))?;
        let snapshot = Arc::new(Self::snapshot(&next));
        *guard = next;
        sender.send_replace(snapshot);
        Ok(())
    }
}

impl EventCache for MemoryEventCache {
    fn source_coverage(
        &self,
        session: &RelayUrl,
        filter: &Filter,
    ) -> Result<Option<SourceCoverage>, EventCacheError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| EventCacheError::Refused("cache state lock poisoned".to_owned()))?;
        Ok(guard
            .coverage
            .iter()
            .find(|coverage| &coverage.session == session && &coverage.filter == filter)
            .cloned())
    }

    fn retain_source_coverage(&self, coverage: SourceCoverage) -> Result<(), EventCacheError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| EventCacheError::Refused("cache state lock poisoned".to_owned()))?;
        let mut next = guard.clone();
        if let Some(existing) = next.coverage.iter_mut().find(|existing| {
            existing.session == coverage.session && existing.filter == coverage.filter
        }) {
            *existing = coverage;
        } else {
            if next.coverage.len() >= self.capacity.get() {
                next.coverage.remove(0);
            }
            next.coverage.push(coverage);
        }
        *guard = next;
        Ok(())
    }

    fn transact(
        &self,
        decide: &dyn Fn(&[RelayEvent]) -> Vec<EventStateMutation>,
    ) -> Result<usize, EventCacheError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| EventCacheError::Refused("cache state lock poisoned".to_owned()))?;
        let current: Vec<RelayEvent> = guard.events.values().cloned().collect();
        let mutations = decide(&current);
        let count = mutations.len();
        if count == 0 {
            return Ok(0);
        }
        let next = self.apply(&guard, mutations)?;
        Self::publish(&mut guard, next, &self.latest)?;
        Ok(count)
    }

    fn commit(&self, mutations: Vec<EventStateMutation>) -> Result<(), EventCacheError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| EventCacheError::Refused("cache state lock poisoned".to_owned()))?;
        let next = self.apply(&guard, mutations)?;
        Self::publish(&mut guard, next, &self.latest)
    }

    fn event(&self, id: EventId) -> Result<Option<RelayEvent>, EventCacheError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| EventCacheError::Refused("cache state lock poisoned".to_owned()))?;
        Ok(guard
            .events
            .iter()
            .find_map(|((event_id, _), event)| (*event_id == id).then(|| event.clone())))
    }

    fn len(&self) -> Result<usize, EventCacheError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| EventCacheError::Refused("cache state lock poisoned".to_owned()))?;
        Ok(guard.events.len())
    }
}

impl QuerySource for MemoryEventCache {
    fn open(&self, _query: &Query) -> Result<OpenedQuerySource, QuerySourceError> {
        let receiver = self.latest.subscribe();
        let initial = receiver.borrow().as_ref().clone();
        Ok(OpenedQuerySource {
            initial,
            changes: Box::new(WatchChanges {
                receiver,
                closed: false,
            }),
        })
    }
}

struct WatchChanges {
    receiver: watch::Receiver<Arc<SourceSnapshot>>,
    closed: bool,
}

impl SourceChanges for WatchChanges {
    fn next_change(&mut self) -> SourceChangeFuture<'_> {
        Box::pin(async move {
            if self.closed {
                return Err(QuerySourceClosed::local_close());
            }
            if self.receiver.changed().await.is_err() {
                // The cache itself was dropped. That is a clean end of the
                // provider, not a failure, and only a clean end is evidence
                // that the provider had nothing further to say.
                self.closed = true;
                return Err(QuerySourceClosed::provider_closed());
            }
            Ok(self.receiver.borrow_and_update().as_ref().clone())
        })
    }

    fn close(&mut self) {
        self.closed = true;
    }
}
