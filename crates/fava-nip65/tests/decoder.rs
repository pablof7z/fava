//! Public behavior proof for the NIP-65-owned decoder boundary.

use std::collections::BTreeSet;

use fava_nip65::RelayList;
use fava_write::{EventBuilder, EventValue, Kind, PublicKey, Tag};
use nostr::types::RelayUrl;

fn author() -> PublicKey {
    PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
        .expect("generator public key")
}

fn relay(url: &str) -> RelayUrl {
    RelayUrl::parse(url).expect("valid relay URL")
}

#[test]
fn hostile_tags_stay_local_and_valid_siblings_survive() {
    let event = EventBuilder::new(author(), Kind::from(10_002_u16))
        .tag(Tag::parse(["r", "wss://read.example", "read"]).expect("read tag"))
        .tag(Tag::parse(["r"]).expect("short tag"))
        .tag(Tag::parse(["r", "not a relay URL", "write"]).expect("invalid relay tag"))
        .tag(Tag::parse(["r", "wss://unknown.example", "sideways"]).expect("unknown marker tag"))
        .tag(Tag::parse(["r", "wss://both.example"]).expect("unmarked tag"))
        .tag(
            Tag::parse(["r", "wss://write.example", "write", "future-cell"])
                .expect("extended write tag"),
        )
        .tag(Tag::parse(["r", "wss://both.example", "read"]).expect("duplicate tag"))
        .tag(Tag::parse(["p", "unrelated"]).expect("unrelated tag"))
        .build()
        .expect("bounded event");

    let list = RelayList::from_event(&EventValue::Unsigned(event)).expect("relay list");

    assert_eq!(
        list.read_relays(),
        &BTreeSet::from([relay("wss://both.example"), relay("wss://read.example"),])
    );
    assert_eq!(
        list.write_relays(),
        &BTreeSet::from([relay("wss://both.example"), relay("wss://write.example"),])
    );
}

#[test]
fn present_empty_marker_is_unknown_not_omitted() {
    let event = EventBuilder::new(author(), Kind::from(10_002_u16))
        .tag(Tag::parse(["r", "wss://empty-marker.example", ""]).expect("empty marker tag"))
        .build()
        .expect("bounded event");

    let list = RelayList::from_event(&EventValue::Unsigned(event)).expect("relay list");

    assert!(list.read_relays().is_empty());
    assert!(list.write_relays().is_empty());
}

#[test]
fn repeated_relay_does_not_consume_distinct_result_bound() {
    let mut builder = EventBuilder::new(author(), Kind::from(10_002_u16));
    for _ in 0..300 {
        builder =
            builder.tag(Tag::parse(["r", "wss://repeated.example"]).expect("repeated relay tag"));
    }
    let event = builder.build().expect("bounded event");

    let list = RelayList::from_event(&EventValue::Unsigned(event)).expect("relay list");

    assert_eq!(list.read_relays().len(), 1);
    assert_eq!(list.write_relays().len(), 1);
}
