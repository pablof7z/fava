//! Exact relay-URL authority for neutral replaceable query results.

use std::collections::BTreeMap;

use fava_query::{Query, QueryError, QueryEvaluator, SourceEvent, SourceKind, SourceRevision};
use fava_query::{SourceSnapshot, SourceStatus};
use fava_query_standard::StandardQueryEvaluator;
use fava_state::{CachedEvent, RelayAccess, RelayEvidence, RelaySessionKey, RelayUrl, Timestamp};
use fava_write::{
    EventValue, LocalWriteEvent, MaterializationId, PublicationEvidence, ReceiptId, SignatureState,
    WriteId,
};
use nostr::event::{Event, EventBuilder, FinalizeEvent, Kind, Tag};
use nostr::key::Keys;

fn relay(url: &str) -> RelayUrl {
    RelayUrl::parse(url).expect("relay url")
}

fn evidence(urls: &[&str]) -> RelayEvidence {
    let mut evidence = RelayEvidence::default();
    for (index, url) in urls.iter().enumerate() {
        evidence.merge(&RelayEvidence::one(
            RelaySessionKey::new(relay(url), RelayAccess::public()),
            Timestamp::from(index as u64 + 1),
        ));
    }
    evidence
}

fn addressable(keys: &Keys, created_at: u64, content: &str) -> Event {
    EventBuilder::new(Kind::from_u16(30_001), content)
        .tags([Tag::identifier("same")])
        .custom_created_at(Timestamp::from(created_at))
        .finalize(keys)
        .expect("event signs")
}

fn cache(events: Vec<(Event, RelayEvidence)>) -> SourceSnapshot {
    SourceSnapshot {
        kind: SourceKind::EventCache,
        revision: SourceRevision(1),
        status: SourceStatus::Open,
        events: events
            .into_iter()
            .map(|(event, evidence)| SourceEvent::Cached(CachedEvent::new(event, evidence)))
            .collect(),
    }
}

fn local(event: Event) -> SourceSnapshot {
    let local = LocalWriteEvent::new(
        EventValue::Signed(event),
        PublicationEvidence {
            receipt_id: ReceiptId::from_u64(1),
            write_id: WriteId::from_u64(2),
            materialization_id: MaterializationId::from_u64(3),
            materialization_source: None,
            materialization_failure: None,
            retired_materializations: Vec::new(),
            signature: SignatureState::Signed,
            destinations: BTreeMap::new(),
        },
    )
    .expect("local event is valid");
    SourceSnapshot {
        kind: SourceKind::WriteStore,
        revision: SourceRevision(1),
        status: SourceStatus::Open,
        events: vec![SourceEvent::Local(local)],
    }
}

fn base_query() -> Query {
    Query::events().kind(Kind::from_u16(30_001)).tag_values(
        fava_query::SingleLetterTag::from_char('d').expect("tag key"),
        ["same"],
    )
}

#[test]
fn only_relays_unions_per_relay_replaceable_winners() {
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
        vec![winner_a.id, winner_b.id]
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
    assert_eq!(oldest.events[0].id(), winner_b.id);
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
    assert!(result.events[0].relay_evidence.is_empty());
    assert!(result.events[0].publication.is_some());
}
