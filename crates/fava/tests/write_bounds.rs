//! Boundary evidence for accepted writes and receipt facts.

use std::collections::{BTreeMap, BTreeSet};

use fava::{EventBuilder, WriteIntent, WriteRouting};
use fava_routing::{RouteContribution, RouteDestination, RoutePlan, RouteRequest};
use fava_state::{RelayAccess, RelaySessionKey, RelayUrl};
use fava_write::{
    EventValue, Kind, ReceiptOutcome, RelayDeliveryOutcome, SignatureState, WriteIntentError,
};
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::FinalizeEvent;
use nostr::key::Keys;

#[tokio::test(flavor = "current_thread")]
async fn slow_receipt_consumer_gets_explicit_lag_instead_of_silent_loss() {
    let keys = Keys::generate();
    let store = MemoryWriteStore::default();
    let mut changes = store.receipt_changes();
    for sequence in 0..257 {
        let event = EventBuilder::new(keys.public_key(), Kind::TextNote)
            .content(format!("receipt change {sequence}"))
            .build()
            .unwrap();
        store
            .accept(WriteIntent::event(event, WriteRouting::Automatic).unwrap())
            .unwrap();
    }
    assert_eq!(
        changes.recv().await,
        Err(tokio::sync::broadcast::error::RecvError::Lagged(1))
    );
    assert_eq!(changes.recv().await.unwrap().0.as_u64(), 2);
}

#[test]
fn explicit_write_relay_fanout_is_bounded_before_custody() {
    let keys = Keys::generate();
    let event = EventBuilder::new(keys.public_key(), Kind::TextNote)
        .build()
        .unwrap();
    let relays = (0..257)
        .map(|index| RelayUrl::parse(&format!("wss://relay-{index}.example")).unwrap())
        .collect();
    assert_eq!(
        WriteIntent::event(event, WriteRouting::Explicit(relays)),
        Err(WriteIntentError::TooManyExplicitRelays {
            actual: 257,
            maximum: 256,
        })
    );
}

#[test]
fn receipt_text_and_signed_body_are_checked_at_the_store_boundary() {
    let keys = Keys::generate();
    let relay = RelayUrl::parse("wss://bounded.example").unwrap();
    let unsigned = EventBuilder::new(keys.public_key(), Kind::TextNote)
        .content("accepted body")
        .build()
        .unwrap();
    let store = MemoryWriteStore::default();
    let accepted = store
        .accept(
            WriteIntent::event(unsigned, WriteRouting::Explicit(BTreeSet::from([relay]))).unwrap(),
        )
        .unwrap();
    assert!(
        store
            .record_signer_refusal(accepted.receipt_id, "x".repeat(4_097))
            .is_err()
    );
    let wrong = EventBuilder::new(keys.public_key(), Kind::TextNote)
        .content("different body")
        .build()
        .unwrap()
        .finalize(&keys)
        .unwrap();
    assert!(store.install_signed(accepted.receipt_id, wrong).is_err());
    let mut receipt = store.receipt(accepted.receipt_id).unwrap().unwrap();
    assert!(matches!(receipt.current.event, EventValue::Unsigned(_)));
    assert_eq!(
        receipt.current.publication.signature,
        SignatureState::Unsigned
    );

    let EventValue::Unsigned(unsigned) = receipt.current.event.clone() else {
        unreachable!("receipt remains unsigned");
    };
    receipt = store
        .install_signed(accepted.receipt_id, unsigned.finalize(&keys).unwrap())
        .unwrap();
    let session = receipt.destinations().keys().next().unwrap().clone();
    store.begin_attempt(accepted.receipt_id, &session).unwrap();
    assert!(
        store
            .record_outcome(
                accepted.receipt_id,
                &session,
                RelayDeliveryOutcome::Acknowledged {
                    message: "x".repeat(4_097),
                },
            )
            .is_err()
    );
}

#[test]
fn automatic_route_fanout_is_bounded_before_receipt_mutation() {
    let keys = Keys::generate();
    let event = EventBuilder::new(keys.public_key(), Kind::TextNote)
        .build()
        .unwrap();
    let store = MemoryWriteStore::default();
    let accepted = store
        .accept(WriteIntent::event(event, WriteRouting::Automatic).unwrap())
        .unwrap();
    let destinations = (0..257)
        .map(|index| {
            RouteDestination::new(
                RelaySessionKey::new(
                    RelayUrl::parse(&format!("wss://automatic-{index}.example")).unwrap(),
                    RelayAccess::public(),
                ),
                BTreeSet::new(),
                "bounded route",
            )
        })
        .collect();
    let plan = RoutePlan::from_contribution(
        1,
        &RouteContribution {
            destinations,
            coverage: BTreeMap::new(),
            unresolved: BTreeSet::new(),
            shortfalls: Vec::new(),
        },
    )
    .unwrap();

    let error = store.apply_route(accepted.receipt_id, &plan).unwrap_err();
    assert_eq!(
        error.to_string(),
        "write store refused operation: route destination fan-out exceeds bound: 257 > 256"
    );
    assert_eq!(
        store
            .receipt(accepted.receipt_id)
            .unwrap()
            .unwrap()
            .route_revision,
        0
    );
}

