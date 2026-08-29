//! Exact relay-URL authority for neutral replaceable query results.

use std::collections::BTreeMap;

use fava_query::{Query, QueryError, QueryEvaluator, SourceEvent, SourceKind, SourceRevision};
use fava_query::{SourceSnapshot, SourceStatus};
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_state::RelayEvent;
use fava_write::{
    EventValue, LocalWriteEvent, RevisionId, PublicationEvidence, ReceiptId, SignatureState,
    WriteId,
};
use nostr::event::{
    Event, EventBuilder, FinalizeEvent, FinalizeUnsignedEvent, Kind, Tag, UnsignedEvent,
};
use nostr::key::Keys;
use nostr::types::{RelayUrl, Timestamp};

fn relay(url: &str) -> RelayUrl {
    RelayUrl::parse(url).expect("relay url")
}

fn evidence(urls: &[&str]) -> Vec<(RelaySessionKey, Timestamp)> {
    urls.iter()
        .enumerate()
        .map(|(index, url)| {
            (
                RelaySessionKey {
                    relay: relay(url),
                    access: RelayAccess::Public,
                },
                Timestamp::from(index as u64 + 1),
            )
        })
        .collect()
}

fn addressable(keys: &Keys, created_at: u64, content: &str) -> Event {
    EventBuilder::new(Kind::from_u16(30_001), content)
        .tags([Tag::identifier("same")])
        .custom_created_at(Timestamp::from(created_at))
        .finalize(keys)
        .expect("event signs")
}

fn addressable_unsigned(keys: &Keys, created_at: u64, content: &str) -> UnsignedEvent {
    let mut event = EventBuilder::new(Kind::from_u16(30_001), content)
        .tags([Tag::identifier("same")])
        .custom_created_at(Timestamp::from(created_at))
        .finalize_unsigned(keys.public_key());
    event.ensure_id();
    event
}

fn cache(events: Vec<(Event, Vec<(RelaySessionKey, Timestamp)>)>) -> SourceSnapshot {
    SourceSnapshot {
        kind: SourceKind::EventCache,
        revision: SourceRevision(1),
        status: SourceStatus::Open,
        retractions: Vec::new(),
        events: events
            .into_iter()
            .flat_map(|(event, evidence)| {
                evidence.into_iter().map(move |(session, observed_at)| {
                    SourceEvent::Relay(RelayEvent::new(event.clone(), session, observed_at))
                })
            })
            .collect(),
    }
}

fn local(event: Event) -> SourceSnapshot {
    let local = LocalWriteEvent::new(
        EventValue::Signed(event),
        PublicationEvidence {
            receipt_id: ReceiptId::try_from(1).expect("nonzero receipt identity"),
            write_id: WriteId::try_from(2).expect("nonzero write identity"),
            revision_id: RevisionId::try_from(3)
                .expect("nonzero revision identity"),
            revision_source: None,
            revision_failure: None,
            retired_revisions: Vec::new(),
            signature: SignatureState::Signed,
            destinations: BTreeMap::new(),
        },
    )
    .expect("local event is valid");
    SourceSnapshot {
        kind: SourceKind::WriteStore,
        revision: SourceRevision(1),
        status: SourceStatus::Open,
        retractions: Vec::new(),
        events: vec![SourceEvent::Local(local)],
    }
}

fn local_unsigned(event: UnsignedEvent) -> SourceSnapshot {
    let local = LocalWriteEvent::new(
        EventValue::Unsigned(event),
        PublicationEvidence {
            receipt_id: ReceiptId::try_from(1).expect("nonzero receipt identity"),
            write_id: WriteId::try_from(2).expect("nonzero write identity"),
            revision_id: RevisionId::try_from(3)
                .expect("nonzero revision identity"),
            revision_source: None,
            revision_failure: None,
            retired_revisions: Vec::new(),
            signature: SignatureState::Unsigned,
            destinations: BTreeMap::new(),
        },
    )
    .expect("local event is valid");
    SourceSnapshot {
        kind: SourceKind::WriteStore,
        revision: SourceRevision(1),
        status: SourceStatus::Open,
        retractions: Vec::new(),
        events: vec![SourceEvent::Local(local)],
    }
}

fn base_query() -> Query {
    Query::events()
        .kinds([Kind::from_u16(30_001)])
        .expect("one kind is bounded")
        .tag_values(
            fava_query::SingleLetterTag::from_char('d').expect("tag key"),
            ["same"],
        )
        .expect("one tag value is bounded")
}

#[test]
fn only_relays_selects_one_cross_source_replaceable_winner() {
    let keys = Keys::generate();
    let relay_a = "wss://relay-a.example";
    let relay_b = "wss://relay-b.example";
    let old_a = addressable(&keys, 10, "old A");
    let winner_b = addressable(&keys, 20, "winner B");
    let winner_a = addressable(&keys, 30, "winner A");
    let sources = [cache(vec![
        (old_a, evidence(&[relay_a])),
        (winner_b.clone(), evidence(&[relay_b])),
        (winner_a.clone(), evidence(&[relay_a])),
    ])];
    let query = base_query()
        .only_from_relays([relay(relay_a), relay(relay_b)])
        .expect("selected relays are non-empty");

    let result = StandardQueryEvaluator
        .evaluate(&query, &sources)
        .expect("evaluation succeeds");
    assert_eq!(
        result
            .events
            .iter()
            .map(fava_query::EventRecord::id)
            .collect::<Vec<_>>(),
        vec![winner_a.id]
    );

    let newest = StandardQueryEvaluator
        .evaluate(&query.clone().limit(1).expect("positive limit"), &sources)
        .expect("limited evaluation succeeds");
    let oldest = StandardQueryEvaluator
        .evaluate(
            &query.oldest_first().limit(1).expect("positive limit"),
            &sources,
        )
        .expect("limited oldest-first evaluation succeeds");
    assert_eq!(newest.events[0].id(), winner_a.id);
    assert_eq!(oldest.events[0].id(), winner_a.id);
}

