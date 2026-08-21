//! Public contract evidence for volatile semantic-write custody.

use std::sync::{Arc, Barrier};

use fava_state::EventCoordinate;
use fava_write::{
    Event, EventBuilder, Kind, MaterializationId, ReplaceableEventEdit, Timestamp,
    UnsignedEvent, WriteIntent, WriteRouting,
};
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::FinalizeEvent;
use nostr::key::Keys;

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

fn materialization(
    actor: fava_write::PublicKey,
    created_at: u64,
    content: &str,
) -> UnsignedEvent {
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
fn memory_first_edit_has_no_prior() {
    let keys = Keys::generate();
    let store = MemoryWriteStore::default();
    let accepted = accept(
        &store,
        edit(keys.public_key()),
        materialization(keys.public_key(), 10, "first"),
        None,
    );

    let receipt = store
        .receipt(accepted.receipt_id)
        .expect("store readable")
        .expect("receipt retained");
    assert_eq!(receipt.write_id, accepted.write_id);
    assert_eq!(receipt.receipt_id, accepted.receipt_id);
    assert_eq!(receipt.current.publication.materialization_id, MaterializationId::from_u64(1));
    assert_eq!(receipt.current.publication.materialization_source, None);
    assert!(receipt.current.publication.retired_materializations.is_empty());
    assert_eq!(store.recover_materialized_edits().unwrap().len(), 1);
}

#[test]
fn memory_simultaneous_coordinate_admission_has_one_owner() {
    let keys = Keys::generate();
    let actor = keys.public_key();
    let store = Arc::new(MemoryWriteStore::default());
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
                materialization(actor, 10, "one effect"),
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
    assert!(matches!(
        changes.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
}

#[test]
fn memory_generation_swap_is_compare_and_set() {
    let keys = Keys::generate();
    let store = MemoryWriteStore::default();
    let base = source(&keys, 10, "base");
    let accepted = accept(
        &store,
        edit(keys.public_key()),
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
                Some(base.id),
                materialization(keys.public_key(), 31, "stale swap"),
                Some(&later_source),
            )
            .is_err()
    );
    assert_eq!(store.receipt(accepted.receipt_id).unwrap(), Some(before_stale));
}

#[test]
fn memory_unqualified_source_is_inert() {
    let keys = Keys::generate();
    let store = MemoryWriteStore::default();
    let selected = source(&keys, 20, "selected");
    let accepted = accept(
        &store,
        edit(keys.public_key()),
        materialization(keys.public_key(), 21, "current"),
        Some(&selected),
    );
    let mut changes = store.receipt_changes();
    let before = store.receipt(accepted.receipt_id).unwrap().unwrap();

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
    assert_eq!(store.receipt(accepted.receipt_id).unwrap(), Some(unchanged));
    assert_eq!(changes.try_recv().unwrap().0, accepted.receipt_id);
    assert!(matches!(
        changes.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
    assert_ne!(before, unchanged);
}
