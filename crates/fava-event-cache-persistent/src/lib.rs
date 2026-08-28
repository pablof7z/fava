//! Durable redb-backed event-cache provider.
//!
//! Events admitted through [`RedbEventCache`] survive process kill and
//! reload on open. The cache is bounded by an exact capacity: inserts
//! beyond capacity are refused except for kind-5 tombstones, which evict
//! the oldest non-deletion entry to preserve NIP-09 correctness.

use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::{Arc, Mutex};

use fava_event_cache::{EventCache, EventCacheError};
use fava_query::{
    OpenedQuerySource, Query, QuerySource, QuerySourceClosed, QuerySourceError, SourceChangeFuture,
    SourceChanges, SourceCoverage, SourceEvent, SourceKind, SourceRetraction, SourceRevision,
    SourceSnapshot, SourceStatus,
};
use fava_relay::RelaySessionKey;
use fava_state::{EventStateMutation, RelayEvent, RetractionCause};
use nostr::event::{EventId, Kind};
use nostr::filter::Filter;
use redb::Database;
use tokio::sync::watch;

mod schema;

fn refused(e: impl std::fmt::Display) -> EventCacheError {
    EventCacheError::Refused(e.to_string())
}

/// Durable event cache backed by a redb database file.
///
/// Events written through this provider survive SIGKILL and reload on
/// [`RedbEventCache::open`]. The in-process snapshot is rebuilt from the
/// redb table on open; every subsequent mutation is committed with
/// [`redb::Durability::Immediate`] before the in-memory state is updated.
pub struct RedbEventCache {
    capacity: NonZeroUsize,
    database: Arc<Database>,
    state: Mutex<CacheState>,
    latest: watch::Sender<Arc<SourceSnapshot>>,
}

/// In-memory mirror of the committed redb table, swapped in only after the
/// matching write has landed on disk.
#[derive(Clone, Debug, Default)]
struct CacheState {
    revision: u64,
    events: BTreeMap<(EventId, RelaySessionKey), RelayEvent>,
    retractions: Vec<SourceRetraction>,
    coverage: Vec<SourceCoverage>,
}

impl RedbEventCache {
    /// Open or create a durable event cache at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`EventCacheError`] when the database cannot be opened or
    /// the persisted schema is incompatible.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, EventCacheError> {
        let capacity = NonZeroUsize::new(10_000).expect("constant is non-zero");
        Self::open_bounded(path, capacity)
    }

    /// Open with an exact capacity bound.
    ///
    /// # Errors
    ///
    /// Returns [`EventCacheError`] when the database cannot be opened or
    /// the persisted schema is incompatible.
    pub fn open_bounded(
        path: impl AsRef<Path>,
        capacity: NonZeroUsize,
    ) -> Result<Self, EventCacheError> {
        let path = path.as_ref();
        let is_new = !path.exists();
        let database = Arc::new(Database::create(path).map_err(refused)?);
        schema::initialize(&database, is_new).map_err(refused)?;
        let events = schema::load(&database).map_err(refused)?;
        let state = CacheState {
            revision: 0,
            events,
            retractions: Vec::new(),
            coverage: Vec::new(),
        };
        let (latest, _) = watch::channel(Arc::new(cache_snapshot(&state)));
        Ok(Self {
            capacity,
            database,
            state: Mutex::new(state),
            latest,
        })
    }

    fn apply(
        &self,
        current: &CacheState,
        mutations: Vec<EventStateMutation>,
    ) -> Result<(CacheState, Vec<RelayEvent>, Vec<(EventId, RelaySessionKey)>), EventCacheError>
    {
        let mut next = current.clone();
        next.retractions = Vec::new();
        let mut inserted: Vec<RelayEvent> = Vec::new();
        let mut removed: Vec<(EventId, RelaySessionKey)> = Vec::new();
        let mut upserts: Vec<RelayEvent> = Vec::new();

        for mutation in mutations {
            match mutation {
                EventStateMutation::Retract {
                    event_id,
                    session,
                    cause,
                } => {
                    if next.events.remove(&(event_id, session.clone())).is_some() {
                        next.retractions
                            .push(SourceRetraction::new(event_id, cause));
                        removed.push((event_id, session));
                    }
                }
                EventStateMutation::Upsert(incoming) => upserts.push(incoming),
            }
        }

        for incoming in upserts {
            incoming
                .event()
                .verify()
                .map_err(|e| EventCacheError::Refused(format!("invalid signed event: {e}")))?;
            let key = (incoming.event().id, incoming.occurrence().session.clone());
            if let Some(retained) = next.events.get(&key) {
                if incoming.occurrence().observed_at < retained.occurrence().observed_at {
                    next.events.insert(key, incoming.clone());
                    inserted.push(incoming);
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
                    .filter(|(_, r)| r.event().kind != Kind::EventDeletion)
                    .min_by_key(|(_, r)| (r.event().created_at, r.event().id))
                    .map(|(k, _)| k.clone())
                    .ok_or_else(|| {
                        EventCacheError::Refused(format!(
                            "bounded event cache capacity {} holds only deletions",
                            self.capacity
                        ))
                    })?;
                next.events.remove(&evicted);
                next.retractions
                    .push(SourceRetraction::new(evicted.0, RetractionCause::Evicted));
                removed.push(evicted);
            }
            next.events.insert(key, incoming.clone());
            inserted.push(incoming);
        }

        if next.events != current.events {
            next.coverage.clear();
        }

        Ok((next, inserted, removed))
    }

    fn persist_and_publish(
        guard: &mut CacheState,
        next: CacheState,
        inserted: Vec<RelayEvent>,
        removed: Vec<(EventId, RelaySessionKey)>,
        database: &Database,
        sender: &watch::Sender<Arc<SourceSnapshot>>,
    ) -> Result<(), EventCacheError> {
        schema::apply_diff(database, &inserted, &removed).map_err(refused)?;
        let mut next = next;
        next.revision = next
            .revision
            .checked_add(1)
            .ok_or_else(|| EventCacheError::Refused("source revision exhausted".to_owned()))?;
        let snap = Arc::new(cache_snapshot(&next));
        *guard = next;
        sender.send_replace(snap);
        Ok(())
    }
}

