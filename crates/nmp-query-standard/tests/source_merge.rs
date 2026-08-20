//! Component evidence for deterministic local source merge semantics.

use nmp_query::{
    EventQuery, QueryEvaluator, SourceEvent, SourceKind, SourceRevision, SourceSnapshot,
    SourceStatus,
};
use nmp_query_standard::StandardQueryEvaluator;
use nmp_state::{AccessContext, CachedEvent, RelayEvidence, RelaySessionKey, RelayUrl, Timestamp};
use nmp_write::{
    EventValue, LocalWriteEvent, PublicationEvidence, ReceiptId, SignatureState, WriteId,
};
use nostr::event::{
    Event, EventBuilder, FinalizeEvent, FinalizeUnsignedEvent, Kind, UnsignedEvent,
};
use nostr::key::Keys;

fn signed_event(keys: &Keys, kind: Kind, created_at: u64, content: &str) -> Event {
    EventBuilder::new(kind, content)
        .custom_created_at(Timestamp::from(created_at))
        .finalize(keys)
        .expect("test event signs")
}

fn unsigned_event(keys: &Keys, kind: Kind, created_at: u64, content: &str) -> UnsignedEvent {
    let mut event = EventBuilder::new(kind, content)
        .custom_created_at(Timestamp::from(created_at))
        .finalize_unsigned(keys.public_key());
    event.ensure_id();
    event
}

fn relay_evidence(urls: &[&str]) -> RelayEvidence {
    let mut evidence = RelayEvidence::default();
    for (index, url) in urls.iter().enumerate() {
        evidence.merge(&RelayEvidence::one(
            RelaySessionKey::new(
                RelayUrl::parse(url).expect("test relay url"),
                AccessContext::public(),
            ),
            Timestamp::from(index as u64 + 1),
        ));
    }
    evidence
}

fn publication() -> PublicationEvidence {
    PublicationEvidence {
        receipt_id: ReceiptId::from_u64(7),
        write_id: WriteId::from_u64(11),
        signature: SignatureState::Signed,
    }
}

fn snapshot(kind: SourceKind, events: Vec<SourceEvent>) -> SourceSnapshot {
    SourceSnapshot {
        kind,
        revision: SourceRevision(1),
        status: SourceStatus::Open,
        events,
    }
}

#[test]
fn same_signed_event_merges_relay_and_publication_evidence() {
    let keys = Keys::generate();
    let event = signed_event(&keys, Kind::TextNote, 10, "hello");
    let cached = CachedEvent::new(
        event.clone(),
        relay_evidence(&["wss://relay-a.example", "wss://relay-b.example"]),
    );
    let local = LocalWriteEvent::new(EventValue::Signed(event), publication())
        .expect("signed event is valid local state");
    let sources = [
        snapshot(SourceKind::EventCache, vec![SourceEvent::Cached(cached)]),
        snapshot(SourceKind::WriteStore, vec![SourceEvent::Local(local)]),
    ];
    let query = EventQuery::events().canonicalize().expect("valid query");

    let result = StandardQueryEvaluator
        .evaluate(&query, &sources)
        .expect("evaluation succeeds");

    assert_eq!(result.events.len(), 1);
    assert_eq!(result.events[0].relay_evidence.len(), 2);
    assert_eq!(result.events[0].publication, Some(publication()));
}

#[test]
fn local_replacement_overlays_then_reveals_cached_predecessor() {
    let keys = Keys::generate();
    let predecessor = signed_event(&keys, Kind::ContactList, 10, "predecessor");
    let successor = unsigned_event(&keys, Kind::ContactList, 20, "successor");
    let successor_id = successor.id.expect("builder computes id");
    let local = LocalWriteEvent::new(
        EventValue::Unsigned(successor),
        PublicationEvidence {
            signature: SignatureState::Unsigned,
            ..publication()
        },
    )
    .expect("unsigned event is finalized");
    let cache = snapshot(
        SourceKind::EventCache,
        vec![SourceEvent::Cached(CachedEvent::new(
            predecessor.clone(),
            relay_evidence(&["wss://relay.example"]),
        ))],
    );
    let writes = snapshot(SourceKind::WriteStore, vec![SourceEvent::Local(local)]);
    let query = EventQuery::events()
        .kind(Kind::ContactList)
        .canonicalize()
        .expect("valid query");

    let overlaid = StandardQueryEvaluator
        .evaluate(&query, &[cache.clone(), writes])
        .expect("evaluation succeeds");
    assert_eq!(overlaid.events.len(), 1);
    assert_eq!(overlaid.events[0].id(), successor_id);

    let after_cancel = StandardQueryEvaluator
        .evaluate(&query, &[cache])
        .expect("evaluation succeeds");
    assert_eq!(after_cancel.events.len(), 1);
    assert_eq!(after_cancel.events[0].id(), predecessor.id);
}

#[test]
fn asking_relays_and_trusting_only_relays_are_distinct() {
    let keys = Keys::generate();
    let event = signed_event(&keys, Kind::TextNote, 10, "source authority");
    let asked = RelayUrl::parse("wss://asked.example").expect("relay url");
    let other = "wss://other.example";
    let cached_other = CachedEvent::new(event.clone(), relay_evidence(&[other]));
    let sources = [snapshot(
        SourceKind::EventCache,
        vec![SourceEvent::Cached(cached_other)],
    )];
    let acquisition_only = EventQuery::events()
        .from_relays([asked.clone()])
        .expect("non-empty relay set")
        .canonicalize()
        .expect("valid query");
    let provenance_constrained = EventQuery::events()
        .only_from_relays([asked.clone()])
        .expect("non-empty relay set")
        .canonicalize()
        .expect("valid query");

    let visible = StandardQueryEvaluator
        .evaluate(&acquisition_only, &sources)
        .expect("evaluation succeeds");
    let hidden = StandardQueryEvaluator
        .evaluate(&provenance_constrained, &sources)
        .expect("evaluation succeeds");
    assert_eq!(visible.events.len(), 1);
    assert!(hidden.events.is_empty());

    let qualified = CachedEvent::new(event, relay_evidence(&[other, "wss://asked.example"]));
    let qualified_sources = [snapshot(
        SourceKind::EventCache,
        vec![SourceEvent::Cached(qualified)],
    )];
    let now_visible = StandardQueryEvaluator
        .evaluate(&provenance_constrained, &qualified_sources)
        .expect("evaluation succeeds");
    assert_eq!(now_visible.events.len(), 1);
}
