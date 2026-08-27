use fava_state::{EventCoordinate, event_coordinate};
use fava_write::{
    EventId, EventValue, Kind, ReplaceableEventEdit, Tag, Timestamp, WriteIntentError,
};
use nostr::event::{EventBuilder, FinalizeEvent};
use nostr::key::Keys;

use super::{
    bookmark_coordinate, bookmark_event, materializer, unbookmark_coordinate, unbookmark_event,
};

const BOOKMARK_KIND: u16 = 10_003;

fn tag(values: &[&str]) -> Tag {
    Tag::parse(values.iter().copied()).expect("nonempty test tag")
}

fn source(
    keys: &Keys,
    kind: Kind,
    created_at: u64,
    content: &str,
    tags: Vec<Tag>,
) -> fava_write::Event {
    EventBuilder::new(kind, content)
        .tags(tags)
        .custom_created_at(Timestamp::from(created_at))
        .finalize(keys)
        .expect("source signs")
}

fn materialize(
    author: fava_write::PublicKey,
    edit: &ReplaceableEventEdit,
    source: Option<&fava_write::Event>,
    created_at: u64,
) -> Result<fava_write::UnsignedEvent, WriteIntentError> {
    let source = source.cloned().map(EventValue::Signed);
    materializer().materialize(edit, author, source.as_ref(), Timestamp::from(created_at))
}

fn article_coordinate() -> EventCoordinate {
    EventCoordinate::Replaceable {
        author: Keys::generate().public_key(),
        kind: Kind::from_u16(30_023),
        identifier: Some("article".to_owned()),
    }
}

fn coordinate_text(coordinate: &EventCoordinate) -> String {
    match coordinate {
        EventCoordinate::Replaceable {
            author,
            kind,
            identifier,
        } => format!(
            "{}:{}:{}",
            kind.as_u16(),
            author.to_hex(),
            identifier.as_deref().unwrap_or_default()
        ),
        EventCoordinate::Event(_) => panic!("test coordinate must be replaceable"),
    }
}

#[test]
fn bookmark_and_unbookmark_are_opposing_authorless_edits() {
    let actor = Keys::generate();
    let event_id = EventId::from_byte_array([7; 32]);
    let coordinate = article_coordinate();
    let add_event = bookmark_event(event_id).expect("event edit");
    let remove_event = unbookmark_event(event_id).expect("opposing event edit");
    assert_ne!(add_event, remove_event);
    assert_eq!(add_event.kind(), Kind::from_u16(BOOKMARK_KIND));
    assert_eq!(add_event.identifier(), None);
    let first =
        materialize(actor.public_key(), &add_event, None, 4).expect("first public bookmark list");
    assert_eq!(first.pubkey, actor.public_key());
    assert_eq!(first.kind, Kind::from_u16(BOOKMARK_KIND));
    assert_eq!(first.tags.as_slice(), &[tag(&["e", &event_id.to_hex()])]);
    assert_eq!(first.content, "");

    let add_coordinate = bookmark_coordinate(coordinate.clone()).expect("coordinate edit");
    assert_ne!(
        add_coordinate,
        unbookmark_coordinate(coordinate.clone()).expect("opposing coordinate edit")
    );
    let coordinate_event =
        materialize(actor.public_key(), &add_coordinate, None, 4).expect("coordinate bookmark");
    assert_eq!(
        coordinate_event.tags.as_slice(),
        &[tag(&["a", &coordinate_text(&coordinate)])]
    );
}

#[test]
fn empty_identifier_addressable_coordinate_round_trips_from_event_helper() {
    let actor = Keys::generate();
    let target_author = Keys::generate().public_key();
    let coordinate = event_coordinate(
        EventId::from_byte_array([6; 32]),
        target_author,
        Kind::from_u16(30_023),
        &[],
    );
    assert_eq!(
        coordinate,
        EventCoordinate::Replaceable {
            author: target_author,
            kind: Kind::from_u16(30_023),
            identifier: Some(String::new()),
        }
    );

    let add = bookmark_coordinate(coordinate.clone())
        .expect("empty identifier is a valid addressable coordinate");
    let added =
        materialize(actor.public_key(), &add, None, 1).expect("coordinate codec round trips");
    assert_eq!(
        added.tags.as_slice(),
        &[tag(&["a", &coordinate_text(&coordinate)])]
    );
    let signed = source(
        &actor,
        Kind::from_u16(BOOKMARK_KIND),
        1,
        "opaque",
        added.tags.to_vec(),
    );
    let remove = unbookmark_coordinate(coordinate).expect("remove edit");
    let removed = materialize(actor.public_key(), &remove, Some(&signed), 2)
        .expect("opposing codec round trips");
    assert!(removed.tags.is_empty());
    assert_eq!(removed.content, "opaque");
}

