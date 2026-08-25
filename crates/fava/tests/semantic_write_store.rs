//! Public contract evidence for volatile semantic-write custody.
//!
//! This file stays above the 500-line soft limit because it owns the complete volatile-store
//! admission, recovery, failure, and evidence-exhaustion matrix; author and current-guard
//! concerns are already split into their adjacent cohesive modules.
use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;

use fava_routing::{CoverageState, RoutePlan, RouteTarget};
use fava_write::{
    Event, EventBuilder, Kind, MaterializationId, ReplaceableEventEdit, Timestamp, UnsignedEvent,
    WriteIntent, WriteRouting,
};
use fava_write_store::{WriteStore, destination_evidence_capacity};
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::FinalizeEvent;
use nostr::key::Keys;
#[path = "semantic_write_store/author.rs"]
mod author;
#[path = "semantic_write_store/current_guard.rs"]
mod current_guard;

fn edit() -> ReplaceableEventEdit {
    ReplaceableEventEdit::new(Kind::ContactList, None, vec![1]).expect("bounded edit")
}

fn materialization(actor: fava_write::PublicKey, created_at: u64, content: &str) -> UnsignedEvent {
    EventBuilder::new(actor, Kind::ContactList)
        .created_at(Timestamp::from(created_at))
        .content(content)
        .build()
        .expect("valid materialization")
}
fn source(keys: &Keys, created_at: u64, content: &str) -> Event {
    materialization(keys.public_key(), created_at, content)
        .finalize(keys)
        .expect("valid signed source")
}
fn accept(
    store: &MemoryWriteStore,
    edit: ReplaceableEventEdit,
    author: fava_write::PublicKey,
    event: UnsignedEvent,
    source: Option<&Event>,
) -> fava_write_store::AcceptedWrite {
    store
        .accept_materialized_edit(
            WriteIntent::edit_as(edit, author, WriteRouting::Automatic).expect("valid edit intent"),
            event,
            source,
        )
        .expect("semantic write accepted")
}

#[test]
fn memory_first_edit_has_no_prior() {
    let keys = Keys::generate();
    let store = MemoryWriteStore::default();
    let accepted = accept(
        &store,
        edit(),
        keys.public_key(),
        materialization(keys.public_key(), 10, "first"),
        None,
    );

    let receipt = store
        .receipt(accepted.receipt_id)
        .expect("store readable")
        .expect("receipt retained");
    assert_eq!(
        receipt.current.publication.materialization_id,
        MaterializationId::from_u64(1)
    );
    assert_eq!(receipt.current.publication.materialization_source, None);
    assert!(
        receipt
            .current
            .publication
            .retired_materializations
            .is_empty()
    );
}

#[test]
fn memory_initial_route_idempotence_compares_complete_persisted_effect() {
    for (label, first, second) in route_effect_mismatches() {
        let store = MemoryWriteStore::bounded(NonZeroUsize::new(2).unwrap());
        assert_route_effect_mismatch_is_atomic(&store, label, &first, &second);
    }
}

fn route_effect_mismatches() -> Vec<(&'static str, RoutePlan, RoutePlan)> {
    let first_target = RouteTarget::Author(Keys::generate().public_key());
    let second_target = RouteTarget::Author(Keys::generate().public_key());
    vec![
        (
            "shortfall",
            initial_route(vec!["first failure".to_owned()], BTreeMap::new()),
            initial_route(vec!["different failure".to_owned()], BTreeMap::new()),
        ),
        (
            "settled-absent coverage",
            initial_route(
                Vec::new(),
                BTreeMap::from([(first_target, CoverageState::SettledAbsent)]),
            ),
            initial_route(
                Vec::new(),
                BTreeMap::from([(second_target, CoverageState::SettledAbsent)]),
            ),
        ),
    ]
}

fn initial_route(
    shortfalls: Vec<String>,
    coverage: BTreeMap<RouteTarget, CoverageState>,
) -> RoutePlan {
    RoutePlan {
        revision: 1,
        destinations: BTreeMap::new(),
        coverage,
        unresolved: BTreeSet::from([RouteTarget::WholeRequest]),
        shortfalls,
    }
}

