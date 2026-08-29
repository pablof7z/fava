use std::sync::Arc;
use std::time::Duration;

use fava::{
    EventValue, Kind, MaterializationId, Receipt, ReceiptOutcome, ReplaceableEventEdit,
    ReplaceableEventMaterializer, Timestamp, WriteRouting, all_terminal,
};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_state::EventStateMutation;
use fava_write::WriteIntent;
use fava_write_store::{AcceptedWrite, WriteStore};
use nostr::key::Keys;

use super::failure_support::publish_edit;
use super::faults::FaultingWriteStore;
use super::support::{
    CountingSigner, RecordingPublisher, TestMaterializer, UnavailableSigner, WindowSigner,
    publication_builder, relay_event, relay_occurrence, signed_source, wait_for_materialization,
};

fn compose_direct(
    store: &FaultingWriteStore,
    materializer: &TestMaterializer,
    author: fava::PublicKey,
    current: &Receipt,
    change: u8,
) -> AcceptedWrite {
    let edit = ReplaceableEventEdit::new(Kind::Custom(10_004), None, vec![change]).unwrap();
    let created_at = Timestamp::from(
        current
            .current
            .event
            .created_at()
            .as_secs()
            .checked_add(1)
            .unwrap(),
    );
    let event = materializer
        .materialize(&edit, author, Some(&current.current.event), created_at)
        .unwrap();
    let reservation = store
        .reserve_active(&edit, author)
        .expect("same coordinate reserves bounded composition");
    store
        .accept_reserved_materialized_edit(
            reservation,
            WriteIntent::edit_as(
                edit,
                author,
                WriteRouting::explicit([super::support::relay_url()]).unwrap(),
            )
            .unwrap(),
            event,
            Some(&current.current.event),
            None,
        )
        .expect("direct composition commits outside the existing runner")
}

#[tokio::test(flavor = "current_thread")]
async fn transient_initial_read_resumes_semantic_runner_without_restart() {
    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(FaultingWriteStore::new());
    let signer = Arc::new(UnavailableSigner::new(keys.public_key()));
    let materializer = Arc::new(TestMaterializer::new(Kind::Custom(10_004)));
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::new(RecordingPublisher::default()),
    )
    .application_materializer(materializer)
    .build()
    .expect("faulting-store assembly");
    store.fail_receipt_reads(1);
    let accepted = publish_edit(&fava, keys.public_key(), Kind::Custom(10_004));

    tokio::time::timeout(Duration::from_secs(1), async {
        while store.remaining_receipt_read_failures() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("runner consumes the injected initial read failure");

    let source = signed_source(&keys, Kind::Custom(10_004), 10, "new source", &[]);
    cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            source,
            relay_occurrence(),
        ))])
        .expect("source change commits");
    let rematerialized = wait_for_materialization(&fava, accepted.receipt_id(), 2).await;
    assert_eq!(rematerialized.receipt_id, accepted.receipt_id());
}

#[tokio::test(flavor = "current_thread")]
async fn durable_sequence_refresh_failure_fences_local_generation_and_stale_replay() {
    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(FaultingWriteStore::new());
    let signer = Arc::new(WindowSigner::new(keys.clone()));
    let materializer = Arc::new(TestMaterializer::new(Kind::Custom(10_004)));
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::new(RecordingPublisher::default()),
    )
    .application_materializer(Arc::clone(&materializer))
    .build()
    .expect("faulting-store assembly");
    let first = publish_edit(&fava, keys.public_key(), Kind::Custom(10_004));
    let generation_one = first.receipt().expect("first generation remains live");

    store.fail_materialized_reads(true);
    let mut custody_reads = store.materialized_read_barrier();
    let composed = compose_direct(&store, &materializer, keys.public_key(), &generation_one, 2);
    assert_eq!(
        composed.current.publication.materialization_id,
        MaterializationId::try_from(2).expect("nonzero materialization identity")
    );
    let third = compose_direct(
        &store,
        &materializer,
        keys.public_key(),
        &store.receipt(composed.receipt_id).unwrap().unwrap(),
        3,
    );
    assert_eq!(
        third.current.publication.materialization_id,
        MaterializationId::try_from(3).expect("nonzero materialization identity")
    );
    tokio::time::timeout(Duration::from_secs(1), async {
        while store.materialized_read_failures() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("runner observes the injected durable custody failure");
    let source = signed_source(&keys, Kind::Custom(10_004), 20, "new source", &[]);
    cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            source,
            relay_occurrence(),
        ))])
        .expect("successor source commits while custody reads fail");
    let failures_before_late_source = *custody_reads.borrow_and_update();
    tokio::time::timeout(Duration::from_secs(1), async {
        while *custody_reads.borrow() <= failures_before_late_source {
            custody_reads.changed().await.unwrap();
        }
    })
    .await
    .expect("a post-source custody retry reaches the deterministic read barrier");
    assert_eq!(
        signer.calls().len(),
        0,
        "local signing advanced without refreshing the durable edit sequence"
    );
    assert_eq!(
        fava.receipt(first.receipt_id())
            .unwrap()
            .unwrap()
            .current
            .publication
            .materialization_id,
        MaterializationId::try_from(3).expect("nonzero materialization identity"),
        "stale replay installed while durable sequence refresh was unavailable"
    );

    store.fail_materialized_reads(false);
    tokio::time::timeout(Duration::from_secs(1), async {
        while signer.calls().len() != 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("current and replayed generations reach their exact signer windows");
    let calls = signer.calls();
    let current = first.receipt().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[1], current.current.id());
    signer.release_one();
    let replayed = wait_for_materialization(&fava, first.receipt_id(), 4).await;
    assert_eq!(signer.calls(), calls);
    let EventValue::Unsigned(event) = replayed.current.event else {
        panic!("blocking signer keeps replay unsigned");
    };
    assert_eq!(event.content, "new source|edit|edit|edit");
}

#[tokio::test(flavor = "current_thread")]
async fn transient_signed_read_errors_do_not_strand_delivery_lane() {
    let keys = Keys::generate();
    let store = Arc::new(FaultingWriteStore::new());
    let signer = Arc::new(CountingSigner::new(keys.clone()));
    let publisher = Arc::new(RecordingPublisher::default());
    let fava = publication_builder(
        Arc::new(MemoryEventCache::default()),
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::clone(&publisher),
    )
    .application_materializer(Arc::new(TestMaterializer::new(Kind::Custom(10_004))))
    .build()
    .expect("faulting-store assembly");
    store.fail_receipt_reads_after_signature(4);
    let accepted = publish_edit(&fava, keys.public_key(), Kind::Custom(10_004));

    tokio::time::timeout(Duration::from_secs(1), async {
        while publisher.attempts().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("delivery lane resumes after transient reads");
    let terminal = accepted
        .settled(all_terminal())
        .await
        .expect("same receipt settles");
    assert_eq!(terminal.outcome, ReceiptOutcome::Complete);
    assert_eq!(terminal.receipt_id, accepted.receipt_id());
    assert_eq!(signer.calls(), 1);
    assert_eq!(publisher.attempts().len(), 1);
}
