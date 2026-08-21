//! Public semantic-write failure isolation and attribution evidence.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

use fava::{
    Event, EventBuilder, Kind, MaterializationId, ReplaceableEventEdit,
    ReplaceableEventMaterializer, Timestamp, UnsignedEvent, WriteIntentError,
};
use fava_event_cache_memory::MemoryEventCache;
use fava_write_store::{WriteStore, destination_evidence_capacity};
use fava_write_store_memory::MemoryWriteStore;
use nostr::key::Keys;

#[path = "semantic_write_failures/support.rs"]
mod failure_support;
#[path = "semantic_write_failures/faults.rs"]
mod faults;
#[path = "semantic_write_failures/reservation.rs"]
mod reservation;
#[path = "semantic_write_failures/source_isolation.rs"]
mod source_isolation;
#[path = "support/semantic_write.rs"]
#[allow(dead_code)]
mod support;
#[path = "semantic_write_failures/transient_reads.rs"]
mod transient_reads;

use failure_support::{assembly, edit_intent, save_source, wait_failure, wait_public_failure};
use support::signed_source;

const VALID: u8 = 0;
const ERROR: u8 = 1;
const PANIC: u8 = 2;
const WRONG_ACTOR: u8 = 3;
const OVERSIZE: u8 = 4;
const WRONG_TIMESTAMP: u8 = 5;
const WRONG_KIND: u8 = 6;

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
        edit.kind() == self.kind
    }

    fn materialize(
        &self,
        _edit: &ReplaceableEventEdit,
        author: fava::PublicKey,
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
            author
        };
        let kind = if self.mode.load(Ordering::SeqCst) == WRONG_KIND {
            Kind::MuteList
        } else {
            self.kind
        };
        let content = if self.mode.load(Ordering::SeqCst) == OVERSIZE {
            "x".repeat(140_000)
        } else {
            source.map_or_else(
                || "initial".to_owned(),
                |event| format!("{}|edit", event.content),
            )
        };
        let returned_at = if self.mode.load(Ordering::SeqCst) == WRONG_TIMESTAMP {
            Timestamp::from(created_at.as_secs().saturating_sub(1))
        } else {
            created_at
        };
        EventBuilder::new(actor, kind)
            .created_at(returned_at)
            .content(content)
            .build()
            .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))
    }
}

