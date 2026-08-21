//! Durable semantic-write parity and schema-hardening evidence.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::time::{SystemTime, UNIX_EPOCH};

use fava_routing::RoutePlan;
use fava_state::EventCoordinate;
use fava_write::{
    Event, EventBuilder, Kind, MaterializationId, ReplaceableEventEdit, Timestamp, UnsignedEvent,
    WriteIntent, WriteRouting,
};
use fava_write_store::{WriteStore, destination_evidence_capacity};
use fava_write_store_redb::RedbWriteStore;
use nostr::event::FinalizeEvent;
use nostr::key::Keys;
use redb::{Database, Durability, TableDefinition};

const META: TableDefinition<&str, u64> = TableDefinition::new("meta");
const RECEIPTS: TableDefinition<u64, &[u8]> = TableDefinition::new("receipts");

fn edit(actor: fava_write::PublicKey) -> ReplaceableEventEdit {
    ReplaceableEventEdit::new(
        actor,
        EventCoordinate::Replaceable {
            author: actor,
            kind: Kind::ContactList,
            identifier: None,
        },
        7,
        vec![1],
        vec![0],
    )
    .expect("bounded edit")
}

fn materialization(actor: fava_write::PublicKey, created_at: u64, body: &str) -> UnsignedEvent {
    EventBuilder::new(actor, Kind::ContactList)
        .created_at(Timestamp::from(created_at))
        .content(body)
        .build()
        .expect("valid materialization")
}

fn source(keys: &Keys, created_at: u64, body: &str) -> Event {
    materialization(keys.public_key(), created_at, body)
        .finalize(keys)
        .expect("valid signed source")
}

fn accept(
    store: &RedbWriteStore,
    edit: ReplaceableEventEdit,
    event: UnsignedEvent,
    source: Option<&Event>,
) -> fava_write_store::AcceptedWrite {
    store
        .accept_materialized_edit(
            WriteIntent::edit(edit, WriteRouting::Automatic).expect("valid edit intent"),
            event,
            source,
        )
        .expect("semantic write accepted")
}

#[test]
fn redb_coordinate_admission_is_single_owner() {
    let path = unique_path("coordinate");
    let keys = Keys::generate();
    let actor = keys.public_key();
    let store = Arc::new(RedbWriteStore::open(&path).expect("redb opens"));
    let mut changes = store.receipt_changes();
    let barrier = Arc::new(Barrier::new(3));
    let mut threads = Vec::new();
    for _ in 0..2 {
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            store.accept_materialized_edit(
                WriteIntent::edit(edit(actor), WriteRouting::Automatic).unwrap(),
                materialization(actor, 10, "one owner"),
                None,
            )
        }));
    }
    barrier.wait();
    let first = threads.remove(0).join().unwrap().unwrap();
    let second = threads.remove(0).join().unwrap().unwrap();
    assert_eq!(first, second);
    assert_eq!(store.len().unwrap(), 1);
    assert_eq!(store.recover_materialized_edits().unwrap().len(), 1);
    assert_eq!(changes.try_recv().unwrap().0, first.receipt_id);
    assert!(changes.try_recv().is_err(), "duplicate acceptance notified");

    drop(changes);
    drop(store);
    let reopened = RedbWriteStore::open(&path).expect("redb reopens");
    let recovered = reopened.recover_materialized_edits().unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].0.receipt_id, first.receipt_id);
    assert_eq!(recovered[0].1, edit(actor));
}

#[test]
fn redb_generation_and_failure_state_match_memory() {
    let path = unique_path("generation");
    let keys = Keys::generate();
    let base = source(&keys, 10, "base");
    let failed_source = source(&keys, 20, "failed source");
    let store = RedbWriteStore::open(&path).unwrap();
    let accepted = accept(
        &store,
        edit(keys.public_key()),
        materialization(keys.public_key(), 11, "generation one"),
        Some(&base),
    );
    let failed = store
        .record_materialization_failure(
            accepted.write_id,
            accepted.receipt_id,
            MaterializationId::from_u64(1),
            Some(base.id),
            Some(&failed_source),
            "first attempt failed".to_owned(),
        )
        .expect("failure commits");
    assert_eq!(failed.current.id(), accepted.current.id());
    drop(store);

    let reopened = RedbWriteStore::open(&path).expect("redb reopens");
    let recovered = reopened.recover_materialized_edits().unwrap();
    assert_eq!(recovered[0].0, failed);
    assert_eq!(recovered[0].2, Some(base.id));
    assert_eq!(recovered[0].3, Some(failed_source.id));
    let successor = reopened
        .install_materialization(
            accepted.write_id,
            accepted.receipt_id,
            MaterializationId::from_u64(1),
            Some(base.id),
            materialization(keys.public_key(), 21, "generation two"),
            Some(&failed_source),
        )
        .expect("retry installs");
    assert_eq!(successor.write_id, accepted.write_id);
    assert_eq!(successor.receipt_id, accepted.receipt_id);
    assert_eq!(
        successor.current.publication.materialization_id,
        MaterializationId::from_u64(2)
    );
    assert_eq!(successor.current.publication.materialization_failure, None);
    assert_eq!(
        successor.current.publication.retired_materializations[0].0,
        MaterializationId::from_u64(1)
    );
    assert!(
        successor.current.publication.retired_materializations[0]
            .3
            .as_deref()
            .is_some_and(|reason| reason.contains("first attempt failed"))
    );
    assert_eq!(reopened.recover_materialized_edits().unwrap()[0].3, None);
}

