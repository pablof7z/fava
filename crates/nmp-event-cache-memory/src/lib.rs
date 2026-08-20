//! Bounded current-process event-cache provider.

use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use nmp_event_cache::{EventCache, EventCacheError};
use nmp_query::{
    CanonicalQuery, OpenedQuerySource, QuerySource, QuerySourceClosed, QuerySourceError,
    SourceChangeFuture, SourceChanges, SourceEvent, SourceKind, SourceRevision, SourceSnapshot,
    SourceStatus,
};
use nmp_state::{CacheMutation, CachedEvent};
use nostr::event::EventId;
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
}

impl EventCache for MemoryEventCache {
    fn commit(&self, mutations: Vec<CacheMutation>) -> Result<(), EventCacheError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| EventCacheError::Refused("cache state lock poisoned".to_owned()))?;
        let mut next = guard.clone();

        for mutation in mutations {
            match mutation {
                CacheMutation::Upsert(incoming) => {
                    if let Some(current) = next.events.get_mut(&incoming.event.id) {
                        if current.event != incoming.event {
                            return Err(EventCacheError::Refused(
                                "same event id carried a different signed body".to_owned(),
                            ));
                        }
                        current.merge_evidence(&incoming.evidence);
                    } else {
                        if next.events.len() == self.capacity.get() {
                            return Err(EventCacheError::Refused(format!(
                                "bounded event cache capacity {} reached",
                                self.capacity
                            )));
                        }
                        next.events.insert(incoming.event.id, incoming);
                    }
                }
                CacheMutation::Retract(id) => {
                    next.events.remove(&id);
                }
            }
        }

        next.revision = next
            .revision
            .checked_add(1)
            .ok_or_else(|| EventCacheError::Refused("source revision exhausted".to_owned()))?;
        let snapshot = Arc::new(Self::snapshot(&next));
        *guard = next;
        self.latest.send_replace(snapshot);
        Ok(())
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
    fn open(&self, _query: &CanonicalQuery) -> Result<OpenedQuerySource, QuerySourceError> {
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
    use nmp_event_cache::EventCache;
    use nmp_state::{AccessContext, RelayEvidence, RelaySessionKey, RelayUrl, Timestamp};
    use nostr::event::{EventBuilder, FinalizeEvent, Kind};
    use nostr::key::Keys;

    use super::*;

    #[test]
    fn failed_capacity_batch_is_atomic() {
        let cache = MemoryEventCache::bounded(NonZeroUsize::new(1).expect("non-zero"));
        let keys = Keys::generate();
        let relay = RelayUrl::parse("wss://relay.example").expect("relay url");
        let evidence = RelayEvidence::one(
            RelaySessionKey::new(relay, AccessContext::public()),
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
}
