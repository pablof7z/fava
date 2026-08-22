//! Public receipt-summary and write-settlement evidence.

use std::collections::{BTreeMap, BTreeSet};

use fava::{Kind, Receipt, RelayDeliveryOutcome};
use fava_state::{RelayAccess, RelaySessionKey, RelayUrl};
use fava_write::{
    EventValue, LocalWriteEvent, MaterializationId, PublicationEvidence, ReceiptId,
    ReceiptOutcome, SignatureState, WriteId, WriteRouting,
};
use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
use nostr::key::Keys;

#[test]
fn receipt_counts_preserve_complete_mixed_destination_evidence() {
    let receipt = mixed_receipt();

    assert_eq!(receipt.acknowledged(), 1);
    assert_eq!(receipt.rejected(), 1);
    assert_eq!(receipt.desired(), 3);
    assert_eq!(receipt.destinations().len(), 4);
    assert!(receipt.destinations().values().any(|outcome| matches!(
        outcome,
        RelayDeliveryOutcome::Unknown { reason } if reason == "handoff ambiguous"
    )));
    assert!(receipt.destinations().values().any(|outcome| matches!(
        outcome,
        RelayDeliveryOutcome::Pending
    )));
}

fn mixed_receipt() -> Receipt {
    let keys = Keys::generate();
    let event = NostrEventBuilder::new(Kind::TextNote, "mixed receipt")
        .finalize(&keys)
        .expect("event signs");
    let acknowledged = session("acknowledged");
    let rejected = session("rejected");
    let unknown = session("unknown");
    let withdrawn = session("withdrawn-pending");
    let destinations = BTreeMap::from([
        (
            acknowledged.clone(),
            RelayDeliveryOutcome::Acknowledged {
                message: "stored exactly".to_owned(),
            },
        ),
        (
            rejected.clone(),
            RelayDeliveryOutcome::Rejected {
                message: "blocked exactly".to_owned(),
            },
        ),
        (
            unknown.clone(),
            RelayDeliveryOutcome::Unknown {
                reason: "handoff ambiguous".to_owned(),
            },
        ),
        (withdrawn, RelayDeliveryOutcome::Pending),
    ]);
    let write_id = WriteId::from_u64(41);
    let receipt_id = ReceiptId::from_u64(41);
    let current = LocalWriteEvent::new(
        EventValue::Signed(event),
        PublicationEvidence {
            receipt_id,
            write_id,
            materialization_id: MaterializationId::from_u64(1),
            materialization_source: None,
            materialization_failure: None,
            retired_materializations: Vec::new(),
            signature: SignatureState::Signed,
            destinations,
        },
    )
    .expect("signed event is query-visible");

    Receipt {
        write_id,
        receipt_id,
        current,
        routing: WriteRouting::Explicit(vec![
            acknowledged.relay.clone(),
            rejected.relay.clone(),
            unknown.relay.clone(),
        ]),
        outcome: ReceiptOutcome::Complete,
        route_revision: 1,
        route_settled: true,
        route_shortfalls: Vec::new(),
        desired_destinations: BTreeSet::from([acknowledged, rejected, unknown]),
        attempts: BTreeMap::new(),
    }
}

fn session(name: &str) -> RelaySessionKey {
    RelaySessionKey::new(
        RelayUrl::parse(&format!("wss://{name}.example")).expect("relay URL"),
        RelayAccess::public(),
    )
}