#[test]
fn redb_stale_and_overflow_mutations_are_atomic_noops() {
    assert_eq!(destination_evidence_capacity(), 256);
    let path = unique_path("atomic-noops");
    let keys = Keys::generate();
    let store = RedbWriteStore::open_bounded(
        &path,
        NonZeroUsize::new(1).unwrap(),
        NonZeroUsize::new(2).unwrap(),
    )
    .unwrap();
    assert_eq!(store.active_capacity(), 1);
    let accepted = accept(
        &store,
        edit(keys.public_key()),
        materialization(keys.public_key(), 1, "generation zero"),
        None,
    );
    let other = Keys::generate();
    assert!(
        store
            .accept_materialized_edit(
                WriteIntent::edit(edit(other.public_key()), WriteRouting::Automatic).unwrap(),
                materialization(other.public_key(), 1, "overflow"),
                None,
            )
            .is_err()
    );
    let before_stale = store.receipt(accepted.receipt_id).unwrap().unwrap();
    let mut changes = store.receipt_changes();
    let stale_source = source(&keys, 2, "stale source");
    assert!(
        store
            .install_materialization(
                accepted.write_id,
                accepted.receipt_id,
                MaterializationId::from_u64(9),
                None,
                materialization(keys.public_key(), 3, "stale"),
                Some(&stale_source),
            )
            .is_err()
    );
    assert_eq!(
        store.receipt(accepted.receipt_id).unwrap(),
        Some(before_stale)
    );
    assert!(changes.try_recv().is_err(), "refusal notified");

    let cancelled = store.cancel(accepted.receipt_id).unwrap().unwrap();
    assert!(cancelled.is_terminal());
    let replacement = accept(
        &store,
        edit(keys.public_key()),
        materialization(keys.public_key(), 10, "replacement owner"),
        None,
    );
    let terminal = store
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
                settled: true,
            },
        )
        .expect("empty route settles");
    assert!(terminal.is_terminal());
    assert!(store.recover_materialized_edits().unwrap().is_empty());
    let before_late = store.receipt(replacement.receipt_id).unwrap();
    assert!(
        store
            .record_materialization_failure(
                replacement.write_id,
                replacement.receipt_id,
                MaterializationId::from_u64(1),
                None,
                Some(&stale_source),
                "late".to_owned(),
            )
            .is_err()
    );
    assert_eq!(store.receipt(replacement.receipt_id).unwrap(), before_late);
}

#[test]
fn redb_schema_mismatch_refuses_without_fallback() {
    let mismatch_path = unique_path("schema-mismatch");
    drop(RedbWriteStore::open(&mismatch_path).expect("new schema is stamped"));
    let database = Database::create(&mismatch_path).expect("raw database opens");
    let mut transaction = database.begin_write().unwrap();
    transaction.set_durability(Durability::Immediate).unwrap();
    transaction
        .open_table(META)
        .unwrap()
        .insert("schema_version", u64::MAX)
        .unwrap();
    transaction.commit().unwrap();
    drop(database);
    assert!(RedbWriteStore::open(&mismatch_path).is_err());

    let missing_path = unique_path("schema-missing");
    let database = Database::create(&missing_path).expect("old database opens");
    let mut transaction = database.begin_write().unwrap();
    transaction.set_durability(Durability::Immediate).unwrap();
    transaction.open_table(RECEIPTS).unwrap();
    transaction
        .open_table(META)
        .unwrap()
        .insert("next_id", 1_u64)
        .unwrap();
    transaction.commit().unwrap();
    drop(database);
    assert!(RedbWriteStore::open(&missing_path).is_err());
}

fn unique_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "fava-redb-semantic-{}-{label}-{nonce}.redb",
        std::process::id()
    ))
}
