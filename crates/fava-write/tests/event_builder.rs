//! Public raw event builder field, order, identity, and bound proofs.

use fava_state::RelayUrl;
use fava_write::{
    EventBuildError, EventBuilder, Kind, Tag, Timestamp, WriteIntentError, WriteRouting,
};
use nostr::key::Keys;

fn keys() -> Keys {
    Keys::parse("0000000000000000000000000000000000000000000000000000000000000001")
        .expect("fixed test key")
}

fn custom(name: &str, value: &str) -> Tag {
    Tag::parse([name, value]).expect("custom tag is valid")
}

#[test]
fn raw_parts_and_bulk_tags_preserve_every_exact_field_and_order() {
    let keys = keys();
    let author = keys.public_key();
    let kind = Kind::Custom(60_001);
    let created_at = Timestamp::from(123);
    let tags = vec![
        Tag::parse(["something something"]).expect("single-field custom tag is valid"),
        custom("x-a", "poop"),
        custom("x-unknown", "one"),
    ];
    let from_parts = EventBuilder::from_parts(
        author,
        kind,
        created_at,
        tags.clone(),
        "opaque raw content".to_owned(),
    )
    .build()
    .expect("raw parts build");
    let bulk = EventBuilder::new(author, kind)
        .created_at(created_at)
        .tags(tags.clone())
        .content("opaque raw content")
        .build()
        .expect("bulk tags build");

    assert_eq!(from_parts, bulk);
    assert_eq!(from_parts.pubkey, author);
    assert_eq!(from_parts.kind, kind);
    assert_eq!(from_parts.created_at, created_at);
    assert_eq!(from_parts.tags.as_slice(), tags.as_slice());
    assert_eq!(from_parts.content, "opaque raw content");
    assert!(from_parts.id.is_some());
}

#[test]
fn raw_parts_and_bulk_tags_share_exact_hostile_bounds() {
    let keys = keys();
    let tags = (0..2_001)
        .map(|index| custom("x-hostile", &index.to_string()))
        .collect::<Vec<_>>();
    let from_parts = EventBuilder::from_parts(
        keys.public_key(),
        Kind::Custom(60_002),
        Timestamp::from(1),
        tags.clone(),
        String::new(),
    )
    .build();
    let bulk = EventBuilder::new(keys.public_key(), Kind::Custom(60_002))
        .tags(tags)
        .build();
    assert!(matches!(
        from_parts,
        Err(EventBuildError::TooManyTags {
            actual: 2_001,
            maximum: 2_000
        })
    ));
    assert!(matches!(
        bulk,
        Err(EventBuildError::TooManyTags {
            actual: 2_001,
            maximum: 2_000
        })
    ));

    let oversized = EventBuilder::from_parts(
        keys.public_key(),
        Kind::Custom(60_002),
        Timestamp::from(1),
        Vec::new(),
        "z".repeat(131_073),
    )
    .build();
    assert!(matches!(oversized, Err(EventBuildError::TooLarge { .. })));
}

#[test]
fn event_build_tag_refusal_converts_without_losing_fields() {
    let converted = WriteIntentError::from(EventBuildError::TooManyTags {
        actual: 2_001,
        maximum: 2_000,
    });
    assert_eq!(
        converted,
        WriteIntentError::TooManyTags {
            actual: 2_001,
            maximum: 2_000,
        }
    );
    assert_eq!(
        converted.to_string(),
        "event tags exceed bound: 2001 > 2000"
    );
}

#[test]
fn event_build_byte_refusal_converts_without_losing_fields() {
    let converted = WriteIntentError::from(EventBuildError::TooLarge {
        bytes: 131_073,
        maximum: 131_072,
    });
    assert_eq!(
        converted,
        WriteIntentError::TooLarge {
            bytes: 131_073,
            maximum: 131_072,
        }
    );
    assert_eq!(
        converted.to_string(),
        "event bytes exceed bound: 131073 > 131072"
    );
}

#[test]
fn event_build_encoding_refusal_converts_without_losing_fields() {
    let converted = WriteIntentError::from(EventBuildError::Encoding("exact encoding".to_owned()));
    assert_eq!(
        converted,
        WriteIntentError::Encoding("exact encoding".to_owned())
    );
    assert_eq!(
        converted.to_string(),
        "event encoding failed: exact encoding"
    );
}

#[test]
fn builder_keeps_neutral_explicit_routing_out_of_the_event_body() {
    let author = keys().public_key();
    let first = relay("first");
    let second = relay("second");
    let builder = EventBuilder::new(author, Kind::Custom(60_003))
        .created_at(Timestamp::from(9))
        .content("same body")
        .to_relays([first.clone(), second.clone(), first.clone()])
        .expect("route composes");
    let plain = EventBuilder::new(author, Kind::Custom(60_003))
        .created_at(Timestamp::from(9))
        .content("same body")
        .build()
        .expect("plain event builds");

    let (routed, routing) = builder
        .into_event_and_routing()
        .expect("routed event builds");

    assert_eq!(routed, plain);
    assert_eq!(routing, WriteRouting::Explicit(vec![first, second]));
}

#[test]
fn event_only_build_refuses_an_attached_explicit_route() {
    let builder = EventBuilder::new(keys().public_key(), Kind::Custom(60_004))
        .to_relays([relay("attached")])
        .expect("route composes");

    assert_eq!(
        builder.build(),
        Err(EventBuildError::ExplicitRoutingAttached)
    );
}

fn relay(name: &str) -> RelayUrl {
    RelayUrl::parse(&format!("wss://{name}.example")).expect("relay URL")
}