#[test]
fn bookmark_preserves_unrelated_state_and_orders_deterministically() {
    let actor = Keys::generate();
    let event_id = EventId::from_byte_array([8; 32]);
    let other_id = EventId::from_byte_array([9; 32]);
    let coordinate = article_coordinate();
    let target = tag(&["e", &event_id.to_hex(), "wss://hint.example", "bookmark"]);
    let tags = vec![
        tag(&["t", "first"]),
        target.clone(),
        tag(&["x", "future", "bytes"]),
        tag(&["a", &coordinate_text(&coordinate), "wss://article.example"]),
        tag(&["e", &other_id.to_hex()]),
        tag(&["e", &event_id.to_hex(), "wss://duplicate.example"]),
        tag(&["e", "not-an-event-id", "malformed"]),
        tag(&["a", "not:a:coordinate", "malformed"]),
    ];
    let encrypted = "opaque-private-ciphertext?iv=must-not-be-read";
    let source = source(
        &actor,
        Kind::from_u16(BOOKMARK_KIND),
        10,
        encrypted,
        tags.clone(),
    );
    let edit = bookmark_event(event_id).expect("bookmark edit");
    let first =
        materialize(actor.public_key(), &edit, Some(&source), 11).expect("bookmark applies");
    let second =
        materialize(actor.public_key(), &edit, Some(&source), 11).expect("deterministic repeat");
    assert_eq!(first, second);
    assert_eq!(first.content, encrypted);
    assert_eq!(
        first.tags.as_slice(),
        &[
            tags[0].clone(),
            target,
            tags[2].clone(),
            tags[3].clone(),
            tags[4].clone(),
            tags[6].clone(),
            tags[7].clone(),
        ]
    );
}

#[test]
fn bookmark_duplicate_and_adjacent_edits_are_idempotent() {
    let actor = Keys::generate();
    let event_id = EventId::from_byte_array([10; 32]);
    let coordinate = article_coordinate();
    let event_hex = event_id.to_hex();
    let coordinate_string = coordinate_text(&coordinate);
    let tags = vec![
        tag(&["e", &event_hex, "first"]),
        tag(&["e", &event_hex, "middle"]),
        tag(&["x", "kept"]),
        tag(&["a", &coordinate_string, "first"]),
        tag(&["a", &coordinate_string, "last"]),
        tag(&["e", &event_hex, "last"]),
    ];
    let original = source(&actor, Kind::from_u16(BOOKMARK_KIND), 20, "opaque", tags);
    let add = bookmark_event(event_id).expect("bookmark edit");
    let once =
        materialize(actor.public_key(), &add, Some(&original), 21).expect("event deduplicates");
    assert_eq!(
        once.tags
            .iter()
            .filter(|tag| tag.as_slice().get(1) == Some(&event_hex))
            .count(),
        1
    );
    let signed_once = source(
        &actor,
        Kind::from_u16(BOOKMARK_KIND),
        21,
        "opaque",
        once.tags.clone().to_vec(),
    );
    let twice = materialize(actor.public_key(), &add, Some(&signed_once), 22)
        .expect("event repeat idempotent");
    assert_eq!(once.tags, twice.tags);

    let remove_coordinate = unbookmark_coordinate(coordinate).expect("coordinate removal");
    let removed = materialize(actor.public_key(), &remove_coordinate, Some(&original), 21)
        .expect("all a tags removed");
    assert!(
        removed
            .tags
            .iter()
            .all(|tag| tag.as_slice().get(1) != Some(&coordinate_string))
    );
    let signed_removed = source(
        &actor,
        Kind::from_u16(BOOKMARK_KIND),
        21,
        "opaque",
        removed.tags.clone().to_vec(),
    );
    let removed_twice = materialize(
        actor.public_key(),
        &remove_coordinate,
        Some(&signed_removed),
        22,
    )
    .expect("repeat removal");
    assert_eq!(removed.tags, removed_twice.tags);
}

#[test]
fn equivalent_duplicate_sets_canonicalize_across_permutations() {
    let actor = Keys::generate();
    let event_id = EventId::from_byte_array([12; 32]);
    let event_hex = event_id.to_hex();
    let low = tag(&["e", &event_hex, "wss://a.example", "a"]);
    let high = tag(&["e", &event_hex, "wss://z.example", "z"]);
    let first = source(
        &actor,
        Kind::from_u16(BOOKMARK_KIND),
        30,
        "opaque",
        vec![
            tag(&["x", "before"]),
            high.clone(),
            tag(&["x", "between"]),
            low.clone(),
            tag(&["x", "after"]),
        ],
    );
    let second = source(
        &actor,
        Kind::from_u16(BOOKMARK_KIND),
        30,
        "opaque",
        vec![
            tag(&["x", "before"]),
            low.clone(),
            tag(&["x", "between"]),
            high,
            tag(&["x", "after"]),
        ],
    );
    let edit = bookmark_event(event_id).expect("bookmark edit");
    let first_output =
        materialize(actor.public_key(), &edit, Some(&first), 31).expect("first permutation");
    let second_output =
        materialize(actor.public_key(), &edit, Some(&second), 31).expect("second permutation");
    assert_eq!(first_output, second_output);
    assert_eq!(first_output.tags[1], low);
}

