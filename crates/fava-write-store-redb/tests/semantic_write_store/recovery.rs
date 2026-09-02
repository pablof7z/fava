use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;

use fava_query::{Query, QuerySource, SourceEvent};
use fava_relay::Authority;
use fava_routing::RoutePlan;
use fava_write::{
    EventValue, Receipt, ReceiptId, ReceiptOutcome, RelayDeliveryOutcome, RevisionId,
    SignatureState, WriteIntent, WriteRouting,
};
use fava_write_store::{WriteStore, destination_evidence_capacity};
use fava_write_store_redb::RedbWriteStore;
use nostr::key::Keys;
use nostr::types::RelayUrl;
use redb::{Database, Durability, ReadableDatabase, ReadableTable};
use serde_json::{Value, json};

use super::{META, RECEIPTS, accept, edit, revision, source, unique_path};

#[test]
fn ordered_explicit_route_survives_reopen_with_one_lane_per_identity() {
    let path = unique_path("ordered-explicit-route");
    let keys = Keys::generate();
    let first = RelayUrl::parse("wss://first.example").unwrap();
    let second = RelayUrl::parse("wss://second.example").unwrap();
    let routing = WriteRouting::explicit([first.clone(), second.clone(), first.clone()])
        .expect("route normalizes");
    let store = RedbWriteStore::open(&path).unwrap();
    let accepted = store
        .accept_applied_edit(
            WriteIntent::edit_as(edit(), keys.public_key(), routing).unwrap(),
            revision(keys.public_key(), 10, "ordered"),
            None,
        )
        .expect("semantic write accepts");
    let receipt_id = accepted.receipt_id.as_u64();
    drop(store);

    let reopened = RedbWriteStore::open(path).expect("ordered route reopens");
    let receipt = reopened
        .receipt(ReceiptId::try_from(receipt_id).expect("persisted receipt identity reattaches"))
        .unwrap()
        .expect("receipt persists");
    assert_eq!(receipt.routing, WriteRouting::Explicit(vec![first, second]));
    assert_eq!(receipt.destinations().len(), 2);
    assert_eq!(receipt.desired_destinations.len(), 2);
}

#[test]
fn row_mutation_refuses_an_access_authority_that_cannot_be_trusted() {
    // Absent. The field has no default, so a row without it does not decode --
    // reconstruction refuses rather than silently treating the write as public.
    let absent_path = terminal_no_destination_path("absent-access");
    mutate_row(&absent_path, |row| {
        row.pointer_mut("/receipt")
            .and_then(serde_json::Value::as_object_mut)
            .expect("the receipt is an object")
            .remove("access");
    });
    let absent = RedbWriteStore::open(absent_path)
        .err()
        .expect("a write with no recorded authority refuses");

    // Malformed. Same door: an authority that cannot be decoded is not an
    // authority, and defaulting it would send the write out as public work.
    let malformed_path = terminal_no_destination_path("malformed-access");
    mutate_row(&malformed_path, |row| {
        set(
            row,
            "/receipt/access",
            serde_json::json!("not an authority"),
        );
    });
    let malformed = RedbWriteStore::open(malformed_path)
        .err()
        .expect("a write whose authority cannot be decoded refuses");

    for error in [absent, malformed] {
        assert!(
            !error.to_string().is_empty(),
            "the refusal names what it could not read"
        );
    }

    // Contradictory no longer has a door: a destination used to carry its own
    // access alongside the receipt's, so the two could disagree. A destination
    // is now a bare relay, so there is nothing left for the receipt's authority
    // to contradict.
}

#[test]
fn row_mutation_refuses_unsound_ordered_route_shapes() {
    let empty_path = terminal_no_destination_path("empty-explicit-route");
    mutate_row(&empty_path, |row| {
        set(
            row,
            "/receipt/routing",
            serde_json::to_value(WriteRouting::Explicit(Vec::new())).unwrap(),
        );
    });
    let empty_error = RedbWriteStore::open(empty_path)
        .err()
        .expect("empty explicit route refuses");
    assert!(
        empty_error
            .to_string()
            .contains("durable explicit route is empty"),
        "unexpected empty-route refusal: {empty_error}"
    );

    let duplicate_path = terminal_no_destination_path("duplicate-explicit-route");
    let relay = RelayUrl::parse("wss://duplicate.example").unwrap();
    mutate_row(&duplicate_path, |row| {
        set(
            row,
            "/receipt/routing",
            serde_json::to_value(WriteRouting::Explicit(vec![relay.clone(), relay])).unwrap(),
        );
    });
    let duplicate_error = RedbWriteStore::open(duplicate_path)
        .err()
        .expect("duplicate explicit route refuses");
    assert!(
        duplicate_error
            .to_string()
            .contains("durable explicit route repeats a relay identity"),
        "unexpected duplicate-route refusal: {duplicate_error}"
    );
}

