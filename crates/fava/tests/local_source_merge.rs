//! Public-facade acceptance evidence for the first local-source vertical slice.

use std::sync::Arc;
use std::time::Duration;

use fava::{Fava, Query};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_state::{
    CacheMutation, CachedEvent, RelayAccess, RelayEvidence, RelaySessionKey, RelayUrl, Timestamp,
};
use fava_write::EventValue;
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{
    Event, EventBuilder, FinalizeEvent, FinalizeUnsignedEvent, Kind, UnsignedEvent,
};
use nostr::key::Keys;
use tokio::time::timeout;

fn assembly() -> (Fava, Arc<MemoryEventCache>, Arc<MemoryWriteStore>) {
    let cache = Arc::new(MemoryEventCache::default());
    let writes = Arc::new(MemoryWriteStore::default());
    let fava = Fava::builder()
        .event_cache(Arc::clone(&cache))
        .write_store(Arc::clone(&writes))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .build()
        .expect("complete local assembly");
    (fava, cache, writes)
}

fn signed_event(keys: &Keys, kind: Kind, created_at: u64, content: &str) -> Event {
    EventBuilder::new(kind, content)
        .custom_created_at(Timestamp::from(created_at))
        .finalize(keys)
        .expect("test event signs")
}

fn unsigned_event(
    keys: &Keys,
    kind: Kind,
    created_at: u64,
    content: impl Into<String>,
) -> UnsignedEvent {
    let mut event = EventBuilder::new(kind, content)
        .custom_created_at(Timestamp::from(created_at))
        .finalize_unsigned(keys.public_key());
    event.ensure_id();
    event
}

fn evidence(relay: &str, observed_at: u64) -> RelayEvidence {
    RelayEvidence::one(
        RelaySessionKey::new(
            RelayUrl::parse(relay).expect("test relay url"),
            RelayAccess::public(),
        ),
        Timestamp::from(observed_at),
    )
}

async fn next_snapshot(feed: &mut fava_observe::Observation) -> Arc<fava::QuerySnapshot> {
    timeout(Duration::from_secs(1), feed.changed())
        .await
        .expect("observation update arrives within bound")
        .expect("observation remains open")
}

#[tokio::test(flavor = "current_thread")]
async fn accepted_local_event_is_visible_without_cache_pollution() {
    let (fava, cache, writes) = assembly();
    let keys = Keys::generate();
    let unsigned = unsigned_event(&keys, Kind::TextNote, 10, "local");
    let id = unsigned.id.expect("builder computes id");
    let mut feed = fava
        .observe(Query::events().cache_only())
        .await
        .expect("query opens from local sources");
    assert!(feed.current().events.is_empty());

    let accepted = writes
        .accept_materialized(EventValue::Unsigned(unsigned))
        .expect("write store accepts finalized local event");
    let visible = next_snapshot(&mut feed).await;

    assert_eq!(visible.events.len(), 1);
    assert_eq!(visible.events[0].id(), id);
    assert_eq!(
        visible.events[0]
            .publication
            .as_ref()
            .map(|publication| publication.receipt_id),
        Some(accepted.receipt_id)
    );
    assert!(cache.is_empty().expect("cache remains readable"));

    assert!(
        writes
            .cancel(accepted.receipt_id)
            .expect("local cancellation commits")
    );
    assert!(next_snapshot(&mut feed).await.events.is_empty());
    assert!(cache.is_empty().expect("cache remains unchanged"));
}

#[tokio::test(flavor = "current_thread")]
async fn cancelling_local_replacement_reveals_cached_predecessor() {
    let (fava, cache, writes) = assembly();
    let keys = Keys::generate();
    let predecessor = signed_event(&keys, Kind::ContactList, 10, "cached");
    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            predecessor.clone(),
            evidence("wss://relay.example", 1),
        ))])
        .expect("cache mutation commits");
    let successor = unsigned_event(&keys, Kind::ContactList, 20, "local");
    let successor_id = successor.id.expect("builder computes id");
    let accepted = writes
        .accept_materialized(EventValue::Unsigned(successor))
        .expect("local successor accepts");
    let query = Query::events().kind(Kind::ContactList).cache_only();
    let mut feed = fava.observe(query).await.expect("query opens");

    assert_eq!(feed.current().events.len(), 1);
    assert_eq!(feed.current().events[0].id(), successor_id);

    writes
        .cancel(accepted.receipt_id)
        .expect("local cancellation commits");
    let revealed = next_snapshot(&mut feed).await;
    assert_eq!(revealed.events.len(), 1);
    assert_eq!(revealed.events[0].id(), predecessor.id);
    assert!(
        cache
            .event(predecessor.id)
            .expect("cache readable")
            .is_some()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn relay_echo_enriches_one_record_without_erasing_receipt() {
    let (fava, cache, writes) = assembly();
    let keys = Keys::generate();
    let event = signed_event(&keys, Kind::TextNote, 10, "echo");
    let accepted = writes
        .accept_materialized(EventValue::Signed(event.clone()))
        .expect("signed local event accepts");
    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            event.clone(),
            evidence("wss://relay-a.example", 1),
        ))])
        .expect("first relay evidence commits");
    let mut feed = fava
        .observe(Query::events().cache_only())
        .await
        .expect("query opens");
    assert_eq!(feed.current().events.len(), 1);
    assert_eq!(feed.current().events[0].relay_evidence.len(), 1);

    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            event,
            evidence("wss://relay-b.example", 2),
        ))])
        .expect("second relay evidence merges");
    let enriched = next_snapshot(&mut feed).await;

    assert_eq!(enriched.events.len(), 1);
    assert_eq!(enriched.events[0].relay_evidence.len(), 2);
    assert_eq!(
        enriched.events[0]
            .publication
            .as_ref()
            .map(|publication| publication.receipt_id),
        Some(accepted.receipt_id)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn acquisition_only_and_provenance_constraint_stay_distinct() {
    let (fava, cache, _writes) = assembly();
    let keys = Keys::generate();
    let event = signed_event(&keys, Kind::TextNote, 10, "authority");
    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            event.clone(),
            evidence("wss://other.example", 1),
        ))])
        .expect("other relay evidence commits");
    let asked = RelayUrl::parse("wss://asked.example").expect("relay url");
    let from = Query::events()
        .from_relays([asked.clone()])
        .expect("non-empty relay set")
        .cache_only();
    let only = Query::events()
        .only_from_relays([asked])
        .expect("non-empty relay set")
        .cache_only();
    let from_feed = fava
        .observe(from)
        .await
        .expect("acquisition-only query opens");
    let mut only_feed = fava.observe(only).await.expect("authority query opens");

    assert_eq!(from_feed.current().events.len(), 1);
    assert!(only_feed.current().events.is_empty());

    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            event,
            evidence("wss://asked.example", 2),
        ))])
        .expect("qualifying relay evidence merges");
    assert_eq!(next_snapshot(&mut only_feed).await.events.len(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn slow_consumer_receives_exact_latest_state_with_bounded_delivery() {
    let (fava, _cache, writes) = assembly();
    let keys = Keys::generate();
    let mut feed = fava
        .observe(Query::events().cache_only())
        .await
        .expect("query opens");

    for index in 1..=3 {
        let event = unsigned_event(&keys, Kind::TextNote, index, format!("event {index}"));
        writes
            .accept_materialized(EventValue::Unsigned(event))
            .expect("local event accepts");
    }

    let latest = next_snapshot(&mut feed).await;
    assert_eq!(latest.events.len(), 3);
    assert_eq!(latest.events[0].created_at(), Timestamp::from(3));
}
