use fava_write::{WriteIntent, WriteRouting};
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::key::Keys;
use std::sync::{Arc, Barrier};

use super::{accept, edit, materialization, source};

#[test]
fn simultaneous_coordinate_admission_has_one_owner() {
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
                WriteIntent::edit_as(edit(), actor, WriteRouting::Automatic).unwrap(),
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
fn same_authorless_edit_has_independent_author_custody() {
    let alice = Keys::generate();
    let bob = Keys::generate();
    let store = MemoryWriteStore::default();

    let alice_write = accept(
        &store,
        edit(),
        alice.public_key(),
        materialization(alice.public_key(), 10, "alice"),
        None,
    );
    let bob_write = accept(
        &store,
        edit(),
        bob.public_key(),
        materialization(bob.public_key(), 10, "bob"),
        None,
    );

    assert_ne!(alice_write.receipt_id, bob_write.receipt_id);
    let recovered = store.recover_materialized_edits().unwrap();
    assert_eq!(recovered.len(), 2);
    assert_eq!(recovered[0].2, alice.public_key());
    assert_eq!(recovered[1].2, bob.public_key());
}

#[test]
fn materialization_or_source_outside_accepted_author_is_refused() {
    let alice = Keys::generate();
    let bob = Keys::generate();
    let store = MemoryWriteStore::default();
    let alice_intent =
        || WriteIntent::edit_as(edit(), alice.public_key(), WriteRouting::Automatic).unwrap();

    assert!(
        store
            .accept_materialized_edit(
                alice_intent(),
                materialization(bob.public_key(), 10, "wrong current author"),
                None,
            )
            .is_err()
    );
    let bob_source = source(&bob, 9, "wrong source author");
    assert!(
        store
            .accept_materialized_edit(
                alice_intent(),
                materialization(alice.public_key(), 10, "alice"),
                Some(&bob_source),
            )
            .is_err()
    );
    assert_eq!(store.len().unwrap(), 0);
}
