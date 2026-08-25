//! Attribution and accepted-filter union remain ingress-owned.

use std::collections::BTreeMap;

use fava_ingest::{RelayIngestError, admit_subscription_event};
use fava_relay::{RelayAccess, RelaySessionKey};
use nostr::event::{EventBuilder, FinalizeEvent, Kind};
use nostr::filter::Filter;
use nostr::key::Keys;
use nostr::message::SubscriptionId;
use nostr::types::{RelayUrl, Timestamp};

fn session() -> RelaySessionKey {
    RelaySessionKey {
        relay: RelayUrl::parse("wss://relay.example").expect("relay URL"),
        access: RelayAccess::Public,
    }
}

#[test]
fn attribution_uses_only_the_named_subscription() {
    let keys = Keys::generate();
    let narrow = SubscriptionId::new("narrow");
    let broad = SubscriptionId::new("broad");
    let accepted = BTreeMap::from([
        (narrow.clone(), vec![Filter::new().kind(Kind::TextNote)]),
        (broad, vec![Filter::new().kind(Kind::ContactList)]),
    ]);
    let contact = EventBuilder::new(Kind::ContactList, "contacts")
        .finalize(&keys)
        .expect("signed");
    assert_eq!(
        admit_subscription_event(&session(), &accepted, &narrow, contact, Timestamp::from(10),),
        Err(RelayIngestError::OffFilter)
    );
}

#[test]
fn every_filter_in_one_req_authorizes_its_union() {
    let keys = Keys::generate();
    let id = SubscriptionId::new("multi");
    let accepted = BTreeMap::from([(
        id.clone(),
        vec![
            Filter::new().kind(Kind::TextNote),
            Filter::new().kind(Kind::ContactList),
        ],
    )]);
    for kind in [Kind::TextNote, Kind::ContactList] {
        let event = EventBuilder::new(kind, "matching")
            .finalize(&keys)
            .expect("signed");
        assert!(
            admit_subscription_event(&session(), &accepted, &id, event, Timestamp::from(10),)
                .is_ok()
        );
    }
}

#[test]
fn empty_accepted_filter_set_authorizes_nothing() {
    let id = SubscriptionId::new("empty");
    let accepted = BTreeMap::from([(id.clone(), Vec::new())]);
    let event = EventBuilder::new(Kind::TextNote, "note")
        .finalize(&Keys::generate())
        .expect("signed");
    assert_eq!(
        admit_subscription_event(&session(), &accepted, &id, event, Timestamp::from(10),),
        Err(RelayIngestError::UnauthorizedSubscription)
    );
}
