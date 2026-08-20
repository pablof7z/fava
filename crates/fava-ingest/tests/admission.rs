//! Relay-ingest attribution and admission evidence.

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

#[test]
fn forged_wrong_subscription_and_off_filter_events_never_enter_the_cache() {
    let cache = MemoryEventCache::default();
    let keys = Keys::generate();
    let expected = SubscriptionId::new("expected");
    let filter = Filter::new().kind(Kind::TextNote);
    let valid = EventBuilder::new(Kind::TextNote, "valid")
        .finalize(&keys)
        .expect("event signs");

    let wrong = admit_subscription_event(
        &cache,
        &session(),
        &expected,
        &SubscriptionId::new("wrong"),
        &filter,
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
        &expected,
        &expected,
        &filter,
        off_filter,
        Timestamp::from(10),
    );
    assert_eq!(refused, Err(RelayIngestError::OffFilter));

    let mut forged = valid.clone();
    forged.content = "forged after signing".to_owned();
    let refused = admit_subscription_event(
        &cache,
        &session(),
        &expected,
        &expected,
        &filter,
        forged,
        Timestamp::from(10),
    );
    assert!(matches!(refused, Err(RelayIngestError::InvalidEvent(_))));
    assert!(cache.is_empty().expect("cache readable"));

    assert_eq!(
        admit_subscription_event(
            &cache,
            &session(),
            &expected,
            &expected,
            &filter,
            valid,
            Timestamp::from(10),
        ),
        Ok(true)
    );
    assert_eq!(cache.len().expect("cache readable"), 1);
}
