//! Boundary evidence for accepted writes and receipt facts.

use std::collections::BTreeSet;

use fava::{EventBuilder, WriteIntent, WriteRouting};
use fava_state::RelayUrl;
use fava_write::{EventValue, Kind, RelayDeliveryOutcome, SignatureState, WriteIntentError};
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
