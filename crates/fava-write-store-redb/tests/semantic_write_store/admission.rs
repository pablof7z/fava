use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;

use fava_routing::{CoverageState, RoutePlan, RouteTarget};
use fava_write::{
    EventBuilder, EventValue, Kind, MaterializationId, ReceiptOutcome, Timestamp, WriteIntent,
    WriteRouting,
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
    let reservation = store
        .reserve_active(&edit(), semantic_keys.public_key())
        .expect("semantic slot reserves");
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
    let unresolved = RouteTarget::Author(Keys::generate().public_key());
    for (label, first, second, expected_shortfalls) in [
        (
            "shortfall",
            initial_route(
                vec!["first failure".to_owned()],
                BTreeMap::new(),
                unresolved.clone(),
            ),
            initial_route(
                vec!["different failure".to_owned()],
                BTreeMap::new(),
                unresolved.clone(),
            ),
            vec!["first failure".to_owned()],
        ),
        (
            "settled-absent coverage",
            initial_route(
                Vec::new(),
                BTreeMap::from([(first_target.clone(), CoverageState::SettledAbsent)]),
                unresolved.clone(),
            ),
            initial_route(
                Vec::new(),
                BTreeMap::from([(second_target.clone(), CoverageState::SettledAbsent)]),
                unresolved.clone(),
            ),
            vec![format!("no relay destination for {first_target:?}")],
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
                store.reserve_active(&edit(), keys.public_key()).unwrap(),
                intent(),
                event(),
                None,
                Some(&first),
            )
            .expect("first route effect commits");
        let retained = store.receipt(accepted.receipt_id).unwrap().unwrap();
        assert_eq!(retained.route_shortfalls, expected_shortfalls, "{label}");
        let mut changes = store.receipt_changes();
        let replayed = store
            .accept_reserved_materialized_edit(
                store.reserve_active(&edit(), keys.public_key()).unwrap(),
                intent(),
                event(),
                None,
                Some(&first),
            )
            .expect("exact persisted route effect replays idempotently");
        assert_eq!(replayed, accepted);
        assert_eq!(
            store.receipt(accepted.receipt_id).unwrap(),
            Some(retained.clone())
        );
        assert!(changes.try_recv().is_err(), "{label} exact replay notified");
        assert!(
            store
                .accept_reserved_materialized_edit(
                    store.reserve_active(&edit(), keys.public_key()).unwrap(),
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
                    .reserve_active(&edit(), keys.public_key())
                    .expect("refusal consumed reservation"),
            )
            .unwrap();
        drop(store);
        std::fs::remove_file(path).ok();
    }
}

#[test]
fn redb_apply_route_replay_compares_complete_persisted_effect() {
    let path = unique_path("apply-route-replay-effect");
    let store = RedbWriteStore::open(&path).unwrap();
    let keys = Keys::generate();
    let accepted = store
        .accept_materialized_edit(
            WriteIntent::edit_as(edit(), keys.public_key(), WriteRouting::Automatic).unwrap(),
            materialization(keys.public_key(), 1, "apply route"),
            None,
        )
        .unwrap();
    let first_target = RouteTarget::Author(Keys::generate().public_key());
    let second_target = RouteTarget::Author(Keys::generate().public_key());
    let unresolved = RouteTarget::Author(Keys::generate().public_key());
    let first = initial_route(
        Vec::new(),
        BTreeMap::from([(first_target.clone(), CoverageState::SettledAbsent)]),
        unresolved.clone(),
    );
    let applied = store
        .apply_route(
            accepted.write_id,
            accepted.receipt_id,
            accepted.current.publication.materialization_id,
            accepted.current.id(),
            &first,
        )
        .expect("first complete route effect applies");
    assert_eq!(
        applied.route_shortfalls,
        vec![format!("no relay destination for {first_target:?}")]
    );

    let mut changes = store.receipt_changes();
    assert_eq!(
        store
            .apply_route(
                accepted.write_id,
                accepted.receipt_id,
                accepted.current.publication.materialization_id,
                accepted.current.id(),
                &first,
            )
            .expect("exact route effect replays idempotently"),
        applied
    );
    assert!(changes.try_recv().is_err(), "exact replay notified");

    let mismatch = initial_route(
        applied.route_shortfalls.clone(),
        BTreeMap::from([(second_target, CoverageState::SettledAbsent)]),
        unresolved,
    );
    assert!(
        store
            .apply_route(
                accepted.write_id,
                accepted.receipt_id,
                accepted.current.publication.materialization_id,
                accepted.current.id(),
                &mismatch,
            )
            .is_err(),
        "coverage-derived shortfall mismatch was accepted"
    );
    assert_eq!(store.receipt(accepted.receipt_id).unwrap(), Some(applied));
    assert!(changes.try_recv().is_err(), "mismatch notified");
    drop(store);
    std::fs::remove_file(path).ok();
}

fn initial_route(
    shortfalls: Vec<String>,
    coverage: BTreeMap<RouteTarget, CoverageState>,
    unresolved: RouteTarget,
) -> RoutePlan {
    RoutePlan {
        revision: 1,
        destinations: BTreeMap::new(),
        coverage,
        unresolved: BTreeSet::from([unresolved]),
        shortfalls,
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
            Some(&EventValue::Signed(higher_id.clone())),
        )
        .unwrap();

    let installed = store
        .install_materialization(
            accepted.write_id,
            accepted.receipt_id,
            MaterializationId::FIRST,
            Some(higher_id.id),
            std::slice::from_ref(&edit()),
            materialization(keys.public_key(), 12, "lower-id generation"),
            Some(&EventValue::Signed(lower_id.clone())),
            None,
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
                MaterializationId::try_from(2).expect("nonzero materialization identity"),
                Some(lower_id.id),
                std::slice::from_ref(&edit()),
                materialization(keys.public_key(), 13, "higher-id retry"),
                Some(&EventValue::Signed(higher_id.clone())),
                None,
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
    };
    let mut identities = Vec::new();

    for timestamp in [2, 3] {
        let reservation = store
            .reserve_active(&edit(), keys.public_key())
            .expect("terminal slot reserves");
        let accepted = store
            .accept_reserved_materialized_edit(
                reservation,
                WriteIntent::edit_as(edit(), keys.public_key(), WriteRouting::Automatic).unwrap(),
                materialization(keys.public_key(), timestamp, "terminal"),
                Some(&EventValue::Signed(selected_source.clone())),
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
