use fava_write::{
    EventBuilder, EventValue, Kind, MaterializationId, PublicKey, ReplaceableEventEdit,
    SignatureState, Timestamp, UnsignedEvent, WriteIntent, WriteRouting,
};
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::FinalizeEvent;
use nostr::key::Keys;

fn edit(change: u8) -> ReplaceableEventEdit {
    ReplaceableEventEdit::new(Kind::ContactList, None, vec![change]).unwrap()
}

fn intent(author: PublicKey, change: u8) -> WriteIntent {
    WriteIntent::edit_as(edit(change), author, WriteRouting::Automatic).unwrap()
}

fn event(author: PublicKey, created_at: u64, content: &str) -> UnsignedEvent {
    EventBuilder::new(author, Kind::ContactList)
        .created_at(Timestamp::from(created_at))
        .content(content)
        .build()
        .unwrap()
}

#[test]
fn memory_reservation_wins_before_signing_authorization() {
    let keys = Keys::generate();
    let store = MemoryWriteStore::default();
    let first = store
        .accept_materialized_edit(
            intent(keys.public_key(), 1),
            event(keys.public_key(), 1, "one"),
            None,
        )
        .unwrap();
    let reservation = store.reserve_active(&edit(2), keys.public_key()).unwrap();

    let deferred = store
        .authorize_signing(
            first.write_id,
            first.receipt_id,
            MaterializationId::FIRST,
            first.current.id(),
        )
        .unwrap();
    assert!(matches!(
        deferred.current.publication.signature,
        SignatureState::Retryable(_)
    ));

    let composed = store
        .accept_reserved_materialized_edit(
            reservation,
            intent(keys.public_key(), 2),
            event(keys.public_key(), 2, "one|two"),
            Some(&first.current.event),
            None,
        )
        .unwrap();
    assert_eq!(composed.write_id, first.write_id);
    assert_eq!(composed.receipt_id, first.receipt_id);
    assert_eq!(
        composed.current.publication.materialization_id,
        MaterializationId::try_from(2).expect("nonzero materialization identity")
    );
    assert!(
        store
            .authorize_signing(
                first.write_id,
                first.receipt_id,
                MaterializationId::FIRST,
                first.current.id()
            )
            .is_err()
    );
}

#[test]
fn memory_authorization_wins_and_holds_one_bounded_successor() {
    let keys = Keys::generate();
    let store = MemoryWriteStore::default();
    let first_event = event(keys.public_key(), 1, "one");
    let first = store
        .accept_materialized_edit(intent(keys.public_key(), 1), first_event.clone(), None)
        .unwrap();
    let authorized = store
        .authorize_signing(
            first.write_id,
            first.receipt_id,
            MaterializationId::FIRST,
            first.current.id(),
        )
        .unwrap();
    assert_eq!(
        authorized.current.publication.signature,
        SignatureState::Authorized
    );

    let reservation = store.reserve_active(&edit(2), keys.public_key()).unwrap();
    let accepted = store
        .accept_reserved_materialized_edit(
            reservation,
            intent(keys.public_key(), 2),
            event(keys.public_key(), 2, "one|two"),
            Some(&first.current.event),
            None,
        )
        .unwrap();
    assert_eq!(accepted.write_id, first.write_id);
    assert_eq!(
        store.receipt(first.receipt_id).unwrap().unwrap(),
        authorized
    );
    assert!(store.reserve_active(&edit(3), keys.public_key()).is_err());

    let signed = first_event.finalize(&keys).unwrap();
    let successor = store
        .install_signed(
            first.write_id,
            first.receipt_id,
            MaterializationId::FIRST,
            first.current.id(),
            signed,
        )
        .unwrap();
    assert_eq!(
        successor.current.publication.materialization_id,
        MaterializationId::try_from(2).expect("nonzero materialization identity")
    );
    assert_eq!(
        successor.current.publication.signature,
        SignatureState::Unsigned
    );
    let EventValue::Unsigned(current) = successor.current.event else {
        panic!("successor is unsigned")
    };
    assert_eq!(current.content, "one|two");
}

#[test]
fn memory_authorized_cancellation_without_successor_is_exact_retryable_work() {
    let keys = Keys::generate();
    let store = MemoryWriteStore::default();
    let accepted = store
        .accept_materialized_edit(
            intent(keys.public_key(), 1),
            event(keys.public_key(), 1, "one"),
            None,
        )
        .unwrap();
    store
        .authorize_signing(
            accepted.write_id,
            accepted.receipt_id,
            MaterializationId::FIRST,
            accepted.current.id(),
        )
        .unwrap();

    let cancelled = store
        .record_signer_retryable(
            accepted.write_id,
            accepted.receipt_id,
            MaterializationId::FIRST,
            accepted.current.id(),
            "authorized signer invocation cancelled before effect; retry is permitted".to_owned(),
        )
        .unwrap();

    assert_eq!(cancelled.write_id, accepted.write_id);
    assert_eq!(cancelled.receipt_id, accepted.receipt_id);
    assert_eq!(cancelled.current.id(), accepted.current.id());
    assert_eq!(
        cancelled.current.publication.materialization_id,
        MaterializationId::FIRST
    );
    assert!(matches!(
        cancelled.current.publication.signature,
        SignatureState::Retryable(reason)
            if reason.contains("cancelled") && reason.contains("retry is permitted")
    ));
}