#[test]
fn single_relay_selects_only_its_winner() {
    let keys = Keys::generate();
    let relay_a = "wss://relay-a.example";
    let relay_b = "wss://relay-b.example";
    let winner_b = addressable(&keys, 20, "winner B");
    let winner_a = addressable(&keys, 30, "winner A");
    let sources = [cache(vec![
        (winner_b.clone(), evidence(&[relay_b])),
        (winner_a.clone(), evidence(&[relay_a])),
    ])];

    let from_a = StandardQueryEvaluator
        .evaluate(
            &base_query()
                .only_from_relays([relay(relay_a)])
                .expect("relay A is selected"),
            &sources,
        )
        .expect("A evaluation succeeds");
    let from_b = StandardQueryEvaluator
        .evaluate(
            &base_query()
                .only_from_relays([relay(relay_b)])
                .expect("relay B is selected"),
            &sources,
        )
        .expect("B evaluation succeeds");
    let absent = StandardQueryEvaluator
        .evaluate(
            &base_query()
                .only_from_relays([relay("wss://relay-c.example")])
                .expect("relay C is selected"),
            &sources,
        )
        .expect("empty evidence is a valid empty result");

    assert_eq!(from_a.events.len(), 1);
    assert_eq!(from_a.events[0].id(), winner_a.id);
    assert_eq!(from_b.events.len(), 1);
    assert_eq!(from_b.events[0].id(), winner_b.id);
    assert!(absent.events.is_empty());
    assert_eq!(
        base_query().only_from_relays(Vec::<RelayUrl>::new()),
        Err(QueryError::EmptyExplicitRelays)
    );
}

#[test]
fn any_local_keeps_global_replaceable_semantics() {
    let keys = Keys::generate();
    let cached_a = addressable(&keys, 20, "cached A");
    let cached_b = addressable(&keys, 30, "cached B");
    let accepted_local = addressable(&keys, 40, "accepted local");
    let sources = [
        cache(vec![
            (cached_a, evidence(&["wss://relay-a.example"])),
            (cached_b, evidence(&["wss://relay-b.example"])),
        ]),
        local(accepted_local.clone()),
    ];

    let result = StandardQueryEvaluator
        .evaluate(&base_query(), &sources)
        .expect("AnyLocal evaluation succeeds");

    assert_eq!(result.events.len(), 1);
    assert_eq!(result.events[0].id(), accepted_local.id);
    assert!(result.events[0].relay_occurrences().is_empty());
    assert!(result.events[0].publication().is_some());
}

/// `docs/spec/partial-spec-api-semantics.md:200` — "An unpublished local event with no
/// qualifying relay provenance MUST NOT appear." — with `:214`: two otherwise identical
/// queries using different source modes "MUST NOT accidentally share evidence or
/// local-result visibility in a way that changes either query's results". A
/// write-store-only successor therefore has no standing in `only_from_relays`
/// coordinate-winner selection: it may not appear, and it may not displace the
/// relay-qualified event the query is actually asking for.
#[test]
fn unpublished_local_event_cannot_hide_a_relay_qualified_predecessor() {
    let keys = Keys::generate();
    let relay_a = "wss://relay-a.example";
    let predecessor = addressable(&keys, 10, "relay-served predecessor");
    let successor = addressable_unsigned(&keys, 20, "purely local successor");
    let query = base_query()
        .only_from_relays([relay(relay_a)])
        .expect("relay A is selected");

    let relay_only = StandardQueryEvaluator
        .evaluate(
            &query,
            &[cache(vec![(predecessor.clone(), evidence(&[relay_a]))])],
        )
        .expect("relay-only evaluation succeeds");
    assert_eq!(
        relay_only
            .events
            .iter()
            .map(fava_query::EventRecord::id)
            .collect::<Vec<_>>(),
        vec![predecessor.id]
    );

    let with_local_write = StandardQueryEvaluator
        .evaluate(
            &query,
            &[
                cache(vec![(predecessor.clone(), evidence(&[relay_a]))]),
                local_unsigned(successor),
            ],
        )
        .expect("evaluation with a local write succeeds");
    assert_eq!(
        with_local_write
            .events
            .iter()
            .map(fava_query::EventRecord::id)
            .collect::<Vec<_>>(),
        vec![predecessor.id],
        "a purely local unpublished write must neither appear nor erase the \
         relay-qualified event at the same coordinate"
    );
}

/// `docs/spec/partial-spec-api-semantics.md:202` — "If a locally published event later
/// acquires qualifying provenance because one of the specified relays serves it, it may
/// then enter the query result."
#[test]
fn relay_served_event_enters_without_local_evidence_under_only_relays() {
    let keys = Keys::generate();
    let relay_a = "wss://relay-a.example";
    let predecessor = addressable(&keys, 10, "relay-served predecessor");
    let successor = addressable(&keys, 20, "published successor");
    let query = base_query()
        .only_from_relays([relay(relay_a)])
        .expect("relay A is selected");

    let served = StandardQueryEvaluator
        .evaluate(
            &query,
            &[
                cache(vec![
                    (predecessor, evidence(&[relay_a])),
                    (successor.clone(), evidence(&[relay_a])),
                ]),
                local(successor.clone()),
            ],
        )
        .expect("evaluation succeeds");
    assert_eq!(
        served
            .events
            .iter()
            .map(fava_query::EventRecord::id)
            .collect::<Vec<_>>(),
        vec![successor.id]
    );
    assert!(served.events[0].publication().is_none());
}
