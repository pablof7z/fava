//! Controlled account-switch races for reactive observation generations.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};

use fava::{EventValue, Fava, Kind, Query, RelayUrl, Timestamp};
use fava_event_cache::{EventCache, EventCacheError};
use fava_event_cache_memory::MemoryEventCache;
use fava_query::{OpenedQuerySource, QuerySource, QuerySourceError, SourceCoverage};
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::Authority;
use fava_state::{EventStateMutation, RelayEvent};
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{EventBuilder, EventId, FinalizeEvent};
use nostr::filter::Filter;
use nostr::key::{Keys, PublicKey};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn switch_during_source_open_never_publishes_the_stale_account() {
    let alice = Keys::generate();
    let bob = Keys::generate();
    let carol = Keys::generate();
    let (fava, cache) = assembly(&alice, &bob, &carol);
    select_accounts(&fava, &alice, &bob, &carol);
    let mut observation = fava
        .observe(Query::events().authors_current_account().cache_only())
        .await
        .expect("reactive observation opens");
    assert_eq!(content(&observation.current()), vec!["alice"]);

    fava.select_account(bob.public_key()).expect("Bob selects");
    cache.wait_until_blocked().await;
    fava.select_account(carol.public_key())
        .expect("Carol selects");
    cache.release();

    let snapshot = tokio::time::timeout(std::time::Duration::from_secs(1), observation.changed())
        .await
        .expect("successor snapshot arrives")
        .expect("observation remains open");
    assert_eq!(content(&snapshot), vec!["carol"]);
    assert!(
        snapshot
            .events
            .iter()
            .all(|record| record.event().author() == carol.public_key())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn synchronization_never_returns_the_prior_generation() {
    let alice = Keys::generate();
    let bob = Keys::generate();
    let carol = Keys::generate();
    let (fava, cache) = assembly(&alice, &bob, &carol);
    select_accounts(&fava, &alice, &bob, &carol);
    let mut observation = fava
        .observe(Query::events().authors_current_account().cache_only())
        .await
        .expect("reactive observation opens");

    fava.select_account(bob.public_key()).expect("Bob selects");
    cache.wait_until_blocked().await;
    let premature = observation
        .synchronize_current_account(std::time::Duration::from_millis(20))
        .await
        .expect("observation remains open");
    cache.release();
    assert!(
        premature.is_none(),
        "the prior Alice generation cannot satisfy Bob synchronization"
    );
    let bob_snapshot = observation
        .synchronize_current_account(std::time::Duration::from_secs(1))
        .await
        .expect("observation remains open")
        .expect("Bob generation arrives");
    assert_eq!(content(&bob_snapshot), vec!["bob"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_during_source_open_synchronously_retires_the_active_generation() {
    let alice = Keys::generate();
    let bob = Keys::generate();
    let carol = Keys::generate();
    let (fava, cache) = assembly(&alice, &bob, &carol);
    select_accounts(&fava, &alice, &bob, &carol);
    let mut observation = fava
        .observe(Query::events().authors_current_account().cache_only())
        .await
        .expect("reactive observation opens");
    let id = observation.id();

    fava.select_account(bob.public_key()).expect("Bob selects");
    cache.wait_until_blocked().await;
    observation.close();
    assert!(
        fava.diagnostics()
            .queries
            .iter()
            .all(|query| query.observation != id)
    );
    cache.release();
    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
    assert!(observation.changed().await.is_err());
    assert!(
        fava.diagnostics()
            .queries
            .iter()
            .all(|query| query.observation != id),
        "retired child cannot resurrect outer diagnostics"
    );
}

fn assembly(alice: &Keys, bob: &Keys, carol: &Keys) -> (Fava, Arc<BlockingCache>) {
    let inner = Arc::new(MemoryEventCache::default());
    for (index, (keys, value)) in [(alice, "alice"), (bob, "bob"), (carol, "carol")]
        .into_iter()
        .enumerate()
    {
        let event = EventBuilder::new(Kind::TextNote, value)
            .finalize(keys)
            .expect("event signs");
        inner
            .admit(
                RelayEvent::new(
                    event,
                    RelayUrl::parse("wss://cache.example").expect("relay URL"),
                    Authority::Unauthenticated,
                    Timestamp::from(index as u64 + 1),
                ),
                Timestamp::from(index as u64 + 1),
            )
            .expect("event admits");
    }
    let cache = Arc::new(BlockingCache::new(inner, bob.public_key()));
    let fava = Fava::builder()
        .event_cache(Arc::clone(&cache))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .build()
        .expect("cache-only assembly");
    (fava, cache)
}

fn select_accounts(fava: &Fava, alice: &Keys, bob: &Keys, carol: &Keys) {
    for key in [alice.public_key(), bob.public_key(), carol.public_key()] {
        fava.add_account(key).expect("account adds");
    }
    fava.select_account(alice.public_key())
        .expect("Alice selects");
}

fn content(snapshot: &fava::QuerySnapshot) -> Vec<&str> {
    snapshot
        .events
        .iter()
        .map(|record| match record.event() {
            EventValue::Unsigned(event) => event.content.as_str(),
            EventValue::Signed(event) => event.content.as_str(),
        })
        .collect()
}

struct BlockingCache {
    inner: Arc<MemoryEventCache>,
    blocked_author: PublicKey,
    armed: AtomicBool,
    entered: Arc<Barrier>,
    release: Arc<Barrier>,
}

impl BlockingCache {
    fn new(inner: Arc<MemoryEventCache>, blocked_author: PublicKey) -> Self {
        Self {
            inner,
            blocked_author,
            armed: AtomicBool::new(true),
            entered: Arc::new(Barrier::new(2)),
            release: Arc::new(Barrier::new(2)),
        }
    }

    async fn wait_until_blocked(&self) {
        let entered = Arc::clone(&self.entered);
        tokio::task::spawn_blocking(move || entered.wait())
            .await
            .expect("barrier task completes");
    }

    fn release(&self) {
        self.release.wait();
    }
}

impl QuerySource for BlockingCache {
    fn open(&self, query: &Query) -> Result<OpenedQuerySource, QuerySourceError> {
        let blocks =
            query.selection().authors.as_ref().is_some_and(|authors| {
                authors.len() == 1 && authors.contains(&self.blocked_author)
            });
        if blocks && self.armed.swap(false, Ordering::SeqCst) {
            self.entered.wait();
            self.release.wait();
        }
        self.inner.open(query)
    }
}

impl EventCache for BlockingCache {
    fn source_coverage(
        &self,
        session: &RelayUrl,
        filter: &Filter,
    ) -> Result<Option<SourceCoverage>, EventCacheError> {
        self.inner.source_coverage(session, filter)
    }

    fn retain_source_coverage(&self, coverage: SourceCoverage) -> Result<(), EventCacheError> {
        self.inner.retain_source_coverage(coverage)
    }

    fn transact(
        &self,
        decide: &dyn Fn(&[RelayEvent]) -> Vec<EventStateMutation>,
    ) -> Result<usize, EventCacheError> {
        self.inner.transact(decide)
    }

    fn commit(&self, mutations: Vec<EventStateMutation>) -> Result<(), EventCacheError> {
        self.inner.commit(mutations)
    }

    fn event(&self, id: EventId) -> Result<Option<RelayEvent>, EventCacheError> {
        self.inner.event(id)
    }

    fn len(&self) -> Result<usize, EventCacheError> {
        self.inner.len()
    }
}
