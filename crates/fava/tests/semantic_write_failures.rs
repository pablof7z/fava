//! Public semantic-write failure isolation and attribution evidence.

use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::time::Duration;

use fava::{
    Event, EventBuilder, EventCoordinate, EventValue, Fava, Kind, MaterializationId,
    ReplaceableEventEdit, ReplaceableEventMaterializer, Timestamp, UnsignedEvent, WriteIntent,
    WriteIntentError, WritePayload, WriteRouting,
};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_state::{CacheMutation, CachedEvent};
use fava_write_store::{WriteStore, destination_evidence_capacity};
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::FinalizeEvent;
use nostr::key::Keys;

#[path = "support/semantic_write.rs"]
mod support;

use support::{
    BlockingSigner, EDIT_FORMAT, NoopTransport, RecordingPublisher, relay_evidence, relay_url,
    signed_source,
};

const VALID: u8 = 0;
const ERROR: u8 = 1;
const PANIC: u8 = 2;
const WRONG_ACTOR: u8 = 3;
const OVERSIZE: u8 = 4;

struct ControlledMaterializer {
    kind: Kind,
    mode: AtomicU8,
    calls: AtomicU64,
}

impl ControlledMaterializer {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            mode: AtomicU8::new(VALID),
            calls: AtomicU64::new(0),
        }
    }

    fn set(&self, mode: u8) {
        self.mode.store(mode, Ordering::SeqCst);
    }

    fn calls(&self) -> u64 {
        self.calls.load(Ordering::SeqCst)
    }
}

impl ReplaceableEventMaterializer for ControlledMaterializer {
    fn kind(&self) -> Kind {
        self.kind
    }

    fn supports(&self, edit: &ReplaceableEventEdit) -> bool {
        edit.format() == EDIT_FORMAT
    }

    fn materialize(
        &self,
        edit: &ReplaceableEventEdit,
        source: Option<&Event>,
        created_at: Timestamp,
    ) -> Result<UnsignedEvent, WriteIntentError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.mode.load(Ordering::SeqCst) {
            ERROR => return Err(WriteIntentError::InvalidEvent("x".repeat(8_192))),
            PANIC => panic!("hostile materializer panic with unbounded details"),
            _ => {}
        }
        let actor = if self.mode.load(Ordering::SeqCst) == WRONG_ACTOR {
            Keys::generate().public_key()
        } else {
            edit.actor()
        };
        let content = if self.mode.load(Ordering::SeqCst) == OVERSIZE {
            "x".repeat(140_000)
        } else {
            source.map_or_else(
                || "initial".to_owned(),
                |event| format!("{}|edit", event.content),
            )
        };
        EventBuilder::new(actor, self.kind)
            .created_at(created_at)
            .content(content)
            .build()
            .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))
    }
}

fn edit_intent(actor: fava::PublicKey, kind: Kind) -> WriteIntent {
    let edit = ReplaceableEventEdit::new(
        actor,
        EventCoordinate::Replaceable {
            author: actor,
            kind,
            identifier: None,
        },
        EDIT_FORMAT,
        vec![1],
        vec![2],
    )
    .unwrap();
    WriteIntent::edit(edit, WriteRouting::Explicit(BTreeSet::from([relay_url()]))).unwrap()
}

fn assembly(
    keys: &Keys,
    cache: Arc<MemoryEventCache>,
    store: Arc<MemoryWriteStore>,
    materializers: Vec<Arc<ControlledMaterializer>>,
) -> Fava {
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

async fn wait_failure(fava: &Fava, receipt_id: fava::ReceiptId) -> fava::Receipt {
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

fn save_source(cache: &MemoryEventCache, source: Event) {
    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            source,
            relay_evidence(),
        ))])
        .expect("source commits");
}

#[tokio::test(flavor = "current_thread")]
async fn materializer_error_is_bounded_and_preserves_current() {
    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(MemoryWriteStore::default());
    let materializer = Arc::new(ControlledMaterializer::new(Kind::ContactList));
    let fava = assembly(
        &keys,
        Arc::clone(&cache),
        store,
        vec![Arc::clone(&materializer)],
    );
    let accepted = fava
        .publish(edit_intent(keys.public_key(), Kind::ContactList))
        .unwrap();
    materializer.set(ERROR);
    save_source(
        &cache,
        signed_source(&keys, Kind::ContactList, 10, "source", &[]),
    );

    let failed = wait_failure(&fava, accepted.receipt_id).await;
    assert_eq!(failed.current.id(), accepted.current.id());
    let evidence = failed.current.publication.materialization_failure.unwrap();
    assert!(evidence.len() <= 4_096);
    assert!(evidence.contains("materialization 1"));
}