#[test]
fn bookmark_bounds_private_and_invalid_sources_are_typed_refusals() {
    let actor = Keys::generate();
    let event_id = EventId::from_byte_array([11; 32]);
    let edit = bookmark_event(event_id).expect("bookmark edit");
    assert_source_refusals(&actor, &edit);
    assert_codec_and_target_refusals(&actor, event_id, &edit);
    assert_size_and_timestamp_refusals(&actor, &edit);
}

#[test]
fn hostile_sources_preserve_the_generic_builder_refusal_before_signature_verification() {
    let actor = Keys::generate();
    let event_id = EventId::from_byte_array([13; 32]);
    let edit = bookmark_event(event_id).expect("bookmark edit");

    let mut escaped = source(
        &actor,
        Kind::from_u16(BOOKMARK_KIND),
        1,
        &"\\".repeat(70_000),
        Vec::new(),
    );
    escaped.id = EventId::from_byte_array([43; 32]);
    let expected = fava_write::EventBuilder::new(
        actor.public_key(),
        Kind::from_u16(BOOKMARK_KIND),
    )
    .created_at(Timestamp::from(2))
    .content(escaped.content.clone())
    .tag(Tag::event(event_id))
    .build()
    .expect_err("hostile content exceeds the generic event bound");
    assert_eq!(
        materialize(actor.public_key(), &edit, Some(&escaped), 2),
        Err(WriteIntentError::from(expected))
    );

    let mut nested_values = Vec::with_capacity(50_000);
    nested_values.push("x".to_owned());
    nested_values.resize(50_000, String::new());
    let mut nested = source(
        &actor,
        Kind::from_u16(BOOKMARK_KIND),
        1,
        "opaque",
        vec![Tag::parse(nested_values).expect("nonempty hostile tag")],
    );
    nested.id = EventId::from_byte_array([44; 32]);
    let expected = fava_write::EventBuilder::new(
        actor.public_key(),
        Kind::from_u16(BOOKMARK_KIND),
    )
    .created_at(Timestamp::from(2))
    .content(nested.content.clone())
    .tag(nested.tags[0].clone())
    .tag(Tag::event(event_id))
    .build()
    .expect_err("hostile tag values exceed the generic event bound");
    assert_eq!(
        materialize(actor.public_key(), &edit, Some(&nested), 2),
        Err(WriteIntentError::from(expected))
    );
}

#[test]
fn oversized_source_can_be_reduced_to_a_bounded_output() {
    let actor = Keys::generate();
    let event_id = EventId::from_byte_array([15; 32]);
    let edit = unbookmark_event(event_id).expect("unbookmark edit");
    let source = source(
        &actor,
        Kind::from_u16(BOOKMARK_KIND),
        1,
        "",
        (0..2_001).map(|_| Tag::event(event_id)).collect(),
    );
    assert!(source.as_json().len() > 131_072);

    let output = materialize(actor.public_key(), &edit, Some(&source), 2)
        .expect("bookmark semantics reduce the hostile source before generic output bounds");
    assert!(output.tags.is_empty());
}

#[test]
fn insertion_is_decided_before_the_builder_enforces_its_tag_bound() {
    let actor = Keys::generate();
    let event_id = EventId::from_byte_array([14; 32]);
    let edit = bookmark_event(event_id).expect("bookmark edit");
    let mut at_cap: Vec<_> = (0..1_999)
        .map(|index| tag(&["x", &index.to_string()]))
        .collect();
    at_cap.push(tag(&["e", &event_id.to_hex(), "hint"]));
    let existing = source(&actor, Kind::from_u16(BOOKMARK_KIND), 1, "", at_cap);
    let retained =
        materialize(actor.public_key(), &edit, Some(&existing), 2).expect("no insertion at cap");
    assert_eq!(retained.tags.len(), 2_000);

    let full_without_target = source(
        &actor,
        Kind::from_u16(BOOKMARK_KIND),
        1,
        "",
        (0..2_000)
            .map(|index| tag(&["x", &index.to_string()]))
            .collect(),
    );
    assert_eq!(
        materialize(actor.public_key(), &edit, Some(&full_without_target), 2),
        Err(WriteIntentError::TooManyTags {
            actual: 2_001,
            maximum: 2_000,
        })
    );
}

