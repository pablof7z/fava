//! Local writes remain access-neutral and compete once with qualifying relays.

use fava_query::{Query, QueryEvaluator, SourceEvent, SourceKind, SourceRevision, SourceSnapshot};
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_state::RelayEvent;
use fava_write::{
    EventValue, LocalWriteEvent, MaterializationId, PublicationEvidence, ReceiptId, SignatureState,
    WriteId,
};
use nostr::event::{Event, EventBuilder, FinalizeEvent, Kind};
use nostr::key::Keys;
use nostr::types::{RelayUrl, Timestamp};
use std::collections::BTreeMap;

fn local(event: Event) -> Result<LocalWriteEvent, fava_write::InvalidEventValue> {
    LocalWriteEvent::new(
        EventValue::Signed(event),
        PublicationEvidence {
            receipt_id: ReceiptId::from_u64(1),
            write_id: WriteId::from_u64(1),
            materialization_id: MaterializationId::from_u64(1),
            materialization_source: None,
            materialization_failure: None,
            retired_materializations: Vec::new(),
            signature: SignatureState::Signed,
            destinations: BTreeMap::new(),
        },
    )
}

#[test]
fn wrong_access_same_id_cannot_leak_occurrence_through_local_record()
-> Result<(), Box<dyn std::error::Error>> {
    let author = Keys::generate();
    let alice = Keys::generate();
    let relay = RelayUrl::parse("wss://relay.example")?;
    let signed = EventBuilder::new(Kind::Metadata, "shared")
        .custom_created_at(Timestamp::from(20))
        .finalize(&author)?;
    let public_key = RelaySessionKey {
        relay,
        access: RelayAccess::Public,
    };
    let relay_source = SourceSnapshot::current(
        SourceKind::EventCache,
        SourceRevision(1),
        vec![SourceEvent::Relay(RelayEvent::new(
            signed.clone(),
            public_key,
            Timestamp::from(1),
        ))],
    );
    let write_source = SourceSnapshot::current(
        SourceKind::WriteStore,
        SourceRevision(1),
        vec![SourceEvent::Local(local(signed.clone())?)],
    );
    let query = Query::events().with_relay_access(RelayAccess::Authenticated(alice.public_key()));
    let result = StandardQueryEvaluator.evaluate(&query, &[relay_source, write_source])?;
    assert_eq!(result.events.len(), 1);
    assert_eq!(result.events[0].id(), signed.id);
    assert!(result.events[0].publication().is_some());
    assert!(result.events[0].relay_occurrences().is_empty());
    Ok(())
}