#[tokio::test(flavor = "current_thread")]
async fn materializer_panic_is_scoped_and_attributed() {
    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let materializer = Arc::new(ControlledMaterializer::new(Kind::ContactList));
    let fava = assembly(
        &keys,
        Arc::clone(&cache),
        Arc::new(MemoryWriteStore::default()),
        vec![Arc::clone(&materializer)],
    );
    let accepted = fava
        .publish(edit_intent(keys.public_key(), Kind::ContactList))
        .unwrap();
    materializer.set(PANIC);
    save_source(
        &cache,
        signed_source(&keys, Kind::ContactList, 10, "source", &[]),
    );

    let failed = wait_failure(&fava, accepted.receipt_id).await;
    assert_eq!(failed.current.id(), accepted.current.id());
    assert!(
        failed
            .current
            .publication
            .materialization_failure
            .unwrap()
            .contains("panicked")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn malformed_and_oversize_outputs_preserve_current() {
    for mode in [WRONG_ACTOR, OVERSIZE] {
        let keys = Keys::generate();
        let cache = Arc::new(MemoryEventCache::default());
        let materializer = Arc::new(ControlledMaterializer::new(Kind::ContactList));
        let fava = assembly(
            &keys,
            Arc::clone(&cache),
            Arc::new(MemoryWriteStore::default()),
            vec![Arc::clone(&materializer)],
        );
        let accepted = fava
            .publish(edit_intent(keys.public_key(), Kind::ContactList))
            .unwrap();
        materializer.set(mode);
        save_source(
            &cache,
            signed_source(&keys, Kind::ContactList, 10, "source", &[]),
        );
        let failed = wait_failure(&fava, accepted.receipt_id).await;
        assert_eq!(failed.current.id(), accepted.current.id());
    }
}

#[test]
fn timestamp_and_evidence_overflow_preserve_current() {
    assert_eq!(destination_evidence_capacity(), 256);
    let keys = Keys::generate();
    let store = MemoryWriteStore::bounded(NonZeroUsize::new(1).unwrap());
    let intent = edit_intent(keys.public_key(), Kind::ContactList);
    assert!(matches!(intent.payload(), WritePayload::Edit(_)));
    let current = EventBuilder::new(keys.public_key(), Kind::ContactList)
        .created_at(Timestamp::from(1))
        .content("current")
        .build()
        .unwrap();
    let accepted = store
        .accept_materialized_edit(intent, current, None)
        .unwrap();
    let before = store.receipt(accepted.receipt_id).unwrap().unwrap();
    let max_source = EventBuilder::new(keys.public_key(), Kind::ContactList)
        .created_at(Timestamp::from(u64::MAX))
        .content("max")
        .build()
        .unwrap()
        .finalize(&keys)
        .unwrap();
    assert!(
        store
            .record_materialization_failure(
                accepted.write_id,
                accepted.receipt_id,
                MaterializationId::from_u64(1),
                None,
                Some(&max_source),
                "materialization timestamp exhausted".to_owned(),
            )
            .is_ok()
    );
    let failed = store.receipt(accepted.receipt_id).unwrap().unwrap();
    assert_eq!(failed.current.id(), before.current.id());
    assert!(matches!(failed.current.event, EventValue::Unsigned(_)));
}

#[tokio::test(flavor = "current_thread")]
async fn recovery_retries_failed_source_once() {
    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(MemoryWriteStore::default());
    let materializer = Arc::new(ControlledMaterializer::new(Kind::ContactList));
    let first = assembly(
        &keys,
        Arc::clone(&cache),
        Arc::clone(&store),
        vec![Arc::clone(&materializer)],
    );
    let accepted = first
        .publish(edit_intent(keys.public_key(), Kind::ContactList))
        .unwrap();
    materializer.set(ERROR);
    save_source(
        &cache,
        signed_source(&keys, Kind::ContactList, 10, "source", &[]),
    );
    wait_failure(&first, accepted.receipt_id).await;
    materializer.set(VALID);

    let calls_before_recovery = materializer.calls();
    let recovered = assembly(
        &keys,
        Arc::clone(&cache),
        Arc::clone(&store),
        vec![Arc::clone(&materializer)],
    );
    let receipt = support::wait_for_materialization(&recovered, accepted.receipt_id, 2).await;
    assert!(
        receipt
            .current
            .publication
            .materialization_failure
            .is_none()
    );
    tokio::task::yield_now().await;
    assert_eq!(materializer.calls(), calls_before_recovery + 1);
    assert_eq!(
        store.recover_materialized_edits().unwrap()[0]
            .0
            .current
            .publication
            .materialization_id,
        MaterializationId::from_u64(2)
    );

    let calls_after_success = materializer.calls();
    let _second_recovery = assembly(&keys, cache, store, vec![Arc::clone(&materializer)]);
    tokio::task::yield_now().await;
    assert_eq!(materializer.calls(), calls_after_success);
}

#[tokio::test(flavor = "current_thread")]
async fn successful_retry_clears_failure_without_duplicate_effect() {
    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(MemoryWriteStore::default());
    let materializer = Arc::new(ControlledMaterializer::new(Kind::ContactList));
    let fava = assembly(
        &keys,
        Arc::clone(&cache),
        Arc::clone(&store),
        vec![Arc::clone(&materializer)],
    );
    let accepted = fava
        .publish(edit_intent(keys.public_key(), Kind::ContactList))
        .unwrap();
    materializer.set(ERROR);
    let failed_source = signed_source(&keys, Kind::ContactList, 10, "failed", &[]);
    save_source(&cache, failed_source);
    wait_failure(&fava, accepted.receipt_id).await;
    materializer.set(VALID);
    let changed = signed_source(&keys, Kind::ContactList, 20, "changed", &[]);
    save_source(&cache, changed);
    let receipt = support::wait_for_materialization(&fava, accepted.receipt_id, 2).await;
    assert!(
        receipt
            .current
            .publication
            .materialization_failure
            .is_none()
    );
    let id = receipt.current.id();
    tokio::task::yield_now().await;
    assert_eq!(
        fava.receipt(accepted.receipt_id)
            .unwrap()
            .unwrap()
            .current
            .id(),
        id
    );
}
