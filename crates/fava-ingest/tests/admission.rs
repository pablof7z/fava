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

fn accepted() -> BTreeMap<SubscriptionId, Filter> {
    BTreeMap::from([(
        SubscriptionId::new("expected"),
        Filter::new().kind(Kind::TextNote),
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
        (narrow.clone(), Filter::new().kind(Kind::TextNote)),
        (broad.clone(), Filter::new().kind(Kind::ContactList)),
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