#[test]
fn automatic_route_shortfall_bound_is_atomic() {
    let keys = Keys::generate();
    let event = EventBuilder::new(keys.public_key(), Kind::TextNote)
        .build()
        .unwrap();
    let store = MemoryWriteStore::default();
    let accepted = store
        .accept(WriteIntent::event(event, WriteRouting::Automatic).unwrap())
        .unwrap();
    let destination = RouteDestination::new(
        RelaySessionKey::new(
            RelayUrl::parse("wss://atomic-route.example").unwrap(),
            RelayAccess::public(),
        ),
        BTreeSet::new(),
        "initial route",
    );
    let first = RoutePlan::from_contribution(
        1,
        &RouteContribution {
            destinations: vec![destination],
            coverage: BTreeMap::new(),
            unresolved: BTreeSet::new(),
            shortfalls: Vec::new(),
        },
    )
    .unwrap();
    let before = store.apply_route(accepted.receipt_id, &first).unwrap();
    let refused = RoutePlan {
        revision: 2,
        destinations: BTreeMap::new(),
        coverage: BTreeMap::new(),
        unresolved: BTreeSet::new(),
        shortfalls: vec!["x".repeat(4_097)],
        settled: true,
    };

    assert!(store.apply_route(accepted.receipt_id, &refused).is_err());
    assert_eq!(store.receipt(accepted.receipt_id).unwrap(), Some(before));
}

#[test]
fn withdrawn_in_flight_lane_stays_open_until_its_outcome_is_recorded() {
    let keys = Keys::generate();
    let event = EventBuilder::new(keys.public_key(), Kind::TextNote)
        .build()
        .unwrap()
        .finalize(&keys)
        .unwrap();
    let store = MemoryWriteStore::default();
    let accepted = store
        .accept(WriteIntent::presigned(event, WriteRouting::Automatic).unwrap())
        .unwrap();
    let session = RelaySessionKey::new(
        RelayUrl::parse("wss://withdrawn-in-flight.example").unwrap(),
        RelayAccess::public(),
    );
    let first = RoutePlan::from_contribution(
        1,
        &RouteContribution {
            destinations: vec![RouteDestination::new(
                session.clone(),
                BTreeSet::new(),
                "initial route",
            )],
            coverage: BTreeMap::new(),
            unresolved: BTreeSet::new(),
            shortfalls: Vec::new(),
        },
    )
    .unwrap();
    store.apply_route(accepted.receipt_id, &first).unwrap();
    store.begin_attempt(accepted.receipt_id, &session).unwrap();
    let withdrawn = RoutePlan {
        revision: 2,
        destinations: BTreeMap::new(),
        coverage: BTreeMap::new(),
        unresolved: BTreeSet::new(),
        shortfalls: Vec::new(),
        settled: true,
    };

    let receipt = store.apply_route(accepted.receipt_id, &withdrawn).unwrap();
    assert_eq!(receipt.outcome, ReceiptOutcome::Open);
    assert!(matches!(
        receipt.destinations().get(&session),
        Some(RelayDeliveryOutcome::Attempting)
    ));
    let receipt = store
        .record_outcome(
            accepted.receipt_id,
            &session,
            RelayDeliveryOutcome::Acknowledged {
                message: "saved historical handoff".to_owned(),
            },
        )
        .unwrap();
    assert_eq!(receipt.outcome, ReceiptOutcome::NoDestination);
}

#[test]
fn settled_empty_automatic_route_has_typed_outcome_and_reason() {
    let keys = Keys::generate();
    let event = EventBuilder::new(keys.public_key(), Kind::TextNote)
        .build()
        .unwrap();
    let request = RouteRequest::write(
        EventValue::Unsigned(event.clone()),
        fava_state::RelayAccess::public(),
    );
    let plan = fava_routing::preview(&[], &request).unwrap();
    let store = MemoryWriteStore::default();
    let accepted = store
        .accept(WriteIntent::event(event, WriteRouting::Automatic).unwrap())
        .unwrap();
    let receipt = store.apply_route(accepted.receipt_id, &plan).unwrap();

    assert_eq!(receipt.outcome, ReceiptOutcome::NoDestination);
    assert!(receipt.route_settled);
    assert!(!receipt.route_shortfalls.is_empty());
}