fn assert_route_effect_mismatch_is_atomic(
    store: &MemoryWriteStore,
    label: &str,
    first: &RoutePlan,
    second: &RoutePlan,
) {
    let keys = Keys::generate();
    let intent =
        || WriteIntent::edit_as(edit(), keys.public_key(), WriteRouting::Automatic).unwrap();
    let event = || materialization(keys.public_key(), 1, "route effect");
    let accepted = store
        .accept_reserved_materialized_edit(
            store.reserve_active().unwrap(),
            intent(),
            event(),
            None,
            Some(first),
        )
        .expect("first route effect commits");
    let retained = store.receipt(accepted.receipt_id).unwrap().unwrap();
    let mut changes = store.receipt_changes();
    assert!(
        store
            .accept_reserved_materialized_edit(
                store.reserve_active().unwrap(),
                intent(),
                event(),
                None,
                Some(second),
            )
            .is_err(),
        "{label} mismatch was accepted as idempotent"
    );
    assert_eq!(store.receipt(accepted.receipt_id).unwrap(), Some(retained));
    assert!(changes.try_recv().is_err(), "{label} mismatch notified");
    store
        .release_active(
            store
                .reserve_active()
                .expect("refusal consumed reservation"),
        )
        .unwrap();
}

#[test]
fn memory_generation_swap_is_compare_and_set() {
    let keys = Keys::generate();
    let store = MemoryWriteStore::default();
    let base = source(&keys, 10, "base");
    let accepted = accept(
        &store,
        edit(),
        keys.public_key(),
        materialization(keys.public_key(), 11, "generation one"),
        Some(&base),
    );
    let successor_source = source(&keys, 20, "successor source");
    let successor = store
        .install_materialization(
            accepted.write_id,
            accepted.receipt_id,
            MaterializationId::from_u64(1),
            Some(base.id),
            materialization(keys.public_key(), 21, "generation two"),
            Some(&successor_source),
        )
        .expect("current generation swaps");

    assert_eq!(successor.write_id, accepted.write_id);
    assert_eq!(successor.receipt_id, accepted.receipt_id);
    assert_eq!(
        successor.current.publication.materialization_id,
        MaterializationId::from_u64(2)
    );
    assert_eq!(
        successor.current.publication.materialization_source,
        Some(successor_source.id)
    );
    assert_eq!(
        successor.current.publication.retired_materializations,
        vec![(
            MaterializationId::from_u64(1),
            accepted.current.id(),
            Some(base.id),
            None,
        )]
    );

    let before_stale = successor.clone();
    let later_source = source(&keys, 30, "later source");
    assert!(
        store
            .install_materialization(
                accepted.write_id,
                accepted.receipt_id,
                MaterializationId::from_u64(1),
                Some(successor_source.id),
                materialization(keys.public_key(), 31, "stale swap"),
                Some(&later_source),
            )
            .is_err()
    );
    assert_eq!(
        store.receipt(accepted.receipt_id).unwrap(),
        Some(before_stale)
    );
}