#[test]
fn row_mutation_refuses_missing_extra_and_substituted_explicit_lanes() {
    let (missing_path, _first, second) = explicit_path("missing-explicit-lane");
    let second_lane = second.clone();
    mutate_typed_receipt(&missing_path, |receipt| {
        receipt
            .current
            .publication
            .destinations
            .remove(&second_lane);
        receipt.desired_destinations.remove(&second_lane);
    });
    assert_lane_mismatch(missing_path, "missing");

    let (extra_path, _, _) = explicit_path("extra-explicit-lane");
    let extra = RelayUrl::parse("wss://extra.example").unwrap();
    mutate_typed_receipt(&extra_path, |receipt| {
        receipt
            .current
            .publication
            .destinations
            .insert(extra.clone(), RelayDeliveryOutcome::Pending);
        receipt.desired_destinations.insert(extra);
    });
    assert_lane_mismatch(extra_path, "extra");

    let (substituted_path, _, second) = explicit_path("substituted-explicit-lane");
    let expected = second;
    let substitute = RelayUrl::parse("wss://substitute.example").unwrap();
    mutate_typed_receipt(&substituted_path, |receipt| {
        let outcome = receipt
            .current
            .publication
            .destinations
            .remove(&expected)
            .expect("expected lane exists");
        receipt
            .current
            .publication
            .destinations
            .insert(substitute.clone(), outcome);
        receipt.desired_destinations.remove(&expected);
        receipt.desired_destinations.insert(substitute);
    });
    assert_lane_mismatch(substituted_path, "substituted");
}

#[test]
fn exact_current_guard_precedes_idempotent_semantic_success() {
    let path = unique_path("exact-current-first");
    let keys = Keys::generate();
    let base = source(&keys, 10, "base");
    let successor_source = source(&keys, 20, "successor");
    let store = RedbWriteStore::open(path).unwrap();
    let accepted = accept(
        &store,
        edit(),
        keys.public_key(),
        revision(keys.public_key(), 11, "generation one"),
        Some(&EventValue::Signed(base.clone())),
    );
    let successor_event = revision(keys.public_key(), 21, "generation two");
    let successor = store
        .install_revision(
            accepted.write_id,
            accepted.receipt_id,
            RevisionId::FIRST,
            Some(base.id),
            std::slice::from_ref(&edit()),
            successor_event.clone(),
            Some(&EventValue::Signed(successor_source.clone())),
            None,
        )
        .unwrap();
    let mut changes = store.receipt_changes();

    assert!(
        store
            .install_revision(
                accepted.write_id,
                accepted.receipt_id,
                RevisionId::FIRST,
                Some(base.id),
                std::slice::from_ref(&edit()),
                successor_event.clone(),
                Some(&EventValue::Signed(successor_source.clone())),
                None,
            )
            .is_err(),
        "stale identity was accepted through the idempotent fast path"
    );
    assert_eq!(store.receipt(accepted.receipt_id).unwrap(), Some(successor));
    assert!(changes.try_recv().is_err(), "stale success notified");

    store
        .install_revision(
            accepted.write_id,
            accepted.receipt_id,
            RevisionId::try_from(2).expect("nonzero revision identity"),
            Some(successor_source.id),
            std::slice::from_ref(&edit()),
            successor_event,
            Some(&EventValue::Signed(successor_source.clone())),
            None,
        )
        .expect("exact idempotent replay remains accepted");
}

