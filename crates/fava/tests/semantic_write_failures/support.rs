use std::sync::Arc;
use std::time::Duration;

use fava::{EditApplier, Event, EventEdit, Fava, Kind, Write};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_state::EventStateMutation;
use fava_write::{WriteIntent, WriteRouting};
use fava_write_store::WriteStore;
use nostr::key::Keys;

use super::ControlledApplier;
use super::support::{
    NoopTransport, RecordingPublisher, UnavailableSigner, relay_event, relay_occurrence, relay_url,
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

pub(super) fn edit(kind: Kind) -> EventEdit {
    EventEdit::new(kind, None, vec![1]).expect("bounded edit")
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
    appliers: Vec<Arc<ControlledApplier>>,
) -> Fava
where
    W: WriteStore + 'static,
{
    Fava::builder()
        .event_cache(cache)
        .write_store(store)
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .signer(Arc::new(UnavailableSigner::new(keys.public_key())))
        .publisher(Arc::new(RecordingPublisher::default()))
        .delivery_policy(Arc::new(
            fava_delivery_standard::StandardDeliveryPolicy::default(),
        ))
        .appliers(
            appliers
                .into_iter()
                .map(|value| value as Arc<dyn EditApplier>),
        )
        .build()
        .unwrap()
}

pub(super) async fn wait_failure(fava: &Fava, receipt_id: fava::ReceiptId) -> fava::Receipt {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let receipt = fava.receipt(receipt_id).unwrap().unwrap();
            if receipt.current.publication.revision_failure.is_some() {
                return receipt;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("failure becomes public")
}

pub(super) async fn wait_public_failure(observation: &mut fava::Observation) -> String {
    let snapshot = observation
        .wait_until(Duration::from_secs(1), |snapshot| {
            snapshot.events.iter().any(|record| {
                record
                    .publication()
                    .and_then(|evidence| evidence.revision_failure.as_ref())
                    .is_some()
            })
        })
        .await
        .expect("observation stays open")
        .expect("failure becomes visible through ordinary query");
    snapshot
        .events
        .iter()
        .find_map(|record| {
            record
                .publication()
                .and_then(|evidence| evidence.revision_failure.clone())
        })
        .expect("matching snapshot carries a failure")
}

pub(super) fn save_source(cache: &MemoryEventCache, source: Event) {
    cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            source,
            relay_occurrence(),
        ))])
        .expect("source commits");
}
