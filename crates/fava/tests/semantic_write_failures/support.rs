use std::sync::Arc;
use std::time::Duration;

use fava::{Event, Fava, Kind, ReplaceableEventEdit, ReplaceableEventMaterializer, Write};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_state::{CacheMutation, CachedEvent};
use fava_write::{WriteIntent, WriteRouting};
use fava_write_store::WriteStore;
use nostr::key::Keys;

use super::ControlledMaterializer;
use super::support::{
    BlockingSigner, NoopTransport, RecordingPublisher, relay_evidence, relay_url,
};

pub(super) fn edit_intent(author: fava::PublicKey, kind: Kind) -> WriteIntent {
    let edit = edit(kind);
    WriteIntent::edit_as(
        edit,
        author,
        WriteRouting::explicit([relay_url()]).expect("neutral explicit route validates"),
    )
    .unwrap()
}

pub(super) fn edit(kind: Kind) -> ReplaceableEventEdit {
    ReplaceableEventEdit::new(kind, None, vec![1]).expect("bounded edit")
}

pub(super) fn publish_edit(fava: &Fava, author: fava::PublicKey, kind: Kind) -> Write {
    fava.by(author)
        .to([relay_url()])
        .expect("public explicit route validates")
        .publish(edit(kind))
        .expect("semantic edit accepts")
}

pub(super) fn assembly<W>(
    keys: &Keys,
    cache: Arc<MemoryEventCache>,
    store: Arc<W>,
    materializers: Vec<Arc<ControlledMaterializer>>,
) -> Fava
where
    W: WriteStore + 'static,
{
    Fava::builder()
        .event_cache(cache)
        .write_store(store)
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .signer(Arc::new(BlockingSigner::new(keys.public_key())))
        .publisher(Arc::new(RecordingPublisher::default()))
        .delivery_policy(Arc::new(
            fava_delivery_standard::StandardDeliveryPolicy::default(),
        ))
        .materializers(
            materializers
                .into_iter()
                .map(|value| value as Arc<dyn ReplaceableEventMaterializer>),
        )
        .build()
        .unwrap()
}

pub(super) async fn wait_failure(fava: &Fava, receipt_id: fava::ReceiptId) -> fava::Receipt {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let receipt = fava.receipt(receipt_id).unwrap().unwrap();
            if receipt
                .current
                .publication
                .materialization_failure
                .is_some()
            {
                return receipt;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("failure becomes public")
}

pub(super) async fn wait_public_failure(observation: &mut fava::Observation) -> String {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let snapshot = observation.changed().await.expect("query stays open");
            if let Some(failure) = snapshot.events.iter().find_map(|record| {
                record
                    .publication
                    .as_ref()
                    .and_then(|evidence| evidence.materialization_failure.clone())
            }) {
                return failure;
            }
        }
    })
    .await
    .expect("failure becomes visible through ordinary query")
}

pub(super) fn save_source(cache: &MemoryEventCache, source: Event) {
    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            source,
            relay_evidence(),
        ))])
        .expect("source commits");
}
