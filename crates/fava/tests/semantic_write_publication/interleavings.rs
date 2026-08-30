use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;
use std::sync::{Arc, Barrier};

use fava::{
    EventBuilder, EventValue, Kind, RevisionId, RelayDeliveryOutcome, EventEdit,
    Timestamp,
};
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_routing::RoutePlan;
use fava_write::{WriteIntent, WritePayload, WriteRouting};
use fava_write_store::{WriteStore, destination_evidence_capacity};
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::FinalizeEvent;
use nostr::key::Keys;

use super::support::{intent, relay_url};

fn revision(keys: &Keys, created_at: u64, content: &str) -> fava::UnsignedEvent {
    EventBuilder::new(Kind::ContactList)
        .created_at(Timestamp::from(created_at))
        .content(content)
        .by(keys.public_key())
        .build()
        .expect("revision builds")
}

fn edit() -> EventEdit {
    EventEdit::new(Kind::ContactList, None, vec![1]).expect("bounded edit")
}

fn accepted(store: &MemoryWriteStore, keys: &Keys) -> fava_write_store::AcceptedWrite {
    store
        .accept_applied_edit(
            intent(keys.public_key(), Kind::ContactList),
            revision(keys, 1, "generation one"),
            None,
        )
        .expect("first generation accepts")
}

fn automatic_route(revision: u64) -> RoutePlan {
    RoutePlan {
        revision,
        destinations: BTreeMap::new(),
        coverage: BTreeMap::new(),
        unresolved: BTreeSet::new(),
        shortfalls: vec!["held generation-one route".to_owned()],
    }
}

