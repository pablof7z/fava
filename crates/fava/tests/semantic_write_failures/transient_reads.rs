use std::sync::Arc;
use std::time::Duration;

use fava::{Kind, ReceiptOutcome, all};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_state::EventStateMutation;
use nostr::key::Keys;

use super::failure_support::publish_edit;
use super::faults::FaultingWriteStore;
use super::support::{
    BlockingSigner, CountingSigner, RecordingPublisher, TestMaterializer, publication_builder,
    relay_event, relay_occurrence, signed_source, wait_for_materialization, wait_for_signer,
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
        .commit(vec![EventStateMutation::Upsert(relay_event(
            source,
            relay_occurrence(),
        ))])
        .expect("source change commits");
    let rematerialized = wait_for_materialization(&fava, accepted.receipt_id(), 2).await;
    wait_for_signer(&signer, 2).await;
    assert_eq!(rematerialized.receipt_id, accepted.receipt_id());
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