#[tokio::test(flavor = "current_thread")]
async fn wrong_injected_timestamp_refuses_first_and_preserves_successor_current() {
    let first_keys = Keys::generate();
    let first_store = Arc::new(MemoryWriteStore::default());
    let first_materializer = Arc::new(ControlledMaterializer::new(Kind::ContactList));
    first_materializer.set(WRONG_TIMESTAMP);
    let first = assembly(
        &first_keys,
        Arc::new(MemoryEventCache::default()),
        Arc::clone(&first_store),
        vec![Arc::clone(&first_materializer)],
    );
    assert!(
        first
            .publish(edit_intent(first_keys.public_key(), Kind::ContactList))
            .is_err()
    );
    assert_eq!(first_materializer.calls(), 1);
    assert!(first_store.is_empty().expect("first store remains empty"));

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
        .expect("valid first generation accepts");
    materializer.set(WRONG_TIMESTAMP);
    save_source(
        &cache,
        signed_source(&keys, Kind::ContactList, 10, "new source", &[]),
    );
    let failed = wait_failure(&fava, accepted.receipt_id).await;

    assert_eq!(failed.current.id(), accepted.current.id());
    assert_eq!(
        failed.current.publication.materialization_id,
        MaterializationId::from_u64(1)
    );
    assert!(
        failed
            .current
            .publication
            .materialization_failure
            .as_deref()
            .is_some_and(|reason| reason.contains("injected timestamp"))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn wrong_author_or_kind_refuses_before_custody() {
    for mode in [WRONG_ACTOR, WRONG_KIND] {
        let keys = Keys::generate();
        let store = Arc::new(MemoryWriteStore::default());
        let materializer = Arc::new(ControlledMaterializer::new(Kind::ContactList));
        materializer.set(mode);
        let fava = assembly(
            &keys,
            Arc::new(MemoryEventCache::default()),
            Arc::clone(&store),
            vec![Arc::clone(&materializer)],
        );

        assert!(
            fava.publish(edit_intent(keys.public_key(), Kind::ContactList))
                .is_err()
        );
        assert_eq!(materializer.calls(), 1);
        assert!(store.is_empty().expect("refusal leaves zero custody"));
    }
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
    let mut observation = fava
        .observe(
            fava::Query::events()
                .authors([keys.public_key()])
                .kind(Kind::ContactList)
                .cache_only(),
        )
        .await
        .expect("semantic query opens");
    let accepted = fava
        .publish(edit_intent(keys.public_key(), Kind::ContactList))
        .unwrap();
    observation
        .changed()
        .await
        .expect("accepted value is public");
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
    assert_eq!(wait_public_failure(&mut observation).await, evidence);
}

#[tokio::test(flavor = "current_thread")]
async fn materializer_panic_is_scoped_and_attributed() {
    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let materializer = Arc::new(ControlledMaterializer::new(Kind::ContactList));
    let healthy = Arc::new(ControlledMaterializer::new(Kind::MuteList));
    let fava = assembly(
        &keys,
        Arc::clone(&cache),
        Arc::new(MemoryWriteStore::default()),
        vec![Arc::clone(&materializer), Arc::clone(&healthy)],
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

    let unaffected = fava
        .publish(edit_intent(keys.public_key(), Kind::MuteList))
        .expect("unrelated receipt accepts");
    save_source(
        &cache,
        signed_source(&keys, Kind::MuteList, 20, "healthy source", &[]),
    );
    let progressed = support::wait_for_materialization(&fava, unaffected.receipt_id, 2).await;
    assert_eq!(
        progressed.current.publication.materialization_id,
        MaterializationId::from_u64(2)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn malformed_and_oversize_outputs_preserve_current() {
    for mode in [WRONG_ACTOR, WRONG_KIND, OVERSIZE] {
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

#[tokio::test(flavor = "current_thread")]
async fn timestamp_and_evidence_overflow_preserve_current() {
    assert_eq!(destination_evidence_capacity(), 256);
    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    save_source(
        &cache,
        signed_source(
            &keys,
            Kind::ContactList,
            u64::MAX - 1,
            "penultimate source",
            &[],
        ),
    );
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
    save_source(
        &cache,
        signed_source(&keys, Kind::ContactList, u64::MAX, "last source", &[]),
    );
    let failed = wait_failure(&fava, accepted.receipt_id).await;
    assert_eq!(failed.current.id(), accepted.current.id());
    assert!(
        failed
            .current
            .publication
            .materialization_failure
            .as_deref()
            .is_some_and(|reason| reason.contains("timestamp exhausted"))
    );

    prove_evidence_exhaustion(&keys);
}

fn prove_evidence_exhaustion(keys: &Keys) {
    let store = MemoryWriteStore::bounded(NonZeroUsize::new(1).unwrap());
    let first = EventBuilder::new(keys.public_key(), Kind::ContactList)
        .created_at(Timestamp::from(1))
        .content("generation zero")
        .build()
        .unwrap();
    let accepted = store
        .accept_materialized_edit(
            edit_intent(keys.public_key(), Kind::ContactList),
            first,
            None,
        )
        .unwrap();
    let mut expected = MaterializationId::from_u64(1);
    let mut expected_source = None;
    for generation in 0..destination_evidence_capacity() {
        let source_time = 2 + generation as u64 * 2;
        let source = signed_source(
            keys,
            Kind::ContactList,
            source_time,
            &format!("source {generation}"),
            &[],
        );
        store
            .install_materialization(
                accepted.write_id,
                accepted.receipt_id,
                expected,
                expected_source,
                EventBuilder::new(keys.public_key(), Kind::ContactList)
                    .created_at(Timestamp::from(source_time + 1))
                    .content(format!("generation {generation}"))
                    .build()
                    .unwrap(),
                Some(&source),
            )
            .unwrap();
        expected = MaterializationId::from_u64(expected.as_u64() + 1);
        expected_source = Some(source.id);
    }
    let before = store.receipt(accepted.receipt_id).unwrap().unwrap();
    let overflow_source = signed_source(keys, Kind::ContactList, 1_000, "overflow source", &[]);
    assert!(
        store
            .install_materialization(
                accepted.write_id,
                accepted.receipt_id,
                expected,
                expected_source,
                EventBuilder::new(keys.public_key(), Kind::ContactList)
                    .created_at(Timestamp::from(1_001))
                    .content("overflow generation")
                    .build()
                    .unwrap(),
                Some(&overflow_source),
            )
            .is_err()
    );
    store
        .record_materialization_failure(
            accepted.write_id,
            accepted.receipt_id,
            expected,
            expected_source,
            Some(&overflow_source),
            "retired materialization evidence capacity reached".to_owned(),
        )
        .unwrap();
    let exhausted = store.receipt(accepted.receipt_id).unwrap().unwrap();
    assert_eq!(exhausted.current.id(), before.current.id());
    assert!(
        exhausted
            .current
            .publication
            .materialization_failure
            .as_deref()
            .is_some_and(|reason| reason.contains("evidence capacity reached"))
    );
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
