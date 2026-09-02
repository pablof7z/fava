//! Reference-hint and relay-evidence routing behavior.

use std::collections::BTreeSet;
use std::sync::Arc;

use fava_query::{EventRecord, Freshness, QuerySnapshot};
use fava_relay::Authority;
use fava_router_hints::HintRouter;
use fava_routing::{RoutePlan, RouteRequest, Router};
use fava_state::{RelayEvent, relay_occurrences_for_event};
use fava_write::{Event, EventBuilder, EventValue, Kind, Tag};
use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
use nostr::key::Keys;
use nostr::types::{RelayUrl, Timestamp};

#[test]
fn the_events_a_request_references_are_asked_of_the_cache() {
    let target = signed_target();
    let account = Keys::generate().public_key();
    let request = reply_to(&target, &relay("hinted"), Authority::As(account));

    let queries = HintRouter::new("hints")
        .queries(&request, &RoutePlan::default())
        .unwrap();

    assert_eq!(queries.len(), 1);
    assert_eq!(
        queries[0].selection().ids,
        Some(BTreeSet::from([target.id]))
    );
    assert_eq!(queries[0].freshness(), Freshness::CacheOnly);
    assert_eq!(queries[0].access(), &Authority::As(account));
}

#[test]
fn reference_hint_and_actual_relay_evidence_are_independent_reasons() {
    let target = signed_target();
    let observed = relay("observed");
    let hinted = relay("hinted");
    let router = HintRouter::new("hints");

    let contribution = router
        .preview(
            &reply_to(&target, &hinted, Authority::Unauthenticated),
            &RoutePlan::default(),
            &[seen_at(&target, &observed)],
        )
        .unwrap();

    assert_eq!(
        destinations(&contribution),
        BTreeSet::from([hinted, observed])
    );
}

#[test]
fn replacing_the_snapshot_replaces_the_relays_it_justified() {
    let target = signed_target();
    let hinted = relay("hinted");
    let first = relay("first");
    let second = relay("second");
    let mut session = HintRouter::new("hints")
        .open(
            reply_to(&target, &hinted, Authority::Unauthenticated),
            Arc::new(RoutePlan::default()),
            vec![seen_at(&target, &first)],
        )
        .unwrap();
    assert_eq!(
        destinations(&session.current()),
        BTreeSet::from([hinted.clone(), first])
    );

    let replaced = session
        .replace(
            Arc::new(RoutePlan::default()),
            vec![seen_at(&target, &second)],
        )
        .unwrap();

    assert_eq!(
        destinations(&replaced),
        BTreeSet::from([hinted, second.clone()])
    );
    assert_eq!(destinations(&session.current()), destinations(&replaced));
}

fn signed_target() -> Event {
    NostrEventBuilder::new(Kind::TextNote, "target")
        .finalize(&Keys::generate())
        .unwrap()
}

fn reply_to(target: &Event, hint: &RelayUrl, access: Authority) -> RouteRequest {
    let reply = EventBuilder::new(Kind::TextNote)
        .tag(Tag::parse(["e", &target.id.to_hex(), hint.as_str(), "reply"]).expect("e tag"))
        .by(Keys::generate().public_key())
        .build()
        .unwrap();
    RouteRequest::Write {
        event: EventValue::Unsigned(reply),
        access,
    }
}

fn seen_at(target: &Event, session: &RelayUrl) -> QuerySnapshot {
    let record = EventRecord::new(
        EventValue::Signed(target.clone()),
        relay_occurrences_for_event(
            target.id,
            &[RelayEvent::new(
                target.clone(),
                session.clone(),
                Authority::Unauthenticated,
                Timestamp::from(10),
            )],
        )
        .unwrap(),
        None,
    )
    .unwrap();
    QuerySnapshot::evaluated(vec![record], &[])
}

fn destinations(contribution: &fava_routing::RouteContribution) -> BTreeSet<RelayUrl> {
    RoutePlan::from_contribution(1, contribution)
        .unwrap()
        .destinations
        .keys()
        .cloned()
        .collect()
}

fn relay(name: &str) -> RelayUrl {
    RelayUrl::parse(&format!("wss://{name}.example")).unwrap()
}