fn public_session() -> RelaySessionKey {
    RelaySessionKey {
        relay: relay_url(),
        access: RelayAccess::Public,
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn retired_completion_is_attributable_and_inert() {
    let keys = Keys::generate();
    let store = MemoryWriteStore::default();
    let accepted = accepted(&store, &keys);
    let generation_one = RevisionId::FIRST;
    let event_one = accepted.current.id();
    let source = revision(&keys, 2, "qualified source")
        .finalize(&keys)
        .expect("source signs");
    let generation_two = store
        .install_revision(
            accepted.write_id,
            accepted.receipt_id,
            generation_one,
            None,
            std::slice::from_ref(&edit()),
            revision(&keys, 3, "generation two"),
            Some(&EventValue::Signed(source.clone())),
            None,
        )
        .expect("successor installs");
    let session = public_session();
    let signed_one = revision(&keys, 1, "generation one")
        .finalize(&keys)
        .expect("retired event signs");

    assert!(
        store
            .install_signed(
                accepted.write_id,
                accepted.receipt_id,
                generation_one,
                event_one,
                signed_one,
            )
            .is_err()
    );
    assert!(
        store
            .record_signer_refusal(
                accepted.write_id,
                accepted.receipt_id,
                generation_one,
                event_one,
                "late signer refusal".to_owned(),
            )
            .is_err()
    );
    assert!(
        store
            .record_signer_refusal(
                accepted.write_id,
                accepted.receipt_id,
                generation_one,
                generation_two.current.id(),
                "retired generation with current event identity".to_owned(),
            )
            .is_err()
    );
    assert!(
        store
            .apply_route(
                accepted.write_id,
                accepted.receipt_id,
                generation_one,
                event_one,
                &automatic_route(1),
            )
            .is_err()
    );
    assert!(
        store
            .begin_attempt(
                accepted.write_id,
                accepted.receipt_id,
                generation_one,
                event_one,
                &session,
                1,
            )
            .is_err()
    );
    assert!(
        store
            .record_outcome(
                accepted.write_id,
                accepted.receipt_id,
                generation_one,
                event_one,
                &session,
                1,
                RelayDeliveryOutcome::Acknowledged {
                    message: "late OK".to_owned()
                },
            )
            .is_err()
    );

    let current = store.receipt(accepted.receipt_id).unwrap().unwrap();
    assert_eq!(current, generation_two);
    assert_eq!(
        current.current.publication.retired_revisions,
        vec![(generation_one, event_one, None, None)]
    );
}

#[test]
fn simultaneous_source_and_completion_converge_once() {
    let keys = Keys::generate();
    let store = Arc::new(MemoryWriteStore::default());
    let accepted = accepted(&store, &keys);
    let source = revision(&keys, 2, "qualified source")
        .finalize(&keys)
        .expect("source signs");
    let barrier = Arc::new(Barrier::new(3));

    let install = {
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        let source = source.clone();
        let keys = keys.clone();
        std::thread::spawn(move || {
            barrier.wait();
            store.install_revision(
                accepted.write_id,
                accepted.receipt_id,
                RevisionId::FIRST,
                None,
                std::slice::from_ref(&edit()),
                revision(&keys, 3, "generation two"),
                Some(&EventValue::Signed(source.clone())),
                None,
            )
        })
    };
    let completion = {
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        std::thread::spawn(move || {
            barrier.wait();
            store.record_signer_refusal(
                accepted.write_id,
                accepted.receipt_id,
                RevisionId::FIRST,
                accepted.current.id(),
                "simultaneous refusal".to_owned(),
            )
        })
    };
    barrier.wait();
    assert!(install.join().unwrap().is_ok());
    let _ = completion.join().unwrap();

    let current = store.receipt(accepted.receipt_id).unwrap().unwrap();
    assert_eq!(
        current.current.publication.revision_id,
        RevisionId::try_from(2).expect("nonzero revision identity")
    );
    assert_eq!(
        current.current.publication.retired_revisions.len(),
        1
    );
}

#[test]
fn semantic_cancellation_is_scoped_and_late_work_is_inert() {
    let keys_a = Keys::generate();
    let keys_b = Keys::generate();
    let store = MemoryWriteStore::default();
    let a = accepted(&store, &keys_a);
    let b = accepted(&store, &keys_b);
    let signed_a = revision(&keys_a, 1, "generation one")
        .finalize(&keys_a)
        .expect("event signs");
    let signed_b = revision(&keys_b, 1, "generation one")
        .finalize(&keys_b)
        .expect("event signs");

    store.cancel(a.receipt_id).expect("A cancels");
    assert!(
        store
            .install_signed(
                a.write_id,
                a.receipt_id,
                RevisionId::FIRST,
                a.current.id(),
                signed_a,
            )
            .is_err()
    );
    store
        .authorize_signing(
            b.write_id,
            b.receipt_id,
            RevisionId::FIRST,
            b.current.id(),
        )
        .expect("B signing authorizes independently");
    assert!(
        store
            .install_signed(
                b.write_id,
                b.receipt_id,
                RevisionId::FIRST,
                b.current.id(),
                signed_b,
            )
            .is_ok()
    );
    assert!(matches!(
        store.receipt(b.receipt_id).unwrap().unwrap().current.event,
        EventValue::Signed(_)
    ));
}

#[test]
fn semantic_task_and_completion_bounds_refuse_cleanly() {
    assert_eq!(destination_evidence_capacity(), 256);
    let store = MemoryWriteStore::bounded(NonZeroUsize::new(1).unwrap());
    let first = Keys::generate();
    let second = Keys::generate();
    accepted(&store, &first);
    let refusal = store.accept_applied_edit(
        match intent(second.public_key(), Kind::ContactList)
            .into_parts()
            .0
        {
            WritePayload::Edit { edit, author } => {
                WriteIntent::edit_as(edit, author, WriteRouting::Explicit(vec![relay_url()]))
            }
            _ => unreachable!(),
        }
        .unwrap(),
        revision(&second, 1, "refused"),
        None,
    );
    assert!(refusal.is_err());
    assert_eq!(store.recover_applied_edits().unwrap().len(), 1);
}

#[test]
fn active_reservation_excludes_unreserved_memory_admission() {
    let store = MemoryWriteStore::bounded(NonZeroUsize::new(1).unwrap());
    let semantic_keys = Keys::generate();
    let raw_keys = Keys::generate();
    let reservation = store
        .reserve_active(
            &fava::EventEdit::new(Kind::ContactList, None, vec![1]).unwrap(),
            semantic_keys.public_key(),
        )
        .expect("semantic slot reserves");
    let raw = WriteIntent::event(
        EventBuilder::new(Kind::TextNote)
            .created_at(Timestamp::from(1))
            .content("unreserved")
            .by(raw_keys.public_key())
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
        .accept_reserved_applied_edit(
            reservation,
            intent(semantic_keys.public_key(), Kind::ContactList),
            revision(&semantic_keys, 1, "reserved"),
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
}

#[test]
fn equal_timestamp_lower_id_is_memory_store_successor() {
    let keys = Keys::generate();
    let store = MemoryWriteStore::default();
    let left = revision(&keys, 10, "left").finalize(&keys).unwrap();
    let right = revision(&keys, 10, "right").finalize(&keys).unwrap();
    let (higher_id, lower_id) = if left.id > right.id {
        (left, right)
    } else {
        (right, left)
    };
    let accepted = store
        .accept_applied_edit(
            intent(keys.public_key(), Kind::ContactList),
            revision(&keys, 11, "higher-id generation"),
            Some(&EventValue::Signed(higher_id.clone())),
        )
        .unwrap();

    let installed = store
        .install_revision(
            accepted.write_id,
            accepted.receipt_id,
            RevisionId::FIRST,
            Some(higher_id.id),
            std::slice::from_ref(&edit()),
            revision(&keys, 12, "lower-id generation"),
            Some(&EventValue::Signed(lower_id.clone())),
            None,
        )
        .expect("equal-time lower event id is authoritative");
    assert_eq!(
        installed.current.publication.revision_source,
        Some(lower_id.id)
    );
    assert!(
        store
            .install_revision(
                accepted.write_id,
                accepted.receipt_id,
                RevisionId::try_from(2).expect("nonzero revision identity"),
                Some(lower_id.id),
                std::slice::from_ref(&edit()),
                revision(&keys, 13, "higher-id retry"),
                Some(&EventValue::Signed(higher_id.clone())),
                None,
            )
            .is_err(),
        "equal-time higher event id cannot displace the winner"
    );
}
