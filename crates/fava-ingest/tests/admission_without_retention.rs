//! Relay admission produces a live contribution without a cache dependency.

use std::collections::BTreeMap;

use fava_ingest::{RelayIngestError, admit_subscription_event};
use fava_relay::Authority;
use nostr::event::{EventBuilder, FinalizeEvent, Kind};
use nostr::filter::Filter;
use nostr::key::Keys;
use nostr::message::SubscriptionId;
use nostr::types::{RelayUrl, Timestamp};

#[test]
fn valid_event_is_atomic_admission_before_optional_retention()
-> Result<(), Box<dyn std::error::Error>> {
    let session = RelayUrl::parse("wss://relay.example")?;
    let subscription = SubscriptionId::new("profiles");
    let event = EventBuilder::new(Kind::Metadata, "profile").finalize(&Keys::generate())?;
    let accepted = BTreeMap::from([(
        subscription.clone(),
        vec![Filter::new().kind(Kind::Metadata)],
    )]);
    let admitted = admit_subscription_event(
        &session,
        &Authority::Unauthenticated,
        &accepted,
        &subscription,
        event.clone(),
        Timestamp::from(7),
    )?;
    assert_eq!(admitted.event(), &event);
    assert_eq!(admitted.occurrence().session, session);
    assert_eq!(admitted.occurrence().authority, Authority::Unauthenticated);
    assert_eq!(admitted.occurrence().observed_at, Timestamp::from(7));
    Ok(())
}

#[test]
fn invalid_event_never_produces_a_contribution() -> Result<(), Box<dyn std::error::Error>> {
    let session = RelayUrl::parse("wss://relay.example")?;
    let subscription = SubscriptionId::new("notes");
    let mut event = EventBuilder::new(Kind::TextNote, "valid").finalize(&Keys::generate())?;
    event.content = "tampered".to_owned();
    let accepted = BTreeMap::from([(
        subscription.clone(),
        vec![Filter::new().kind(Kind::TextNote)],
    )]);
    assert!(matches!(
        admit_subscription_event(
            &session,
            &Authority::Unauthenticated,
            &accepted,
            &subscription,
            event,
            Timestamp::from(7),
        ),
        Err(RelayIngestError::InvalidEvent(_))
    ));
    Ok(())
}
