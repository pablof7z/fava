use std::num::NonZeroUsize;

use fava_write::{EventBuilder, Kind, MaterializationId, Timestamp, WriteIntent, WriteRouting};
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
