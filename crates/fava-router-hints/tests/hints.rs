//! Reference-hint and relay-evidence routing behavior.

use std::collections::BTreeSet;

use fava_query::EventRecord;
use fava_relay::Authority;
use fava_router_hints::HintRouter;
use fava_routing::{RoutePlan, RouteRequest, Router};
use fava_state::{RelayEvent, relay_occurrences_for_event};
use fava_write::{EventBuilder, EventValue, Kind, Tag};
use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
use nostr::key::Keys;
use nostr::types::{RelayUrl, Timestamp};

#[test]
fn reference_hint_and_actual_relay_evidence_are_independent_reasons() {
    let author = Keys::generate();
    let target = NostrEventBuilder::new(Kind::TextNote, "target")
        .finalize(&Keys::generate())
        .unwrap();
    let observed = relay("observed");
    let hinted = relay("hinted");
    let record = EventRecord::new(
        EventValue::Signed(target.clone()),
        relay_occurrences_for_event(
            target.id,
            &[RelayEvent::new(
                target.clone(),
                observed.clone(),
                Authority::Unauthenticated,
                Timestamp::from(10),
            )],
        )
        .unwrap(),
        None,
    )
    .unwrap();
    let router = HintRouter::new("hints");
    router.remember(&record);
    let reply = EventBuilder::new(Kind::TextNote)
        .tag(Tag::parse(["e", &target.id.to_hex(), hinted.as_str(), "reply"]).expect("e tag"))
        .by(author.public_key())
        .build()
        .unwrap();

    let contribution = router
        .preview(
            &RouteRequest::Write {
                event: EventValue::Unsigned(reply),
                access: fava_relay::Authority::Unauthenticated,
            },
            &RoutePlan::default(),
            &[],
        )
        .unwrap();
    let plan = RoutePlan::from_contribution(1, &contribution).unwrap();
    let relays: BTreeSet<_> = plan.destinations.keys().cloned().collect();

    assert_eq!(relays, BTreeSet::from([hinted, observed]));
}

fn relay(name: &str) -> RelayUrl {
    RelayUrl::parse(&format!("wss://{name}.example")).unwrap()
}