fn cache_snapshot(state: &CacheState) -> SourceSnapshot {
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

impl EventCache for RedbEventCache {
    fn source_coverage(
        &self,
        session: &RelaySessionKey,
        filter: &Filter,
    ) -> Result<Option<SourceCoverage>, EventCacheError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| EventCacheError::Refused("state lock poisoned".to_owned()))?;
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
            .map_err(|_| EventCacheError::Refused("state lock poisoned".to_owned()))?;
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
            .map_err(|_| EventCacheError::Refused("state lock poisoned".to_owned()))?;
        let current: Vec<RelayEvent> = guard.events.values().cloned().collect();
        let mutations = decide(&current);
        let count = mutations.len();
        if count == 0 {
            return Ok(0);
        }
        let (next, inserted, removed) = self.apply(&guard, mutations)?;
        if inserted.is_empty() && removed.is_empty() {
            return Ok(0);
        }
        Self::persist_and_publish(
            &mut guard,
            next,
            inserted,
            removed,
            &self.database,
            &self.latest,
        )?;
        Ok(count)
    }

    fn commit(&self, mutations: Vec<EventStateMutation>) -> Result<(), EventCacheError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| EventCacheError::Refused("state lock poisoned".to_owned()))?;
        let (next, inserted, removed) = self.apply(&guard, mutations)?;
        if inserted.is_empty() && removed.is_empty() {
            return Ok(());
        }
        Self::persist_and_publish(
            &mut guard,
            next,
            inserted,
            removed,
            &self.database,
            &self.latest,
        )
    }

    fn event(&self, id: EventId) -> Result<Option<RelayEvent>, EventCacheError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| EventCacheError::Refused("state lock poisoned".to_owned()))?;
        Ok(guard
            .events
            .iter()
            .find_map(|((eid, _), ev)| (*eid == id).then(|| ev.clone())))
    }

    fn len(&self) -> Result<usize, EventCacheError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| EventCacheError::Refused("state lock poisoned".to_owned()))?;
        Ok(guard.events.len())
    }
}

impl QuerySource for RedbEventCache {
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

#[cfg(test)]
mod tests {
    use super::*;
    use fava_event_cache::EventCache;
    use fava_relay::RelayAccess;
    use nostr::event::{EventBuilder, FinalizeEvent, Kind};
    use nostr::key::Keys;
    use nostr::types::{RelayUrl, Timestamp};
    use std::num::NonZeroUsize;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(1);

    fn test_db_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "fava-event-cache-persistent-test-{}-{}.redb",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
        ))
    }

    fn make_relay_event(keys: &Keys, content: &str) -> RelayEvent {
        let event = EventBuilder::new(Kind::TextNote, content)
            .custom_created_at(Timestamp::now())
            .finalize(keys)
            .expect("signed event");
        let session = RelaySessionKey {
            relay: RelayUrl::parse("ws://127.0.0.1:7777").expect("valid url"),
            access: RelayAccess::Public,
        };
        RelayEvent::new(event, session, Timestamp::now())
    }

    #[test]
    fn empty_cache_is_empty_on_open() {
        let path = test_db_path();
        let cache = RedbEventCache::open(&path).expect("open");
        assert_eq!(cache.len().expect("len"), 0);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn events_survive_reopen() {
        let path = test_db_path();
        let keys = Keys::generate();
        let relay_event = make_relay_event(&keys, "persistent test event");
        let event_id = relay_event.event().id;

        {
            let cache = RedbEventCache::open_bounded(&path, NonZeroUsize::new(100).unwrap())
                .expect("open first");
            cache
                .admit(relay_event.clone(), Timestamp::now())
                .expect("admit");
            assert_eq!(cache.len().expect("len"), 1);
        }

        // Reopen — event must survive.
        let cache2 = RedbEventCache::open(&path).expect("reopen");
        assert_eq!(cache2.len().expect("len after reopen"), 1);
        assert!(
            cache2.event(event_id).expect("event lookup").is_some(),
            "event must be present after reopen"
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn retracted_event_does_not_survive_reopen() {
        let path = test_db_path();
        let keys = Keys::generate();
        let relay_event = make_relay_event(&keys, "retract test");
        let event_id = relay_event.event().id;
        let session = relay_event.occurrence().session.clone();

        {
            let cache = RedbEventCache::open(&path).expect("open");
            cache
                .admit(relay_event.clone(), Timestamp::now())
                .expect("admit");
            cache
                .commit(vec![EventStateMutation::Retract {
                    event_id,
                    session,
                    cause: RetractionCause::Evicted,
                }])
                .expect("retract");
            assert_eq!(cache.len().expect("len"), 0);
        }

        let cache2 = RedbEventCache::open(&path).expect("reopen after retract");
        assert_eq!(cache2.len().expect("len after reopen"), 0);
        let _ = std::fs::remove_file(path);
    }
}
