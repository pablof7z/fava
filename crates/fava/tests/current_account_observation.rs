//! Public evidence that one observation follows the session current account.

use std::sync::Arc;
use std::time::Duration;

use fava::{EventValue, Fava, Kind, Query, RelayUrl, Timestamp};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_state::RelayEvent;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{Event, EventBuilder, FinalizeEvent};
use nostr::key::Keys;

#[tokio::test(flavor = "current_thread")]
async fn one_observation_reroots_when_current_account_changes() {
    let alice = Keys::generate();
    let bob = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    admit(&cache, note(&alice, "alice"), 1);
    admit(&cache, note(&bob, "bob"), 2);
    let fava = Fava::builder()
        .event_cache(Arc::clone(&cache))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .build()
        .expect("cache-only assembly");
    fava.add_account(alice.public_key()).expect("Alice adds");
    fava.add_account(bob.public_key()).expect("Bob adds");
    fava.select_account(alice.public_key())
        .expect("Alice selects");

    let mut observation = fava
        .observe(
            Query::events()
                .authors_current_account()
                .kinds([Kind::TextNote])
                .expect("one kind")
                .cache_only(),
        )
        .await
        .expect("reactive observation opens");
    let id = observation.id();
    assert_contents(&observation.current(), &["alice"]);

    fava.select_account(bob.public_key()).expect("Bob selects");
    let bob_snapshot = changed(&mut observation).await;
    assert_eq!(observation.id(), id);
    assert_contents(&bob_snapshot, &["bob"]);

    fava.clear_current_account().expect("selection clears");
    let empty = changed(&mut observation).await;
    assert_eq!(observation.id(), id);
    assert!(empty.events.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn rapid_switch_and_old_source_change_preserve_latest_account() {
    let alice = Keys::generate();
    let bob = Keys::generate();
    let carol = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    admit(&cache, note(&alice, "alice-old"), 1);
    admit(&cache, note(&bob, "bob"), 2);
    admit(&cache, note(&carol, "carol"), 3);
    let fava = Fava::builder()
        .event_cache(Arc::clone(&cache))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .build()
        .expect("cache-only assembly");
    fava.add_account(alice.public_key()).expect("Alice adds");
    fava.add_account(bob.public_key()).expect("Bob adds");
    fava.add_account(carol.public_key()).expect("Carol adds");
    fava.select_account(alice.public_key())
        .expect("Alice selects");
    let mut observation = fava
        .observe(Query::events().authors_current_account().cache_only())
        .await
        .expect("reactive observation opens");

    admit(&cache, note(&alice, "alice-late"), 4);
    fava.select_account(bob.public_key()).expect("Bob selects");
    let bob_snapshot = changed(&mut observation).await;
    assert_contents(&bob_snapshot, &["bob"]);

    fava.select_account(alice.public_key())
        .expect("Alice selects");
    fava.select_account(bob.public_key()).expect("Bob selects");
    fava.select_account(carol.public_key())
        .expect("Carol selects last");
    let carol_snapshot = changed(&mut observation).await;
    assert_contents(&carol_snapshot, &["carol"]);
}

async fn changed(observation: &mut fava::Observation) -> Arc<fava::QuerySnapshot> {
    tokio::time::timeout(Duration::from_secs(1), observation.changed())
        .await
        .expect("account switch produces a snapshot")
        .expect("observation stays open")
}

fn note(keys: &Keys, content: &str) -> Event {
    EventBuilder::new(Kind::TextNote, content)
        .finalize(keys)
        .expect("event signs")
}

fn admit(cache: &MemoryEventCache, event: Event, observed_at: u64) {
    let session = RelaySessionKey {
        relay: RelayUrl::parse("wss://cache.example").expect("relay URL"),
        access: RelayAccess::Public,
    };
    cache
        .admit(
            RelayEvent::new(event, session, Timestamp::from(observed_at)),
            Timestamp::from(observed_at),
        )
        .expect("event admits");
}

fn assert_contents(snapshot: &fava::QuerySnapshot, expected: &[&str]) {
    let contents: Vec<_> = snapshot
        .events
        .iter()
        .map(|record| match record.event() {
            EventValue::Unsigned(event) => event.content.as_str(),
            EventValue::Signed(event) => event.content.as_str(),
        })
        .collect();
    assert_eq!(contents, expected);
}
