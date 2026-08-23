//! Relay-ingest attribution and admission evidence.

use std::collections::BTreeMap;

use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_ingest::{RelayIngestError, admit_subscription_event};
use fava_state::{RelayAccess, RelaySessionKey, RelayUrl, Timestamp};
use nostr::event::{EventBuilder, FinalizeEvent, Kind};
use nostr::filter::Filter;
use nostr::key::Keys;
use nostr::message::SubscriptionId;

fn session() -> RelaySessionKey {
    RelaySessionKey::new(
        RelayUrl::parse("wss://relay.example").expect("relay URL"),
        RelayAccess::public(),
    )
}

fn accepted() -> BTreeMap<SubscriptionId, Vec<Filter>> {
    BTreeMap::from([(
        SubscriptionId::new("expected"),
        vec![Filter::new().kind(Kind::TextNote)],
    )])
}

#[test]
fn forged_wrong_subscription_and_off_filter_events_never_enter_the_cache() {
    let cache = MemoryEventCache::default();
    let keys = Keys::generate();
    let accepted = accepted();
    let expected = SubscriptionId::new("expected");
    let valid = EventBuilder::new(Kind::TextNote, "valid")
        .finalize(&keys)
        .expect("event signs");

    let wrong = admit_subscription_event(
        &cache,
        &session(),
        &accepted,
        &SubscriptionId::new("wrong"),
        valid.clone(),
        Timestamp::from(10),
    );
    assert_eq!(wrong, Err(RelayIngestError::WrongSubscription));

    let off_filter = EventBuilder::new(Kind::ContactList, "off filter")
        .finalize(&keys)
        .expect("event signs");
    let refused = admit_subscription_event(
        &cache,
        &session(),
        &accepted,
        &expected,
        off_filter,
        Timestamp::from(10),
    );
    assert_eq!(refused, Err(RelayIngestError::OffFilter));

    let mut forged = valid.clone();
    forged.content = "forged after signing".to_owned();
    let refused = admit_subscription_event(
        &cache,
        &session(),
        &accepted,
        &expected,
        forged,
        Timestamp::from(10),
    );
    assert!(matches!(refused, Err(RelayIngestError::InvalidEvent(_))));
    assert!(cache.is_empty().expect("cache readable"));

    assert_eq!(
        admit_subscription_event(
            &cache,
            &session(),
            &accepted,
            &expected,
            valid,
            Timestamp::from(10),
        ),
        Ok(true)
    );
    assert_eq!(cache.len().expect("cache readable"), 1);
}

/// A relay chooses the subscription ID on every EVENT frame. It must not be
/// able to choose which accepted filter authorizes that event: an EVENT
/// attributed to subscription A is validated against A's accepted filter only,
/// even when the same session accepted a broader subscription B that the event
/// would satisfy.
#[test]
fn relay_cannot_admit_an_event_under_a_filter_it_was_not_attributed_to() {
    let cache = MemoryEventCache::default();
    let keys = Keys::generate();

    let narrow = SubscriptionId::new("a");
    let broad = SubscriptionId::new("b");
    let accepted = BTreeMap::from([
        (narrow.clone(), vec![Filter::new().kind(Kind::TextNote)]),
        (broad.clone(), vec![Filter::new().kind(Kind::ContactList)]),
    ]);

    let only_matches_broad = EventBuilder::new(Kind::ContactList, "b")
        .finalize(&keys)
        .expect("event signs");

    assert_eq!(
        admit_subscription_event(
            &cache,
            &session(),
            &accepted,
            &narrow,
            only_matches_broad.clone(),
            Timestamp::from(10),
        ),
        Err(RelayIngestError::OffFilter),
        "an EVENT attributed to A must not be validated by B's accepted filter"
    );
    assert!(cache.is_empty().expect("cache readable"));

    assert_eq!(
        admit_subscription_event(
            &cache,
            &session(),
            &accepted,
            &broad,
            only_matches_broad,
            Timestamp::from(10),
        ),
        Ok(true)
    );
    assert_eq!(cache.len().expect("cache readable"), 1);
}

/// A NIP-01 REQ carries one or more filters and the relay serves their union.
/// Keeping only the first one silently drops every event the later filters
/// asked for.
#[test]
fn every_filter_a_multi_filter_req_installed_still_authorizes_its_events() {
    let cache = MemoryEventCache::default();
    let keys = Keys::generate();
    let id = SubscriptionId::new("multi");
    let accepted = BTreeMap::from([(
        id.clone(),
        vec![
            Filter::new().kind(Kind::TextNote),
            Filter::new().kind(Kind::ContactList),
        ],
    )]);

    let under_first = EventBuilder::new(Kind::TextNote, "first")
        .finalize(&keys)
        .expect("event signs");
    let under_second = EventBuilder::new(Kind::ContactList, "second")
        .finalize(&keys)
        .expect("event signs");

    assert_eq!(
        admit_subscription_event(
            &cache,
            &session(),
            &accepted,
            &id,
            under_first,
            Timestamp::from(10),
        ),
        Ok(true)
    );
    assert_eq!(
        admit_subscription_event(
            &cache,
            &session(),
            &accepted,
            &id,
            under_second,
            Timestamp::from(10),
        ),
        Ok(true),
        "an event asked for by a later filter of the same REQ must not be dropped"
    );
    assert_eq!(cache.len().expect("cache readable"), 2);

    let unasked = EventBuilder::new(Kind::Metadata, "unasked")
        .finalize(&keys)
        .expect("event signs");
    assert_eq!(
        admit_subscription_event(
            &cache,
            &session(),
            &accepted,
            &id,
            unasked,
            Timestamp::from(10),
        ),
        Err(RelayIngestError::OffFilter),
        "the union of the installed filters is still an exact bound"
    );
}

/// A subscription accepted with no filter authorizes nothing. Admitting under
/// it would let a relay push anything it likes.
#[test]
fn a_subscription_accepted_with_no_filter_authorizes_nothing() {
    let cache = MemoryEventCache::default();
    let keys = Keys::generate();
    let id = SubscriptionId::new("empty");
    let accepted = BTreeMap::from([(id.clone(), Vec::new())]);
    let event = EventBuilder::new(Kind::TextNote, "anything")
        .finalize(&keys)
        .expect("event signs");

    assert_eq!(
        admit_subscription_event(
            &cache,
            &session(),
            &accepted,
            &id,
            event,
            Timestamp::from(10),
        ),
        Err(RelayIngestError::UnauthorizedSubscription)
    );
    assert!(cache.is_empty().expect("cache readable"));
}
