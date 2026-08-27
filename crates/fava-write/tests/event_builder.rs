//! Public raw event builder field, order, identity, and bound proofs.

use fava_write::{
    Event, EventBuildError, EventBuilder, Kind, Tag, Timestamp, WriteIntentError, WriteRouting,
};
use nostr::key::Keys;
use nostr::types::RelayUrl;

fn keys() -> Keys {
    Keys::parse("0000000000000000000000000000000000000000000000000000000000000001")
        .expect("fixed test key")
}

fn custom(name: &str, value: &str) -> Tag {
    Tag::parse([name, value]).expect("custom tag is valid")
}

/// Minimal known-good signed Nostr event for tag tests.
///
/// kind=1, pubkey=e8ed37..., id=38acf9...
fn signed_event() -> Event {
    Event::from_json(
        r#"{"content":"test","created_at":1703184271,"id":"38acf9b08d06859e49237688a9fd6558c448766f47457236c2331f93538992c6","kind":1,"pubkey":"e8ed3798c6ffebffa08501ac39e271662bfd160f688f94c45d692d8767dd345a","sig":"f76d5ecc8e7de688ac12b9d19edaacdcffb8f0c8fa2a44c00767363af3f04dbc069542ddc5d2f63c94cb5e6ce701589d538cf2db3b1f1211a96596fabb6ecafe","tags":[]}"#,
    )
    .expect("known-good event JSON")
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
fn builder_bounds_cumulative_raw_routes_before_normalization() {
    let repeated = relay("builder-raw");
    let builder = EventBuilder::new(keys().public_key(), Kind::Custom(60_005))
        .to_relays(vec![repeated.clone(); 512])
        .expect("first finite batch fits")
        .to_relays(vec![repeated; 512])
        .expect("cumulative raw bound fits exactly");
    assert!(matches!(
        builder.to_relays([relay("overflow")]),
        Err(WriteIntentError::TooManyRawExplicitRelays {
            actual: 1_025,
            maximum: 1_024,
        })
    ));
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

// ── Taggable API tests ────────────────────────────────────────────────────────

#[test]
fn tag_with_raw_tag_preserves_existing_behaviour() {
    let author = keys().public_key();
    let raw = Tag::parse(["h", "my-group"]).expect("h tag");
    let event = EventBuilder::new(author, Kind::TextNote)
        .tag(raw.clone())
        .build()
        .expect("builds");
    assert_eq!(event.tags.as_slice(), [raw]);
}

#[test]
fn tag_with_public_key_produces_p_tag() {
    let author = keys().public_key();
    let other = Keys::parse("0000000000000000000000000000000000000000000000000000000000000002")
        .expect("second key")
        .public_key();
    let event = EventBuilder::new(author, Kind::TextNote)
        .tag(other)
        .build()
        .expect("builds");
    let tags = event.tags.as_slice();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].kind(), "p");
    assert_eq!(tags[0].content(), Some(other.to_hex().as_str()));
}

#[test]
fn tag_with_event_produces_nip22_e_p_k_tags() {
    let author = keys().public_key();
    let parent = signed_event();
    let event = EventBuilder::new(author, Kind::Custom(1111))
        .tag(&parent)
        .build()
        .expect("builds");
    let tags = event.tags.as_slice();
    assert_eq!(tags.len(), 3, "expect e + p + k");

    let e = &tags[0];
    assert_eq!(e.kind(), "e");
    assert_eq!(e.content(), Some(parent.id.to_hex().as_str()));
    assert_eq!(e.as_slice().get(2).map(String::as_str), Some(""));
    assert_eq!(
        e.as_slice().get(3).map(String::as_str),
        Some(parent.pubkey.to_hex().as_str())
    );

    let p = &tags[1];
    assert_eq!(p.kind(), "p");
    assert_eq!(p.content(), Some(parent.pubkey.to_hex().as_str()));

    let k = &tags[2];
    assert_eq!(k.kind(), "k");
    assert_eq!(k.content(), Some(parent.kind.to_string().as_str()));
}

#[test]
fn tag_with_raw_array_produces_single_tag() {
    let author = keys().public_key();
    let event = EventBuilder::new(author, Kind::TextNote)
        .tag(["custom-key", "custom-value"])
        .build()
        .expect("builds");
    let tags = event.tags.as_slice();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].kind(), "custom-key");
    assert_eq!(tags[0].content(), Some("custom-value"));
}

#[test]
fn tag_event_marked_produces_nip10_e_and_p_tags() {
    let author = keys().public_key();
    let parent = signed_event();
    let event = EventBuilder::new(author, Kind::TextNote)
        .tag_event_marked(&parent, "reply")
        .build()
        .expect("builds");
    let tags = event.tags.as_slice();
    assert_eq!(tags.len(), 2, "expect e + p only, no k tag");

    let e = &tags[0];
    assert_eq!(e.kind(), "e");
    assert_eq!(e.content(), Some(parent.id.to_hex().as_str()));
    assert_eq!(e.as_slice().get(2).map(String::as_str), Some(""));
    assert_eq!(e.as_slice().get(3).map(String::as_str), Some("reply"));

    let p = &tags[1];
    assert_eq!(p.kind(), "p");
    assert_eq!(p.content(), Some(parent.pubkey.to_hex().as_str()));
}

#[test]
fn tag_event_marked_accepts_different_markers() {
    let author = keys().public_key();
    let parent = signed_event();
    for marker in ["root", "reply", "mention", "poop"] {
        let event = EventBuilder::new(author, Kind::TextNote)
            .tag_event_marked(&parent, marker)
            .build()
            .expect("builds");
        let e_tag = &event.tags.as_slice()[0];
        assert_eq!(
            e_tag.as_slice().get(3).map(String::as_str),
            Some(marker),
            "marker {marker} not at position 3"
        );
    }
}

fn relay(name: &str) -> RelayUrl {
    RelayUrl::parse(&format!("wss://{name}.example")).expect("relay URL")
}