#[test]
fn memory_unqualified_source_is_inert() {
    let keys = Keys::generate();
    let store = MemoryWriteStore::default();
    let selected = source(&keys, 20, "selected");
    let accepted = accept(
        &store,
        edit(),
        keys.public_key(),
        materialization(keys.public_key(), 21, "current"),
        Some(&selected),
    );
    let mut changes = store.receipt_changes();
    let older = source(&keys, 19, "older");
    for candidate in [&selected, &older] {
        assert!(
            store
                .install_materialization(
                    accepted.write_id,
                    accepted.receipt_id,
                    MaterializationId::from_u64(1),
                    Some(selected.id),
                    materialization(keys.public_key(), 22, "inert"),
                    Some(candidate),
                )
                .is_err()
        );
    }
    assert!(
        store
            .install_materialization(
                accepted.write_id,
                accepted.receipt_id,
                MaterializationId::from_u64(1),
                Some(selected.id),
                materialization(keys.public_key(), 22, "missing source"),
                None,
            )
            .is_ok(),
        "source removal is a qualified transition"
    );
    let removed = store.receipt(accepted.receipt_id).unwrap().unwrap();
    assert_eq!(removed.current.publication.materialization_source, None);

    let unchanged = removed.clone();
    assert!(
        store
            .install_materialization(
                accepted.write_id,
                accepted.receipt_id,
                MaterializationId::from_u64(2),
                None,
                materialization(keys.public_key(), 23, "already empty"),
                None,
            )
            .is_err()
    );
    assert_eq!(
        store.receipt(accepted.receipt_id).unwrap(),
        Some(unchanged.clone())
    );
    assert_eq!(changes.try_recv().unwrap().0, accepted.receipt_id);
    assert!(matches!(
        changes.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
}

#[test]
fn memory_failure_preserves_current_and_is_attributed() {
    let keys = Keys::generate();
    let store = MemoryWriteStore::default();
    let base = source(&keys, 10, "base");
    let accepted = accept(
        &store,
        edit(),
        keys.public_key(),
        materialization(keys.public_key(), 11, "current"),
        Some(&base),
    );
    let failed_source = source(&keys, 20, "failed source");
    let before = store.receipt(accepted.receipt_id).unwrap().unwrap();
    let record_failure = || {
        store.record_materialization_failure(
            accepted.write_id,
            accepted.receipt_id,
            MaterializationId::from_u64(1),
            Some(base.id),
            Some(&failed_source),
            "x".repeat(5_000),
        )
    };
    let failed = record_failure().expect("post-accept failure is durable evidence");

    let mut without_failure = failed.clone();
    without_failure.current.publication.materialization_failure = None;
    assert_eq!(without_failure, before);
    let failure = failed
        .current
        .publication
        .materialization_failure
        .as_deref()
        .expect("failure is visible");
    assert!(failure.contains(&failed_source.id.to_string()));
    assert!(failure.ends_with("failed"));
    assert!(failure.len() <= 4_096);
    let mut changes = store.receipt_changes();
    assert_eq!(record_failure().unwrap(), failed);
    assert!(changes.try_recv().is_err());

    let recovered = store.recover_materialized_edits().unwrap();
    assert_eq!(recovered[0].2, keys.public_key());
    assert_eq!(recovered[0].4, Some(failed_source.id));
}

#[test]
fn memory_successful_retry_clears_failure_atomically() {
    let keys = Keys::generate();
    let store = MemoryWriteStore::default();
    let base = source(&keys, 10, "base");
    let accepted = accept(
        &store,
        edit(),
        keys.public_key(),
        materialization(keys.public_key(), 11, "current"),
        Some(&base),
    );
    let retry_source = source(&keys, 20, "retry source");
    store
        .record_materialization_failure(
            accepted.write_id,
            accepted.receipt_id,
            MaterializationId::from_u64(1),
            Some(base.id),
            Some(&retry_source),
            "first attempt failed".to_owned(),
        )
        .unwrap();
    let mut changes = store.receipt_changes();
    let successor_event = materialization(keys.public_key(), 21, "retry succeeded");
    let successor = store
        .install_materialization(
            accepted.write_id,
            accepted.receipt_id,
            MaterializationId::from_u64(1),
            Some(base.id),
            successor_event.clone(),
            Some(&retry_source),
        )
        .expect("retry installs atomically");

    assert_eq!(
        successor.current.publication.materialization_id,
        MaterializationId::from_u64(2)
    );
    assert_eq!(
        successor.current.publication.materialization_source,
        Some(retry_source.id)
    );
    assert_eq!(successor.current.publication.materialization_failure, None);
    assert!(
        successor.current.publication.retired_materializations[0]
            .3
            .as_deref()
            .is_some_and(|failure| failure.contains("first attempt failed"))
    );
    assert_eq!(store.recover_materialized_edits().unwrap()[0].4, None);
    assert_eq!(changes.try_recv().unwrap().1, Some(successor.clone()));

    let repeated = store
        .install_materialization(
            accepted.write_id,
            accepted.receipt_id,
            MaterializationId::from_u64(2),
            Some(retry_source.id),
            successor_event,
            Some(&retry_source),
        )
        .expect("repeated success is idempotent");
    assert_eq!(repeated, successor);
    assert!(matches!(
        changes.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
}

#[test]
fn memory_live_edit_recovers_once_and_terminal_is_inert() {
    let keys = Keys::generate();
    let store = MemoryWriteStore::default();
    let accepted = accept(
        &store,
        edit(),
        keys.public_key(),
        materialization(keys.public_key(), 10, "live"),
        None,
    );
    let recovered = store.recover_materialized_edits().unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].0.receipt_id, accepted.receipt_id);
    assert_eq!(recovered[0].2, keys.public_key());

    let cancelled = store.cancel(accepted.receipt_id).unwrap().unwrap();
    assert!(cancelled.is_terminal());
    assert!(store.recover_materialized_edits().unwrap().is_empty());
    assert!(
        store
            .record_materialization_failure(
                accepted.write_id,
                accepted.receipt_id,
                MaterializationId::from_u64(1),
                None,
                None,
                "late completion".to_owned(),
            )
            .is_err()
    );

    let replacement = accept(
        &store,
        edit(),
        keys.public_key(),
        materialization(keys.public_key(), 20, "new owner"),
        None,
    );
    let settled = store
        .apply_route(
            replacement.write_id,
            replacement.receipt_id,
            replacement.current.publication.materialization_id,
            replacement.current.id(),
            &RoutePlan {
                revision: 1,
                destinations: BTreeMap::new(),
                coverage: BTreeMap::new(),
                unresolved: BTreeSet::new(),
                shortfalls: Vec::new(),
            },
        )
        .expect("empty route settles terminally");
    assert!(settled.is_terminal());
    assert!(store.recover_materialized_edits().unwrap().is_empty());
}

#[test]
fn memory_evidence_exhaustion_has_no_partial_effect() {
    assert_eq!(destination_evidence_capacity(), 256);
    let bounded = MemoryWriteStore::bounded(NonZeroUsize::new(1).unwrap());
    assert_eq!(bounded.active_capacity(), 1);
    let first_keys = Keys::generate();
    accept(
        &bounded,
        edit(),
        first_keys.public_key(),
        materialization(first_keys.public_key(), 1, "capacity owner"),
        None,
    );
    let second_keys = Keys::generate();
    assert!(
        bounded
            .accept_materialized_edit(
                WriteIntent::edit_as(edit(), second_keys.public_key(), WriteRouting::Automatic,)
                    .unwrap(),
                materialization(second_keys.public_key(), 1, "refused"),
                None,
            )
            .is_err()
    );
    assert_eq!(bounded.recover_materialized_edits().unwrap().len(), 1);

    let store = MemoryWriteStore::default();
    let keys = Keys::generate();
    let accepted = accept(
        &store,
        edit(),
        keys.public_key(),
        materialization(keys.public_key(), 1, "generation zero"),
        None,
    );
    let mut expected = MaterializationId::from_u64(1);
    let mut expected_source = None;
    for generation in 0..destination_evidence_capacity() {
        let source_time = 2 + (generation as u64 * 2);
        let next_source = source(&keys, source_time, &format!("source {generation}"));
        store
            .install_materialization(
                accepted.write_id,
                accepted.receipt_id,
                expected,
                expected_source,
                materialization(
                    keys.public_key(),
                    source_time + 1,
                    &format!("generation {generation}"),
                ),
                Some(&next_source),
            )
            .unwrap();
        expected = MaterializationId::from_u64(expected.as_u64() + 1);
        expected_source = Some(next_source.id);
    }
    let before = store.receipt(accepted.receipt_id).unwrap().unwrap();
    let mut changes = store.receipt_changes();
    let overflow_source = source(&keys, 1_000, "overflow source");
    assert!(
        store
            .install_materialization(
                accepted.write_id,
                accepted.receipt_id,
                expected,
                expected_source,
                materialization(keys.public_key(), 1_001, "overflow generation"),
                Some(&overflow_source),
            )
            .is_err()
    );
    assert_eq!(store.receipt(accepted.receipt_id).unwrap(), Some(before));
    assert!(matches!(
        changes.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
}