#[test]
fn materializer_preserves_the_exact_event_builder_tag_refusal() {
    let actor = Keys::generate();
    let event_id = EventId::from_byte_array([15; 32]);
    let edit = unbookmark_event(event_id).expect("unbookmark edit");
    let source = source(
        &actor,
        Kind::from_u16(BOOKMARK_KIND),
        1,
        "opaque",
        (0..2_001)
            .map(|index| tag(&["x", &index.to_string()]))
            .collect(),
    );

    assert_eq!(
        materialize(actor.public_key(), &edit, Some(&source), 2),
        Err(WriteIntentError::TooManyTags {
            actual: 2_001,
            maximum: 2_000,
        })
    );
}

fn assert_source_refusals(actor: &Keys, edit: &ReplaceableEventEdit) {
    let other_actor = Keys::generate();
    let wrong_actor = source(
        &other_actor,
        Kind::from_u16(BOOKMARK_KIND),
        1,
        "private",
        Vec::new(),
    );
    let wrong_kind = source(actor, Kind::ContactList, 1, "private", Vec::new());
    assert!(matches!(
        materialize(actor.public_key(), edit, Some(&wrong_actor), 2),
        Err(WriteIntentError::InvalidEvent(_))
    ));
    assert!(matches!(
        materialize(actor.public_key(), edit, Some(&wrong_kind), 2),
        Err(WriteIntentError::InvalidEvent(_))
    ));

    let mut tampered = source(
        actor,
        Kind::from_u16(BOOKMARK_KIND),
        1,
        "ciphertext",
        Vec::new(),
    );
    tampered.content = "tampered-private-content".to_owned();
    assert!(matches!(
        materialize(actor.public_key(), edit, Some(&tampered), 2),
        Err(WriteIntentError::InvalidEvent(_))
    ));
}

fn assert_codec_and_target_refusals(actor: &Keys, event_id: EventId, edit: &ReplaceableEventEdit) {
    let malformed = ReplaceableEventEdit::new(Kind::from_u16(BOOKMARK_KIND), None, vec![255])
        .expect("bounded malformed edit");
    assert!(!materializer().supports(&malformed));
    assert!(matches!(
        materialize(actor.public_key(), &malformed, None, 1),
        Err(WriteIntentError::Encoding(_))
    ));
    let addressable = ReplaceableEventEdit::new(
        Kind::from_u16(30_023),
        Some("article".to_owned()),
        edit.change().to_vec(),
    )
    .expect("neutral addressable edit");
    assert!(!materializer().supports(&addressable));
    let mut legacy_versioned = vec![1];
    legacy_versioned.extend_from_slice(edit.change());
    let legacy = ReplaceableEventEdit::new(Kind::from_u16(BOOKMARK_KIND), None, legacy_versioned)
        .expect("bounded legacy bytes");
    assert!(!materializer().supports(&legacy));
    assert!(matches!(
        materialize(actor.public_key(), &legacy, None, 1),
        Err(WriteIntentError::Encoding(_))
    ));

    let invalid_coordinate = EventCoordinate::Event(event_id);
    assert!(matches!(
        bookmark_coordinate(invalid_coordinate),
        Err(WriteIntentError::InvalidEvent(_))
    ));
    let large_coordinate = EventCoordinate::Replaceable {
        author: actor.public_key(),
        kind: Kind::from_u16(30_023),
        identifier: Some("x".repeat(70_000)),
    };
    assert!(bookmark_coordinate(large_coordinate).is_ok());
}

fn assert_size_and_timestamp_refusals(actor: &Keys, edit: &ReplaceableEventEdit) {
    let too_many = source(
        actor,
        Kind::from_u16(BOOKMARK_KIND),
        1,
        "opaque",
        (0..2_001)
            .map(|index| tag(&["x", &index.to_string()]))
            .collect(),
    );
    assert_eq!(
        materialize(actor.public_key(), edit, Some(&too_many), 2),
        Err(WriteIntentError::TooManyTags {
            actual: 2_002,
            maximum: 2_000,
        })
    );
    let too_large = source(
        actor,
        Kind::from_u16(BOOKMARK_KIND),
        1,
        &"x".repeat(140_000),
        Vec::new(),
    );
    assert!(matches!(
        materialize(actor.public_key(), edit, Some(&too_large), 2),
        Err(WriteIntentError::TooLarge { .. })
    ));
    let latest = source(
        actor,
        Kind::from_u16(BOOKMARK_KIND),
        u64::MAX,
        "opaque",
        Vec::new(),
    );
    assert!(matches!(
        materialize(actor.public_key(), edit, Some(&latest), u64::MAX),
        Err(WriteIntentError::InvalidEvent(_))
    ));
}
