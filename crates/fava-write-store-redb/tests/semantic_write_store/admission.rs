use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;

use fava_routing::{CoverageState, RoutePlan, RouteTarget};
use fava_write::{
    EventBuilder, Kind, MaterializationId, ReceiptOutcome, Timestamp, WriteIntent, WriteRouting,
};
use fava_write_store::WriteStore;
use fava_write_store_redb::RedbWriteStore;
use nostr::key::Keys;

use super::{edit, materialization, source, unique_path};

#[test]
fn active_reservation_excludes_unreserved_redb_admission() {
    let path = unique_path("reserved-excludes-unreserved");
    let store = RedbWriteStore::open_bounded(
        &path,
        NonZeroUsize::new(1).unwrap(),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap();
    let semantic_keys = Keys::generate();
    let raw_keys = Keys::generate();
    let reservation = store.reserve_active().expect("semantic slot reserves");
    let raw = WriteIntent::event(
        EventBuilder::new(raw_keys.public_key(), Kind::TextNote)
            .created_at(Timestamp::from(1))
            .content("unreserved")
            .build()
            .unwrap(),
        WriteRouting::Automatic,
    )
    .unwrap();

    assert!(
        store.accept(raw).is_err(),
        "unreserved raw custody must not steal a held semantic slot"
    );
    let accepted = store
        .accept_reserved_materialized_edit(
            reservation,
            WriteIntent::edit_as(edit(), semantic_keys.public_key(), WriteRouting::Automatic)
                .unwrap(),
            materialization(semantic_keys.public_key(), 1, "reserved"),
            None,
            None,
        )
        .expect("the held reservation commits without a second capacity refusal");
    assert_eq!(
        store
            .receipt(accepted.receipt_id)
            .unwrap()
            .unwrap()
            .write_id,
        accepted.write_id
    );
    drop(store);
    std::fs::remove_file(path).ok();
}

#[test]
fn redb_initial_route_idempotence_compares_complete_persisted_effect() {
    let first_target = RouteTarget::Author(Keys::generate().public_key());
    let second_target = RouteTarget::Author(Keys::generate().public_key());
    for (label, first, second) in [
        (
            "shortfall",
            initial_route(vec!["first failure".to_owned()], BTreeMap::new()),
            initial_route(vec!["different failure".to_owned()], BTreeMap::new()),
        ),
        (
            "settled-absent coverage",
            initial_route(
                Vec::new(),
                BTreeMap::from([(first_target.clone(), CoverageState::SettledAbsent)]),
            ),
            initial_route(
                Vec::new(),
                BTreeMap::from([(second_target.clone(), CoverageState::SettledAbsent)]),
            ),
        ),
    ] {
        let path = unique_path(label);
        let store = RedbWriteStore::open_bounded(
            &path,
            NonZeroUsize::new(2).unwrap(),
            NonZeroUsize::new(1).unwrap(),
        )
        .unwrap();
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
                Some(&first),
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
                    Some(&second),
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
        drop(store);
        std::fs::remove_file(path).ok();
    }
}

fn initial_route(
    shortfalls: Vec<String>,
    coverage: BTreeMap<RouteTarget, CoverageState>,
) -> RoutePlan {
    RoutePlan {
        revision: 1,
        destinations: BTreeMap::new(),
        coverage,
        unresolved: BTreeSet::new(),
        shortfalls,
        settled: false,
    }
}

#[test]
fn equal_timestamp_lower_id_is_redb_store_successor() {
    let path = unique_path("equal-time-winner");
    let keys = Keys::generate();
    let store = RedbWriteStore::open(&path).unwrap();
    let left = source(&keys, 10, "left");
    let right = source(&keys, 10, "right");
    let (higher_id, lower_id) = if left.id > right.id {
        (left, right)
    } else {
        (right, left)
    };
    let accepted = store
        .accept_materialized_edit(
            WriteIntent::edit_as(edit(), keys.public_key(), WriteRouting::Automatic).unwrap(),
            materialization(keys.public_key(), 11, "higher-id generation"),
            Some(&higher_id),
        )
        .unwrap();

    let installed = store
        .install_materialization(
            accepted.write_id,
            accepted.receipt_id,
            MaterializationId::from_u64(1),
            Some(higher_id.id),
            materialization(keys.public_key(), 12, "lower-id generation"),
            Some(&lower_id),
        )
        .expect("equal-time lower event id is authoritative");
    assert_eq!(
        installed.current.publication.materialization_source,
        Some(lower_id.id)
    );
    assert!(
        store
            .install_materialization(
                accepted.write_id,
                accepted.receipt_id,
                MaterializationId::from_u64(2),
                Some(lower_id.id),
                materialization(keys.public_key(), 13, "higher-id retry"),
                Some(&higher_id),
            )
            .is_err()
    );
    drop(store);
    std::fs::remove_file(path).ok();
}

#[test]
fn terminal_initial_routes_release_semantic_custody_and_obey_retention() {
    let path = unique_path("terminal-initial-route");
    let store = RedbWriteStore::open_bounded(
        &path,
        NonZeroUsize::new(1).unwrap(),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap();
    let keys = Keys::generate();
    let selected_source = source(&keys, 1, "terminal source");
    let route = RoutePlan {
        revision: 1,
        destinations: BTreeMap::new(),
        coverage: BTreeMap::new(),
        unresolved: BTreeSet::new(),
        shortfalls: Vec::new(),
        settled: true,
    };
    let mut identities = Vec::new();

    for timestamp in [2, 3] {
        let reservation = store.reserve_active().expect("terminal slot reserves");
        let accepted = store
            .accept_reserved_materialized_edit(
                reservation,
                WriteIntent::edit_as(edit(), keys.public_key(), WriteRouting::Automatic).unwrap(),
                materialization(keys.public_key(), timestamp, "terminal"),
                Some(&selected_source),
                Some(&route),
            )
            .expect("terminal route commits atomically");
        let receipt = store
            .receipt(accepted.receipt_id)
            .unwrap()
            .expect("terminal receipt remains readable");
        assert_eq!(receipt.outcome, ReceiptOutcome::NoDestination);
        assert_eq!(
            receipt.current.publication.materialization_source,
            Some(selected_source.id)
        );
        assert_eq!(accepted.current, receipt.current);
        assert!(store.recover_materialized_edits().unwrap().is_empty());
        identities.push(accepted.receipt_id);
    }

    assert_ne!(identities[0], identities[1]);
    assert_eq!(store.len().unwrap(), 1, "terminal retention remains exact");
    assert!(store.receipt(identities[0]).unwrap().is_none());
    drop(store);

    let reopened = RedbWriteStore::open_bounded(
        &path,
        NonZeroUsize::new(1).unwrap(),
        NonZeroUsize::new(1).unwrap(),
    )
    .expect("bounded store reopens");
    assert_eq!(reopened.len().unwrap(), 1);
    let retained = reopened
        .receipt(identities[1])
        .unwrap()
        .expect("terminal receipt remains readable after reopen");
    assert_eq!(
        retained.current.publication.materialization_source,
        Some(selected_source.id)
    );
    assert!(reopened.recover_materialized_edits().unwrap().is_empty());
    drop(reopened);
    std::fs::remove_file(path).ok();
}
