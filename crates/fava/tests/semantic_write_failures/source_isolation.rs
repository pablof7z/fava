use std::sync::Arc;

use fava::{EventValue, Fava, Kind, MaterializationId};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_state::EventStateMutation;
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::key::Keys;

use super::faults::{ClosingEventCache, FaultingWriteStore};
use super::support::{
    NoopTransport, RecordingPublisher, UnavailableSigner, relay_event, relay_occurrence,
    signed_source, wait_for_materialization,
};
use super::{ControlledMaterializer, publish_edit, wait_failure};

fn build<C, W>(
    keys: &Keys,
    cache: Arc<C>,
    store: Arc<W>,
    materializer: Arc<ControlledMaterializer>,
) -> Fava
where
    C: EventCache + 'static,
    W: WriteStore + 'static,
{
    Fava::builder()
        .event_cache(cache)
        .write_store(store)
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .signer(Arc::new(UnavailableSigner::new(keys.public_key())))
        .publisher(Arc::new(RecordingPublisher::default()))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .application_materializer(materializer)
        .build()
        .expect("controlled source assembly")
}

#[tokio::test(flavor = "current_thread")]
async fn cache_source_closure_keeps_write_store_source_live() {
    let keys = Keys::generate();
    let cache = Arc::new(ClosingEventCache::new());
    let store = Arc::new(MemoryWriteStore::default());
    let materializer = Arc::new(ControlledMaterializer::new(Kind::Custom(10_004)));
    let fava = build(&keys, Arc::clone(&cache), Arc::clone(&store), materializer);
    let accepted = publish_edit(&fava, keys.public_key(), Kind::Custom(10_004));

    cache.close_observations();
    let failure = wait_failure(&fava, accepted.receipt_id()).await;
    assert!(
        failure
            .current
            .publication
            .materialization_failure
            .as_deref()
            .is_some_and(|reason| reason.contains("event-cache"))
    );
    let source = signed_source(&keys, Kind::Custom(10_004), 10, "write-store source", &[]);
    store
        .accept_materialized(EventValue::Signed(source.clone()))
        .expect("independent signed local source commits");
    let rematerialized = wait_for_materialization(&fava, accepted.receipt_id(), 2).await;

    assert_eq!(
        rematerialized.current.publication.materialization_source,
        Some(source.id)
    );
    assert_eq!(
        rematerialized.current.publication.materialization_id,
        MaterializationId::try_from(2).expect("nonzero materialization identity")
    );
    assert!(
        rematerialized.current.publication.retired_materializations[0]
            .3
            .as_deref()
            .is_some_and(|reason| reason.contains("event-cache"))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn write_store_source_closure_keeps_cache_source_live() {
    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(FaultingWriteStore::new());
    let materializer = Arc::new(ControlledMaterializer::new(Kind::Custom(10_004)));
    let fava = build(&keys, Arc::clone(&cache), Arc::clone(&store), materializer);
    let accepted = publish_edit(&fava, keys.public_key(), Kind::Custom(10_004));

    store.close_observations();
    let failure = wait_failure(&fava, accepted.receipt_id()).await;
    assert!(
        failure
            .current
            .publication
            .materialization_failure
            .as_deref()
            .is_some_and(|reason| reason.contains("write-store"))
    );
    let source = signed_source(&keys, Kind::Custom(10_004), 10, "cache source", &[]);
    cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            source.clone(),
            relay_occurrence(),
        ))])
        .expect("independent cache source commits");
    let rematerialized = wait_for_materialization(&fava, accepted.receipt_id(), 2).await;

    assert_eq!(
        rematerialized.current.publication.materialization_source,
        Some(source.id)
    );
    assert!(
        rematerialized.current.publication.retired_materializations[0]
            .3
            .as_deref()
            .is_some_and(|reason| reason.contains("write-store"))
    );
}
