//! M3 observation task and latest-state boundedness evidence.

use std::sync::Arc;
use std::time::Duration;

use fava::{EventValue, Fava, Query};
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_state::Timestamp;
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{EventBuilder, FinalizeEvent, Kind};
use nostr::key::Keys;

fn assembly() -> (Fava, Arc<MemoryWriteStore>) {
    let writes = Arc::new(MemoryWriteStore::default());
    let fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::clone(&writes))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .build()
        .expect("local assembly");
    (fava, writes)
}

#[tokio::test(flavor = "current_thread")]
async fn one_thousand_idle_observations_share_the_current_runtime_thread() {
    let (fava, _writes) = assembly();
    let thread = std::thread::current().id();
    let mut observations = Vec::with_capacity(1_000);
    for _ in 0..1_000 {
        observations.push(
            fava.observe(Query::events().cache_only())
                .await
                .expect("observation opens"),
        );
        assert_eq!(std::thread::current().id(), thread);
    }
    assert_eq!(observations.len(), 1_000);
    assert!(
        observations
            .iter()
            .all(|observation| observation.current().events.is_empty())
    );
    drop(observations);
    tokio::task::yield_now().await;
    assert_eq!(std::thread::current().id(), thread);
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_pulls_and_large_burst_deliver_one_exact_latest_state() {
    let (fava, writes) = assembly();
    let mut observation = fava
        .observe(Query::events().cache_only())
        .await
        .expect("observation opens");
    for _ in 0..128 {
        assert!(
            tokio::time::timeout(Duration::ZERO, observation.changed())
                .await
                .is_err()
        );
    }
    let keys = Keys::generate();
    for index in 0..256 {
        let event = EventBuilder::new(Kind::TextNote, format!("event-{index}"))
            .custom_created_at(Timestamp::from(index + 1))
            .finalize(&keys)
            .expect("event signs");
        writes
            .accept_materialized(EventValue::Signed(event))
            .expect("event accepts");
    }

    let latest = tokio::time::timeout(Duration::from_secs(1), observation.changed())
        .await
        .expect("latest-state deadline")
        .expect("observation stays open");
    assert_eq!(latest.events.len(), 256);
    assert!(fava.diagnostics().coalesced_query_updates > 0);
}
