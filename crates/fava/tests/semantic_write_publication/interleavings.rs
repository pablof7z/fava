use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;
use std::sync::{Arc, Barrier};

use fava::{
    EventBuilder, EventValue, Kind, MaterializationId, RelayDeliveryOutcome, Timestamp,
    WriteRouting,
};
use fava_routing::RoutePlan;
use fava_state::{RelayAccess, RelaySessionKey};
use fava_write_store::{WriteStore, destination_evidence_capacity};
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::FinalizeEvent;
use nostr::key::Keys;

use super::support::{intent, relay_url};

fn materialization(keys: &Keys, created_at: u64, content: &str) -> fava::UnsignedEvent {
    EventBuilder::new(keys.public_key(), Kind::ContactList)
        .created_at(Timestamp::from(created_at))
        .content(content)
        .build()
        .expect("materialization builds")
}

fn accepted(store: &MemoryWriteStore, keys: &Keys) -> fava_write_store::AcceptedWrite {
    store
        .accept_materialized_edit(
            intent(keys.public_key(), Kind::ContactList),
            materialization(keys, 1, "generation one"),
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
        settled: false,
    }
}

#[test]
fn retired_completion_is_attributable_and_inert() {
    let keys = Keys::generate();
    let store = MemoryWriteStore::default();
    let accepted = accepted(&store, &keys);
    let generation_one = MaterializationId::from_u64(1);
    let event_one = accepted.current.id();
    let source = materialization(&keys, 2, "qualified source")
        .finalize(&keys)
        .expect("source signs");
    let generation_two = store
        .install_materialization(
            accepted.write_id,
            accepted.receipt_id,
            generation_one,
            None,
            materialization(&keys, 3, "generation two"),
            Some(&source),
        )
        .expect("successor installs");
    let session = RelaySessionKey::new(relay_url(), RelayAccess::public());
    let signed_one = materialization(&keys, 1, "generation one")
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
        current.current.publication.retired_materializations,
        vec![(generation_one, event_one, None, None)]
    );
}

#[test]
fn simultaneous_source_and_completion_converge_once() {
    let keys = Keys::generate();
    let store = Arc::new(MemoryWriteStore::default());
    let accepted = accepted(&store, &keys);
    let source = materialization(&keys, 2, "qualified source")
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
            store.install_materialization(
                accepted.write_id,
                accepted.receipt_id,
                MaterializationId::from_u64(1),
                None,
                materialization(&keys, 3, "generation two"),
                Some(&source),
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
                MaterializationId::from_u64(1),
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
        current.current.publication.materialization_id,
        MaterializationId::from_u64(2)
    );
    assert_eq!(
        current.current.publication.retired_materializations.len(),
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
    let signed_a = materialization(&keys_a, 1, "generation one")
        .finalize(&keys_a)
        .expect("event signs");
    let signed_b = materialization(&keys_b, 1, "generation one")
        .finalize(&keys_b)
        .expect("event signs");

    store.cancel(a.receipt_id).expect("A cancels");
    assert!(
        store
            .install_signed(
                a.write_id,
                a.receipt_id,
                MaterializationId::from_u64(1),
                a.current.id(),
                signed_a,
            )
            .is_err()
    );
    assert!(
        store
            .install_signed(
                b.write_id,
                b.receipt_id,
                MaterializationId::from_u64(1),
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
    let refusal = store.accept_materialized_edit(
        match intent(second.public_key(), Kind::ContactList)
            .into_parts()
            .0
        {
            fava::WritePayload::Edit { edit, author } => fava::WriteIntent::edit_as(
                edit,
                author,
                WriteRouting::Explicit(BTreeSet::from([relay_url()])),
            ),
            _ => unreachable!(),
        }
        .unwrap(),
        materialization(&second, 1, "refused"),
        None,
    );
    assert!(refusal.is_err());
    assert_eq!(store.recover_materialized_edits().unwrap().len(), 1);
}
