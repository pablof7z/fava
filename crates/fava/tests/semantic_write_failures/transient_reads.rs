use std::sync::Arc;
use std::time::Duration;

use fava::{
    EventValue, Kind, MaterializationId, ReceiptOutcome, ReplaceableEventEdit,
    ReplaceableEventMaterializer, Timestamp, WriteRouting, all,
};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_state::{CacheMutation, CachedEvent};
use fava_write::WriteIntent;
use fava_write_store::WriteStore;
use nostr::key::Keys;

use super::failure_support::publish_edit;
use super::faults::FaultingWriteStore;
use super::support::{
    BlockingSigner, CountingSigner, RecordingPublisher, TestMaterializer, publication_builder,
    relay_evidence, signed_source, wait_for_materialization, wait_for_signer,
};

#[tokio::test(flavor = "current_thread")]
async fn transient_initial_read_resumes_semantic_runner_without_restart() {
    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(FaultingWriteStore::new());
    let signer = Arc::new(BlockingSigner::new(keys.public_key()));
    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList));
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::new(RecordingPublisher::default()),
    )
    .materializer(materializer)
    .build()
    .expect("faulting-store assembly");
    store.fail_receipt_reads(1);
    let accepted = publish_edit(&fava, keys.public_key(), Kind::ContactList);

    wait_for_signer(&signer, 1).await;
    let source = signed_source(&keys, Kind::ContactList, 10, "new source", &[]);
    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            source,
            relay_evidence(),
        ))])
        .expect("source change commits");
    let rematerialized = wait_for_materialization(&fava, accepted.receipt_id(), 2).await;
    wait_for_signer(&signer, 2).await;
    assert_eq!(rematerialized.receipt_id, accepted.receipt_id());
}

#[tokio::test(flavor = "current_thread")]
async fn durable_sequence_refresh_failure_fences_local_generation_and_stale_replay() {
    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(FaultingWriteStore::new());
    let signer = Arc::new(BlockingSigner::new(keys.public_key()));
    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList));
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::new(RecordingPublisher::default()),
    )
    .materializer(Arc::clone(&materializer))
    .build()
    .expect("faulting-store assembly");
    let first = publish_edit(&fava, keys.public_key(), Kind::ContactList);
    wait_for_signer(&signer, 1).await;
    let generation_one = first.receipt().expect("first generation remains live");

    let second_edit = ReplaceableEventEdit::new(Kind::ContactList, None, vec![2]).unwrap();
    let created_at = Timestamp::from(
        generation_one
            .current
            .event
            .created_at()
            .as_secs()
            .checked_add(1)
            .unwrap(),
    );
    let second_event = materializer
        .materialize(
            &second_edit,
            keys.public_key(),
            Some(&generation_one.current.event),
            created_at,
        )
        .unwrap();
    store.fail_materialized_reads(true);
    let reservation = store
        .reserve_active(&second_edit, keys.public_key())
        .expect("same coordinate reserves composition");
    let composed = store
        .accept_reserved_materialized_edit(
            reservation,
            WriteIntent::edit_as(
                second_edit,
                keys.public_key(),
                WriteRouting::explicit([super::support::relay_url()]).unwrap(),
            )
            .unwrap(),
            second_event,
            Some(&generation_one.current.event),
            None,
        )
        .expect("second edit commits durably outside the existing runner");
    assert_eq!(
        composed.current.publication.materialization_id,
        MaterializationId::from_u64(2)
    );

    tokio::time::timeout(Duration::from_secs(1), async {
        while store.materialized_read_failures() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("runner observes the injected durable custody failure");
    let third_edit = ReplaceableEventEdit::new(Kind::ContactList, None, vec![3]).unwrap();
    let third_event = materializer
        .materialize(
            &third_edit,
            keys.public_key(),
            Some(&composed.current.event),
            Timestamp::from(
                composed
                    .current
                    .event
                    .created_at()
                    .as_secs()
                    .checked_add(1)
                    .unwrap(),
            ),
        )
        .unwrap();
    let third_reservation = store
        .reserve_active(&third_edit, keys.public_key())
        .expect("same coordinate reserves another bounded composition");
    let third = store
        .accept_reserved_materialized_edit(
            third_reservation,
            WriteIntent::edit_as(
                third_edit,
                keys.public_key(),
                WriteRouting::explicit([super::support::relay_url()]).unwrap(),
            )
            .unwrap(),
            third_event,
            Some(&composed.current.event),
            None,
        )
        .expect("newer durable composition supersedes the failed refresh target");
    assert_eq!(
        third.current.publication.materialization_id,
        MaterializationId::from_u64(3)
    );
    let source = signed_source(&keys, Kind::ContactList, 20, "new source", &[]);
    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            source,
            relay_evidence(),
        ))])
        .expect("successor source commits while custody reads fail");
    tokio::time::sleep(Duration::from_millis(25)).await;
    assert_eq!(
        signer.calls(),
        1,
        "local signing advanced without refreshing the durable edit sequence"
    );
    assert_eq!(
        fava.receipt(first.receipt_id())
            .unwrap()
            .unwrap()
            .current
            .publication
            .materialization_id,
        MaterializationId::from_u64(3),
        "stale replay installed while durable sequence refresh was unavailable"
    );

    store.fail_materialized_reads(false);
    let replayed = wait_for_materialization(&fava, first.receipt_id(), 4).await;
    wait_for_signer(&signer, 3).await;
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
    .materializer(Arc::new(TestMaterializer::new(Kind::ContactList)))
    .build()
    .expect("faulting-store assembly");
    store.fail_receipt_reads_after_signature(4);
    let accepted = publish_edit(&fava, keys.public_key(), Kind::ContactList);

    tokio::time::timeout(Duration::from_secs(1), async {
        while publisher.attempts().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("delivery lane resumes after transient reads");
    let terminal = accepted.settled(all()).await.expect("same receipt settles");
    assert_eq!(terminal.outcome, ReceiptOutcome::Complete);
    assert_eq!(terminal.receipt_id, accepted.receipt_id());
    assert_eq!(signer.calls(), 1);
    assert_eq!(publisher.attempts().len(), 1);
}
