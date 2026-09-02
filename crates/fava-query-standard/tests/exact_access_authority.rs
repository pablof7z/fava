//! Exact authority filters atomic relay contributions before winner selection.

use fava_query::{Query, QueryEvaluator, SourceEvent, SourceKind, SourceRevision, SourceSnapshot};
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::Authority;
use fava_state::RelayEvent;
use nostr::event::{EventBuilder, FinalizeEvent, Kind};
use nostr::key::Keys;
use nostr::types::{RelayUrl, Timestamp};

#[test]
fn same_url_different_authority_has_distinct_authority_and_one_winner()
-> Result<(), Box<dyn std::error::Error>> {
    let author = Keys::generate();
    let alice = Keys::generate();
    let relay = RelayUrl::parse("wss://relay.example")?;
    let public_event = EventBuilder::new(Kind::Metadata, "public")
        .custom_created_at(Timestamp::from(20))
        .finalize(&author)?;
    let alice_event = EventBuilder::new(Kind::Metadata, "alice")
        .custom_created_at(Timestamp::from(10))
        .finalize(&author)?;
    let source = SourceSnapshot::current(
        SourceKind::EventCache,
        SourceRevision(1),
        vec![
            SourceEvent::Relay(RelayEvent::new(
                public_event.clone(),
                relay.clone(),
                Authority::Unauthenticated,
                Timestamp::from(1),
            )),
            SourceEvent::Relay(RelayEvent::new(
                alice_event.clone(),
                relay.clone(),
                Authority::As(alice.public_key()),
                Timestamp::from(2),
            )),
        ],
    );
    let public_query = Query::events()
        .with_relay_access(Authority::Unauthenticated)
        .only_from_relays([relay.clone()])?;
    let alice_query = Query::events()
        .with_relay_access(Authority::As(alice.public_key()))
        .only_from_relays([relay.clone()])?;
    let public = StandardQueryEvaluator.evaluate(&public_query, std::slice::from_ref(&source))?;
    let private = StandardQueryEvaluator.evaluate(&alice_query, &[source])?;
    assert_eq!(public.events.len(), 1);
    assert_eq!(private.events.len(), 1);
    assert_eq!(public.events[0].id(), public_event.id);
    assert_eq!(private.events[0].id(), alice_event.id);
    assert!(
        public.events[0]
            .relay_occurrences()
            .occurrences()
            .all(|item| item.session == relay && item.authority == Authority::Unauthenticated)
    );
    assert!(
        private.events[0].relay_occurrences().occurrences().all(
            |item| item.session == relay && item.authority == Authority::As(alice.public_key())
        )
    );
    Ok(())
}

#[test]
fn selection_filters_candidates_before_coordinate_winner_selection()
-> Result<(), Box<dyn std::error::Error>> {
    let author = Keys::generate();
    let relay = RelayUrl::parse("wss://relay.example")?;
    let selected = EventBuilder::new(Kind::Metadata, "selected")
        .custom_created_at(Timestamp::from(10))
        .finalize(&author)?;
    let newer_unselected = EventBuilder::new(Kind::Metadata, "newer-unselected")
        .custom_created_at(Timestamp::from(20))
        .finalize(&author)?;
    let source = SourceSnapshot::current(
        SourceKind::LiveRelay {
            session: relay.clone(),
        },
        SourceRevision(1),
        vec![
            SourceEvent::Relay(RelayEvent::new(
                selected.clone(),
                relay.clone(),
                Authority::Unauthenticated,
                Timestamp::from(1),
            )),
            SourceEvent::Relay(RelayEvent::new(
                newer_unselected,
                relay.clone(),
                Authority::Unauthenticated,
                Timestamp::from(2),
            )),
        ],
    );
    let query = Query::events()
        .ids([selected.id])?
        .only_from_relays([relay])?;

    let snapshot = StandardQueryEvaluator.evaluate(&query, &[source])?;

    assert_eq!(snapshot.events.len(), 1);
    assert_eq!(snapshot.events[0].id(), selected.id);
    Ok(())
}
