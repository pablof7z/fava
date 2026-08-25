//! Exact access filters atomic relay contributions before winner selection.

use fava_query::{Query, QueryEvaluator, SourceEvent, SourceKind, SourceRevision, SourceSnapshot};
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_state::RelayEvent;
use nostr::event::{EventBuilder, FinalizeEvent, Kind};
use nostr::key::Keys;
use nostr::types::{RelayUrl, Timestamp};

#[test]
fn same_url_different_access_has_distinct_authority_and_one_winner()
-> Result<(), Box<dyn std::error::Error>> {
    let author = Keys::generate();
    let alice = Keys::generate();
    let relay = RelayUrl::parse("wss://relay.example")?;
    let public_key = RelaySessionKey {
        relay: relay.clone(),
        access: RelayAccess::Public,
    };
    let alice_key = RelaySessionKey {
        relay: relay.clone(),
        access: RelayAccess::Authenticated(alice.public_key()),
    };
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
                public_key.clone(),
                Timestamp::from(1),
            )),
            SourceEvent::Relay(RelayEvent::new(
                alice_event.clone(),
                alice_key.clone(),
                Timestamp::from(2),
            )),
        ],
    );
    let public_query = Query::events()
        .with_relay_access(RelayAccess::Public)
        .only_from_relays([relay.clone()])?;
    let alice_query = Query::events()
        .with_relay_access(RelayAccess::Authenticated(alice.public_key()))
        .only_from_relays([relay])?;
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
            .all(|item| item.session == public_key)
    );
    assert!(
        private.events[0]
            .relay_occurrences()
            .occurrences()
            .all(|item| item.session == alice_key)
    );
    Ok(())
}

#[test]
fn selection_filters_candidates_before_coordinate_winner_selection()
-> Result<(), Box<dyn std::error::Error>> {
    let author = Keys::generate();
    let relay = RelayUrl::parse("wss://relay.example")?;
    let session = RelaySessionKey {
        relay: relay.clone(),
        access: RelayAccess::Public,
    };
    let selected = EventBuilder::new(Kind::Metadata, "selected")
        .custom_created_at(Timestamp::from(10))
        .finalize(&author)?;
    let newer_unselected = EventBuilder::new(Kind::Metadata, "newer-unselected")
        .custom_created_at(Timestamp::from(20))
        .finalize(&author)?;
    let source = SourceSnapshot::current(
        SourceKind::LiveRelay {
            session: session.clone(),
        },
        SourceRevision(1),
        vec![
            SourceEvent::Relay(RelayEvent::new(
                selected.clone(),
                session.clone(),
                Timestamp::from(1),
            )),
            SourceEvent::Relay(RelayEvent::new(
                newer_unselected,
                session,
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
