use std::collections::BTreeSet;

use fava_relay::{RelayAccess, RelaySessionKey};
use fava_routing::RoutePlan;
use fava_write::{EventValue, MaterializationId, SignatureState, WriteIntent, WriteRouting};
use fava_write_store::WriteStore;
use fava_write_store_redb::RedbWriteStore;
use nostr::event::FinalizeEvent;
use nostr::key::Keys;
use nostr::types::RelayUrl;

use super::{edit, materialization, unique_path};

#[test]
#[allow(clippy::too_many_lines)] // One restart proof preserves every exact successor fact.
fn redb_authorized_generation_and_bounded_successor_survive_reopen() {
    let path = unique_path("authorized-successor");
    let keys = Keys::generate();
    let author = keys.public_key();
    let first_event = materialization(author, 1, "one");
    let initial_route = RoutePlan::explicit(
        [RelayUrl::parse("wss://authorized-predecessor.example").unwrap()],
        &RelayAccess::Public,
        &BTreeSet::new(),
    )
    .unwrap();
    let relay = RelayUrl::parse("wss://authorized-successor.example").unwrap();
    let successor_route =
        RoutePlan::explicit([relay.clone()], &RelayAccess::Public, &BTreeSet::new()).unwrap();
    let first = {
        let store = RedbWriteStore::open(&path).unwrap();
        let first_edit = edit();
        let first_reservation = store.reserve_active(&first_edit, author).unwrap();
        let first = store
            .accept_reserved_materialized_edit(
                first_reservation,
                WriteIntent::edit_as(first_edit, author, WriteRouting::Automatic).unwrap(),
                first_event.clone(),
                None,
                Some(&initial_route),
            )
            .unwrap();
        store
            .authorize_signing(
                first.write_id,
                first.receipt_id,
                MaterializationId::from_u64(1),
                first.current.id(),
            )
            .unwrap();
        let second =
            fava_write::ReplaceableEventEdit::new(fava_write::Kind::ContactList, None, vec![2])
                .unwrap();
        let reservation = store.reserve_active(&second, author).unwrap();
        store
            .accept_reserved_materialized_edit(
                reservation,
                WriteIntent::edit_as(second, author, WriteRouting::Automatic).unwrap(),
                materialization(author, 2, "one|two"),
                Some(&first.current.event),
                Some(&successor_route),
            )
            .unwrap();
        first
    };

    let reopened = RedbWriteStore::open(&path).unwrap();
    let recovered = reopened.receipt(first.receipt_id).unwrap().unwrap();
    assert_eq!(
        recovered.current.publication.materialization_id,
        MaterializationId::from_u64(2)
    );
    assert_eq!(
        recovered.current.publication.signature,
        SignatureState::Unsigned
    );
    assert_eq!(
        recovered.route_revision,
        initial_route.revision.saturating_add(1)
    );
    assert!(recovered.destinations().contains_key(&RelaySessionKey {
        relay,
        access: RelayAccess::Public,
    }));
    assert!(
        reopened
            .authorize_signing(
                first.write_id,
                first.receipt_id,
                MaterializationId::from_u64(1),
                first.current.id(),
            )
            .is_err()
    );
    let EventValue::Unsigned(successor_event) = recovered.current.event.clone() else {
        panic!("recovered successor is unsigned")
    };
    reopened
        .authorize_signing(
            first.write_id,
            first.receipt_id,
            MaterializationId::from_u64(2),
            recovered.current.id(),
        )
        .unwrap();
    let signed = reopened
        .install_signed(
            first.write_id,
            first.receipt_id,
            MaterializationId::from_u64(2),
            recovered.current.id(),
            successor_event.finalize(&keys).unwrap(),
        )
        .unwrap();
    assert_eq!(
        signed.current.publication.materialization_id,
        MaterializationId::from_u64(2)
    );
    assert_eq!(signed.current.publication.signature, SignatureState::Signed);
    let EventValue::Signed(current) = signed.current.event else {
        panic!("successor is signed")
    };
    assert_eq!(current.content, "one|two");
}

#[test]
fn redb_authorized_generation_without_successor_reopens_as_exact_retryable_work() {
    let path = unique_path("authorized-retryable");
    let keys = Keys::generate();
    let author = keys.public_key();
    let accepted = {
        let store = RedbWriteStore::open(&path).unwrap();
        let accepted = store
            .accept_materialized_edit(
                WriteIntent::edit_as(edit(), author, WriteRouting::Automatic).unwrap(),
                materialization(author, 1, "one"),
                None,
            )
            .unwrap();
        store
            .authorize_signing(
                accepted.write_id,
                accepted.receipt_id,
                MaterializationId::from_u64(1),
                accepted.current.id(),
            )
            .unwrap();
        accepted
    };

    let reopened = RedbWriteStore::open(&path).unwrap();
    let recovered = reopened.receipt(accepted.receipt_id).unwrap().unwrap();
    assert_eq!(recovered.current.id(), accepted.current.id());
    assert_eq!(
        recovered.current.publication.materialization_id,
        MaterializationId::from_u64(1)
    );
    let SignatureState::Retryable(reason) = recovered.current.publication.signature else {
        panic!("ambiguous authorization remains attributable retryable work")
    };
    assert!(reason.contains(&accepted.write_id.as_u64().to_string()));
    assert!(reason.contains(&accepted.receipt_id.as_u64().to_string()));
    assert!(reason.contains(&accepted.current.id().to_string()));
    assert!(reason.contains("retry is permitted"));
}

#[test]
fn redb_authorized_cancellation_without_successor_survives_reopen_exactly() {
    let path = unique_path("authorized-cancelled-retryable");
    let keys = Keys::generate();
    let author = keys.public_key();
    let accepted = {
        let store = RedbWriteStore::open(&path).unwrap();
        let accepted = store
            .accept_materialized_edit(
                WriteIntent::edit_as(edit(), author, WriteRouting::Automatic).unwrap(),
                materialization(author, 1, "one"),
                None,
            )
            .unwrap();
        store
            .authorize_signing(
                accepted.write_id,
                accepted.receipt_id,
                MaterializationId::from_u64(1),
                accepted.current.id(),
            )
            .unwrap();
        store
            .record_signer_retryable(
                accepted.write_id,
                accepted.receipt_id,
                MaterializationId::from_u64(1),
                accepted.current.id(),
                "authorized signer invocation cancelled before effect; retry is permitted"
                    .to_owned(),
            )
            .unwrap();
        accepted
    };

    let reopened = RedbWriteStore::open(&path).unwrap();
    let recovered = reopened.receipt(accepted.receipt_id).unwrap().unwrap();
    assert_eq!(recovered.write_id, accepted.write_id);
    assert_eq!(recovered.receipt_id, accepted.receipt_id);
    assert_eq!(recovered.current.id(), accepted.current.id());
    assert_eq!(
        recovered.current.publication.materialization_id,
        MaterializationId::from_u64(1)
    );
    assert!(matches!(
        recovered.current.publication.signature,
        SignatureState::Retryable(reason)
            if reason.contains("cancelled") && reason.contains("retry is permitted")
    ));
}
