//! Bounded current-process event-cache provider.

use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use fava_event_cache::{EventCache, EventCacheError};
use fava_query::{
    OpenedQuerySource, Query, QuerySource, QuerySourceClosed, QuerySourceError, SourceChangeFuture,
    SourceChanges, SourceEvent, SourceKind, SourceRevision, SourceSnapshot, SourceStatus,
};
use fava_state::{CacheMutation, CachedEvent};
use nostr::event::{EventId, Kind};
use tokio::sync::watch;

/// Bounded in-memory cache with coherent latest-state observations.
pub struct MemoryEventCache {
    capacity: NonZeroUsize,
    state: Mutex<CacheState>,
    latest: watch::Sender<Arc<SourceSnapshot>>,
}

#[derive(Clone, Debug, Default)]
struct CacheState {
    revision: u64,
    events: BTreeMap<EventId, CachedEvent>,
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
                .map(SourceEvent::Cached)
                .collect(),
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
        mutations: Vec<CacheMutation>,
    ) -> Result<CacheState, EventCacheError> {
        let mut next = current.clone();
        let mut upserts = Vec::new();
        for mutation in mutations {
            match mutation {
                CacheMutation::Retract { event_id, .. } => {
                    next.events.remove(&event_id);
                }
                CacheMutation::Upsert(incoming) => upserts.push(incoming),
            }
        }

        for incoming in upserts {
            incoming.event.verify().map_err(|error| {
                EventCacheError::Refused(format!("invalid signed event: {error}"))
            })?;
            if let Some(retained) = next.events.get_mut(&incoming.event.id) {
                if retained.event != incoming.event {
                    return Err(EventCacheError::Refused(
                        "same event id carried a different signed body".to_owned(),
                    ));
                }
                retained.merge_evidence(&incoming.evidence);
                continue;
            }
            if next.events.len() >= self.capacity.get() {
                if incoming.event.kind != Kind::EventDeletion {
                    return Err(EventCacheError::Refused(format!(
                        "bounded event cache capacity {} reached",
                        self.capacity
                    )));
                }
                let evicted = next
                    .events
                    .values()
                    .filter(|retained| retained.event.kind != Kind::EventDeletion)
                    .min_by_key(|retained| (retained.event.created_at, retained.event.id))
                    .map(|retained| retained.event.id)
                    .ok_or_else(|| {
                        EventCacheError::Refused(format!(
                            "bounded event cache capacity {} holds only deletions",
                            self.capacity
                        ))
                    })?;
                next.events.remove(&evicted);
            }
            next.events.insert(incoming.event.id, incoming);
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
    fn transact(
        &self,
        decide: &dyn Fn(&[CachedEvent]) -> Vec<CacheMutation>,
    ) -> Result<usize, EventCacheError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| EventCacheError::Refused("cache state lock poisoned".to_owned()))?;
        let current: Vec<CachedEvent> = guard.events.values().cloned().collect();
        let mutations = decide(&current);
        let count = mutations.len();
        if count == 0 {
            return Ok(0);
        }
        let next = self.apply(&guard, mutations)?;
        Self::publish(&mut guard, next, &self.latest)?;
        Ok(count)
    }

    fn commit(&self, mutations: Vec<CacheMutation>) -> Result<(), EventCacheError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| EventCacheError::Refused("cache state lock poisoned".to_owned()))?;
        let next = self.apply(&guard, mutations)?;
        Self::publish(&mut guard, next, &self.latest)
    }

    fn event(&self, id: EventId) -> Result<Option<CachedEvent>, EventCacheError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| EventCacheError::Refused("cache state lock poisoned".to_owned()))?;
        Ok(guard.events.get(&id).cloned())
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
            if self.closed || self.receiver.changed().await.is_err() {
                return Err(QuerySourceClosed);
            }
            Ok(self.receiver.borrow_and_update().as_ref().clone())
        })
    }

    fn close(&mut self) {
        self.closed = true;
    }
}

#[cfg(test)]
mod tests {
    use fava_event_cache::EventCache;
    use fava_state::{RelayAccess, RelayEvidence, RelaySessionKey, RelayUrl, Timestamp};
    use nostr::event::{EventBuilder, FinalizeEvent, Kind};
    use nostr::key::Keys;

    use super::*;

    #[test]
    fn failed_capacity_batch_is_atomic() {
        let cache = MemoryEventCache::bounded(NonZeroUsize::new(1).expect("non-zero"));
        let keys = Keys::generate();
        let relay = RelayUrl::parse("wss://relay.example").expect("relay url");
        let evidence = RelayEvidence::one(
            RelaySessionKey::new(relay, RelayAccess::public()),
            Timestamp::from(1),
        );
        let first = EventBuilder::new(Kind::TextNote, "first")
            .finalize(&keys)
            .expect("event signs");
        let second = EventBuilder::new(Kind::TextNote, "second")
            .finalize(&keys)
            .expect("event signs");

        let result = cache.commit(vec![
            CacheMutation::Upsert(CachedEvent::new(first, evidence.clone())),
            CacheMutation::Upsert(CachedEvent::new(second, evidence)),
        ]);

        assert!(result.is_err());
        assert_eq!(cache.len().expect("cache readable"), 0);
    }

    #[test]
    fn invalid_signed_event_is_refused_without_mutation() {
        let cache = MemoryEventCache::default();
        let keys = Keys::generate();
        let relay = RelayUrl::parse("wss://relay.example").expect("relay url");
        let evidence = RelayEvidence::one(
            RelaySessionKey::new(relay, RelayAccess::public()),
            Timestamp::from(1),
        );
        let mut event = EventBuilder::new(Kind::TextNote, "signed")
            .finalize(&keys)
            .expect("event signs");
        event.content = "tampered after signing".to_owned();

        let result = cache.commit(vec![CacheMutation::Upsert(CachedEvent::new(
            event, evidence,
        ))]);

        assert!(result.is_err());
        assert!(cache.is_empty().expect("cache readable"));
    }
}