#[test]
fn terminal_eviction_retains_terminalizing_receipt_across_reopen() {
    let path = unique_path("terminal-self-eviction");
    let store = RedbWriteStore::open_bounded(
        &path,
        NonZeroUsize::new(2).unwrap(),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap();
    let first_keys = Keys::generate();
    let second_keys = Keys::generate();
    let first = accept(
        &store,
        edit(),
        first_keys.public_key(),
        revision(first_keys.public_key(), 10, "first"),
        None,
    );
    let second = accept(
        &store,
        edit(),
        second_keys.public_key(),
        revision(second_keys.public_key(), 10, "second"),
        None,
    );
    let mut changes = store.receipt_changes();
    settle_no_destination(&store, &second);
    while changes.try_recv().is_ok() {}
    settle_no_destination(&store, &first);

    assert!(store.receipt(first.receipt_id).unwrap().is_some());
    assert!(store.receipt(second.receipt_id).unwrap().is_none());
    assert_eq!(changes.try_recv().unwrap(), (second.receipt_id, None));
    assert_eq!(changes.try_recv().unwrap().0, first.receipt_id);
    assert!(changes.try_recv().is_err());
    assert_published_receipt(&store, &first.current);
    drop(store);

    let reopened = RedbWriteStore::open_bounded(
        path,
        NonZeroUsize::new(2).unwrap(),
        NonZeroUsize::new(1).unwrap(),
    )
    .expect("bounded terminal state reopens");
    assert!(reopened.receipt(first.receipt_id).unwrap().is_some());
    assert!(reopened.receipt(second.receipt_id).unwrap().is_none());
    assert_published_receipt(&reopened, &first.current);
}

#[test]
fn reopen_refuses_recovered_counts_beyond_configured_bounds_without_dropping_rows() {
    let active_path = unique_path("recovered-active-bound");
    let active = RedbWriteStore::open_bounded(
        &active_path,
        NonZeroUsize::new(2).unwrap(),
        NonZeroUsize::new(2).unwrap(),
    )
    .unwrap();
    for actor in [Keys::generate().public_key(), Keys::generate().public_key()] {
        accept(&active, edit(), actor, revision(actor, 10, "active"), None);
    }
    drop(active);
    assert!(
        RedbWriteStore::open_bounded(
            &active_path,
            NonZeroUsize::new(1).unwrap(),
            NonZeroUsize::new(2).unwrap(),
        )
        .is_err()
    );
    assert_eq!(
        RedbWriteStore::open_bounded(
            active_path,
            NonZeroUsize::new(2).unwrap(),
            NonZeroUsize::new(2).unwrap(),
        )
        .unwrap()
        .recover_open()
        .unwrap()
        .len(),
        2,
        "refused open mutated or dropped active custody"
    );

    let terminal_path = unique_path("recovered-terminal-bound");
    let terminal = RedbWriteStore::open_bounded(
        &terminal_path,
        NonZeroUsize::new(2).unwrap(),
        NonZeroUsize::new(2).unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for actor in [Keys::generate().public_key(), Keys::generate().public_key()] {
        let accepted = accept(
            &terminal,
            edit(),
            actor,
            revision(actor, 10, "terminal"),
            None,
        );
        settle_no_destination(&terminal, &accepted);
        ids.push(accepted.receipt_id);
    }
    drop(terminal);
    assert!(
        RedbWriteStore::open_bounded(
            &terminal_path,
            NonZeroUsize::new(2).unwrap(),
            NonZeroUsize::new(1).unwrap(),
        )
        .is_err()
    );
    let reopened = RedbWriteStore::open_bounded(
        terminal_path,
        NonZeroUsize::new(2).unwrap(),
        NonZeroUsize::new(2).unwrap(),
    )
    .unwrap();
    assert!(
        ids.into_iter()
            .all(|id| reopened.receipt(id).unwrap().is_some())
    );
}

#[test]
fn row_mutation_reconstruction_refuses_every_malformed_invariant() {
    assert_row_mutation_refused("semantic-author", |row| {
        set(
            row,
            "/semantic/author",
            serde_json::to_value(Keys::generate().public_key()).unwrap(),
        );
    });
    assert_row_mutation_refused("publication-identity", |row| {
        set(row, "/receipt/current/publication/receipt_id", json!(99));
    });
    assert_row_mutation_refused("event-identity", |row| {
        let other = revision(Keys::generate().public_key(), 99, "other")
            .id
            .unwrap();
        set(
            row,
            "/receipt/current/id",
            serde_json::to_value(other).unwrap(),
        );
    });
    assert_row_mutation_refused("signature-identity", |row| {
        set(
            row,
            "/receipt/current/publication/signature",
            serde_json::to_value(SignatureState::Signed).unwrap(),
        );
    });
    assert_row_mutation_refused("receipt-text", |row| {
        set(row, "/receipt/route_shortfalls", json!(["x".repeat(5_000)]));
    });
    assert_row_mutation_refused("outcome-coherence", |row| {
        set(
            row,
            "/receipt/outcome",
            serde_json::to_value(ReceiptOutcome::Complete).unwrap(),
        );
    });

    let destination_path = valid_path("destination-bound");
    let mut oversized = BTreeMap::new();
    for index in 0..=destination_evidence_capacity() {
        let relay = RelayUrl::parse(&format!("wss://relay-{index}.example")).unwrap();
        oversized.insert(relay, RelayDeliveryOutcome::Pending);
    }
    let receipt = read_receipt(&destination_path);
    let mut publication = receipt.current.publication;
    publication.destinations = oversized;
    let encoded = serde_json::to_value(publication).unwrap();
    mutate_row(&destination_path, |row| {
        set(
            row,
            "/receipt/current/publication/destinations",
            encoded["destinations"].clone(),
        );
    });
    assert!(RedbWriteStore::open(destination_path).is_err());

    let next_id_path = valid_path("next-id");
    set_next_id(&next_id_path, 1);
    assert!(RedbWriteStore::open(next_id_path).is_err());

    let keys = Keys::generate();
    let base = source(&keys, 10, "base");
    let failed = source(&keys, 20, "failed");
    let failed_path = unique_path("failed-attribution");
    let store = RedbWriteStore::open(&failed_path).unwrap();
    let accepted = accept(
        &store,
        edit(),
        keys.public_key(),
        revision(keys.public_key(), 11, "current"),
        Some(&EventValue::Signed(base.clone())),
    );
    store
        .record_revision_failure(
            accepted.write_id,
            accepted.receipt_id,
            RevisionId::FIRST,
            Some(base.id),
            Some(&EventValue::Signed(failed.clone())),
            "failed".to_owned(),
        )
        .unwrap();
    drop(store);
    mutate_row(&failed_path, |row| {
        set(
            row,
            "/receipt/current/publication/revision_failure",
            json!("wrong attribution"),
        );
    });
    assert!(RedbWriteStore::open(failed_path).is_err());

    let timestamp_path = valid_source_path("source-timestamp");
    mutate_row(&timestamp_path, |row| {
        set(row, "/semantic/current_source/1", json!(12));
    });
    assert!(RedbWriteStore::open(timestamp_path).is_err());
}

#[test]
fn zero_write_and_receipt_identity_cannot_reenter_through_recovery() {
    let path = valid_path("zero-durable-identity");
    move_row_to_zero(&path);

    let error = RedbWriteStore::open(path)
        .err()
        .expect("zero durable identity refuses");
    assert!(
        error.to_string().contains("nonzero") || error.to_string().contains("identity is zero"),
        "unexpected zero-identity refusal: {error}"
    );
}

#[test]
fn exhausted_durable_identity_refuses_acceptance_atomically() {
    let path = valid_path("exhausted-durable-identity");
    set_next_id(&path, u64::MAX);
    let store = RedbWriteStore::open(&path).expect("maximum next identity recovers");
    let mut changes = store.receipt_changes();
    let keys = Keys::generate();
    let error = store
        .accept_applied_edit(
            WriteIntent::edit_as(edit(), keys.public_key(), WriteRouting::Automatic).unwrap(),
            revision(keys.public_key(), 20, "exhausted"),
            None,
        )
        .expect_err("exhausted identity refuses");
    assert_eq!(
        error.to_string(),
        "write store refused operation: write identity exhausted"
    );
    assert_eq!(store.len().unwrap(), 1);
    assert!(changes.try_recv().is_err());
    assert!(
        store
            .receipt(ReceiptId::try_from(u64::MAX).expect("maximum is nonzero"))
            .unwrap()
            .is_none()
    );
    drop(store);

    assert_eq!(read_next_id(&path), u64::MAX);
    assert_eq!(RedbWriteStore::open(path).unwrap().len().unwrap(), 1);
}

#[test]
fn schema_mismatch_refusal_precedes_malformed_row_decode() {
    let path = valid_path("version-before-row");
    let database = Database::create(&path).unwrap();
    let mut transaction = database.begin_write().unwrap();
    transaction.set_durability(Durability::Immediate).unwrap();
    {
        transaction
            .open_table(META)
            .unwrap()
            .insert("schema_version", u64::MAX)
            .unwrap();
    }
    {
        transaction
            .open_table(RECEIPTS)
            .unwrap()
            .insert(1, b"not-json".as_slice())
            .unwrap();
    }
    transaction.commit().unwrap();
    drop(database);

    let error = RedbWriteStore::open(path)
        .err()
        .expect("mismatched version refuses");
    assert!(
        error.to_string().contains("schema version mismatch"),
        "row decoding ran before version refusal: {error}"
    );
}

#[test]
fn row_mutation_accepts_attributed_empty_source_failure() {
    let path = unique_path("empty-source-failure");
    let keys = Keys::generate();
    let base = source(&keys, 10, "base");
    let store = RedbWriteStore::open(&path).unwrap();
    let accepted = accept(
        &store,
        edit(),
        keys.public_key(),
        revision(keys.public_key(), 11, "current"),
        Some(&EventValue::Signed(base.clone())),
    );
    store
        .record_revision_failure(
            accepted.write_id,
            accepted.receipt_id,
            RevisionId::FIRST,
            Some(base.id),
            None,
            "empty source failed".to_owned(),
        )
        .unwrap();
    drop(store);

    let reopened = RedbWriteStore::open(path).expect("attributed empty-source failure reopens");
    let recovered = reopened.recover_applied_edits().unwrap();
    assert!(
        recovered[0]
            .0
            .current
            .publication
            .revision_failure
            .as_deref()
            .is_some_and(|reason| reason.contains("source empty state failed"))
    );
    assert_eq!(recovered[0].2, keys.public_key());
    assert_eq!(recovered[0].4, None);
}

fn settle_no_destination(store: &RedbWriteStore, accepted: &fava_write_store::AcceptedWrite) {
    store
        .apply_route(
            accepted.write_id,
            accepted.receipt_id,
            accepted.current.publication.revision_id,
            accepted.current.id(),
            &RoutePlan {
                revision: 1,
                destinations: BTreeMap::new(),
                coverage: BTreeMap::new(),
                unresolved: BTreeSet::new(),
                shortfalls: Vec::new(),
            },
        )
        .unwrap();
}

fn assert_published_receipt(store: &RedbWriteStore, expected: &fava_write::LocalWriteEvent) {
    let opened = QuerySource::open(store, &Query::events().cache_only()).unwrap();
    assert_eq!(opened.initial.events.len(), 1);
    assert!(matches!(
        &opened.initial.events[0],
        SourceEvent::Local(current) if current == expected
    ));
}

fn valid_path(label: &str) -> std::path::PathBuf {
    let path = unique_path(label);
    let keys = Keys::generate();
    let store = RedbWriteStore::open(&path).unwrap();
    accept(
        &store,
        edit(),
        keys.public_key(),
        revision(keys.public_key(), 11, "current"),
        None,
    );
    drop(store);
    path
}

fn explicit_path(label: &str) -> (std::path::PathBuf, RelayUrl, RelayUrl) {
    let path = unique_path(label);
    let keys = Keys::generate();
    let first = RelayUrl::parse("wss://first.example").unwrap();
    let second = RelayUrl::parse("wss://second.example").unwrap();
    let store = RedbWriteStore::open(&path).unwrap();
    store
        .accept_applied_edit(
            WriteIntent::edit_as(
                edit(),
                keys.public_key(),
                WriteRouting::explicit([first.clone(), second.clone()]).unwrap(),
            )
            .unwrap(),
            revision(keys.public_key(), 10, "explicit"),
            None,
        )
        .unwrap();
    drop(store);
    (path, first, second)
}

fn assert_lane_mismatch(path: std::path::PathBuf, label: &str) {
    let error = RedbWriteStore::open(path)
        .err()
        .unwrap_or_else(|| panic!("{label} explicit lane was reconstructed"));
    assert!(
        error
            .to_string()
            .contains("durable explicit route disagrees with its destination lanes"),
        "unexpected {label}-lane refusal: {error}"
    );
}

fn mutate_typed_receipt(path: &std::path::Path, mutate: impl FnOnce(&mut Receipt)) {
    let mut receipt = read_receipt(path);
    mutate(&mut receipt);
    mutate_row(path, |row| {
        set(row, "/receipt", serde_json::to_value(receipt).unwrap());
    });
}

fn valid_source_path(label: &str) -> std::path::PathBuf {
    let path = unique_path(label);
    let keys = Keys::generate();
    let base = source(&keys, 10, "base");
    let store = RedbWriteStore::open(&path).unwrap();
    accept(
        &store,
        edit(),
        keys.public_key(),
        revision(keys.public_key(), 11, "current"),
        Some(&EventValue::Signed(base.clone())),
    );
    drop(store);
    path
}

fn terminal_no_destination_path(label: &str) -> std::path::PathBuf {
    let path = unique_path(label);
    let keys = Keys::generate();
    let store = RedbWriteStore::open(&path).unwrap();
    let accepted = accept(
        &store,
        edit(),
        keys.public_key(),
        revision(keys.public_key(), 10, "terminal"),
        None,
    );
    settle_no_destination(&store, &accepted);
    drop(store);
    path
}

fn assert_row_mutation_refused(label: &str, mutate: impl FnOnce(&mut Value)) {
    let path = valid_path(label);
    mutate_row(&path, mutate);
    assert!(
        RedbWriteStore::open(path).is_err(),
        "malformed {label} row was reconstructed"
    );
}

fn read_receipt(path: &std::path::Path) -> Receipt {
    RedbWriteStore::open(path)
        .unwrap()
        .receipt(fava_write::ReceiptId::try_from(1).expect("nonzero receipt identity"))
        .unwrap()
        .unwrap()
}

fn mutate_row(path: &std::path::Path, mutate: impl FnOnce(&mut Value)) {
    let database = Database::create(path).unwrap();
    let mut transaction = database.begin_write().unwrap();
    transaction.set_durability(Durability::Immediate).unwrap();
    {
        let mut table = transaction.open_table(RECEIPTS).unwrap();
        let bytes = table.get(1).unwrap().unwrap().value().to_vec();
        let mut row: Value = serde_json::from_slice(&bytes).unwrap();
        mutate(&mut row);
        let encoded = serde_json::to_vec(&row).unwrap();
        table.insert(1, encoded.as_slice()).unwrap();
    }
    transaction.commit().unwrap();
}

fn move_row_to_zero(path: &std::path::Path) {
    let database = Database::create(path).unwrap();
    let mut transaction = database.begin_write().unwrap();
    transaction.set_durability(Durability::Immediate).unwrap();
    {
        let mut table = transaction.open_table(RECEIPTS).unwrap();
        let bytes = table.remove(1).unwrap().unwrap().value().to_vec();
        let mut row: Value = serde_json::from_slice(&bytes).unwrap();
        for pointer in [
            "/receipt/receipt_id",
            "/receipt/write_id",
            "/receipt/current/publication/receipt_id",
            "/receipt/current/publication/write_id",
        ] {
            set(&mut row, pointer, json!(0));
        }
        let encoded = serde_json::to_vec(&row).unwrap();
        table.insert(0, encoded.as_slice()).unwrap();
    }
    transaction.commit().unwrap();
}

fn set_next_id(path: &std::path::Path, next_id: u64) {
    let database = Database::create(path).unwrap();
    let mut transaction = database.begin_write().unwrap();
    transaction.set_durability(Durability::Immediate).unwrap();
    transaction
        .open_table(META)
        .unwrap()
        .insert("next_id", next_id)
        .unwrap();
    transaction.commit().unwrap();
}

fn read_next_id(path: &std::path::Path) -> u64 {
    let database = Database::create(path).unwrap();
    database
        .begin_read()
        .unwrap()
        .open_table(META)
        .unwrap()
        .get("next_id")
        .unwrap()
        .unwrap()
        .value()
}

fn set(row: &mut Value, pointer: &str, value: Value) {
    *row.pointer_mut(pointer)
        .unwrap_or_else(|| panic!("persisted row lacks {pointer}")) = value;
}

/// A write resumes under the authority it was accepted with, read back from the
/// store rather than defaulted by whatever process reopened it.
///
/// This is the failure the authority exists to prevent: a write accepted over
/// one account's authenticated session, parked, and then resumed by a process
/// that has no selection, going out as public work.
#[test]
fn a_stored_write_keeps_its_authority_across_a_reopen() {
    let account = Keys::generate().public_key();
    let relay = RelayUrl::parse("wss://authenticated.example").unwrap();
    let path = unique_path("authority-across-reopen");

    let receipt_id = {
        let store = RedbWriteStore::open(&path).unwrap();
        let keys = Keys::generate();
        let intent = WriteIntent::event(
            revision(keys.public_key(), 1, "gm"),
            WriteRouting::Explicit(vec![relay.clone()]),
        )
        .unwrap()
        .under(Authority::As(account));
        store.accept(intent).unwrap().receipt_id
    };

    // A different process, with no account selected, reopens the store.
    let reopened = RedbWriteStore::open(&path).unwrap();
    let receipt = reopened
        .receipt(receipt_id)
        .unwrap()
        .expect("the write survives the reopen");

    assert_eq!(
        receipt.access,
        Authority::As(account),
        "the authority came from the store, not from the reopening process"
    );
}
