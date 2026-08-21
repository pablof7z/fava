use fava_state::EventCoordinate;
use fava_write::{EventId, Kind, ReplaceableEventEdit, Tag, Timestamp, WriteIntentError};
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
    edit: &ReplaceableEventEdit,
    source: Option<&fava_write::Event>,
    created_at: u64,
) -> Result<fava_write::UnsignedEvent, WriteIntentError> {
    materializer().materialize(edit, source, Timestamp::from(created_at))
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
fn bookmark_empty_and_unbookmark_inverse() {
    let actor = Keys::generate();
    let event_id = EventId::from_byte_array([7; 32]);
    let coordinate = article_coordinate();
    let add_event = bookmark_event(actor.public_key(), event_id).expect("event edit");
    let remove_event = unbookmark_event(actor.public_key(), event_id).expect("event inverse");
    assert_eq!(add_event.inverse(), remove_event);
    assert_eq!(remove_event.inverse(), add_event);
    assert_eq!(
        add_event.coordinate(),
        &EventCoordinate::Replaceable {
            author: actor.public_key(),
            kind: Kind::from_u16(BOOKMARK_KIND),
            identifier: None,
        }
    );
    let first = materialize(&add_event, None, 4).expect("first public bookmark list");
    assert_eq!(first.pubkey, actor.public_key());
    assert_eq!(first.kind, Kind::from_u16(BOOKMARK_KIND));
    assert_eq!(first.tags.as_slice(), &[tag(&["e", &event_id.to_hex()])]);
    assert_eq!(first.content, "");

    let add_coordinate =
        bookmark_coordinate(actor.public_key(), coordinate.clone()).expect("coordinate edit");
    assert_eq!(
        add_coordinate.inverse(),
        unbookmark_coordinate(actor.public_key(), coordinate.clone()).expect("coordinate inverse")
    );
    let coordinate_event = materialize(&add_coordinate, None, 4).expect("coordinate bookmark");
    assert_eq!(
        coordinate_event.tags.as_slice(),
        &[tag(&["a", &coordinate_text(&coordinate)])]
    );
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
    let edit = bookmark_event(actor.public_key(), event_id).expect("bookmark edit");
    let first = materialize(&edit, Some(&source), 11).expect("bookmark applies");
    let second = materialize(&edit, Some(&source), 11).expect("deterministic repeat");
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
    let add = bookmark_event(actor.public_key(), event_id).expect("bookmark edit");
    let once = materialize(&add, Some(&original), 21).expect("event deduplicates");
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
    let twice = materialize(&add, Some(&signed_once), 22).expect("event repeat idempotent");
    assert_eq!(once.tags, twice.tags);

    let remove_coordinate =
        unbookmark_coordinate(actor.public_key(), coordinate).expect("coordinate removal");
    let removed = materialize(&remove_coordinate, Some(&original), 21).expect("all a tags removed");
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
    let removed_twice =
        materialize(&remove_coordinate, Some(&signed_removed), 22).expect("repeat removal");
    assert_eq!(removed.tags, removed_twice.tags);
}

#[test]
fn bookmark_bounds_private_and_invalid_sources_are_typed_refusals() {
    let actor = Keys::generate();
    let event_id = EventId::from_byte_array([11; 32]);
    let edit = bookmark_event(actor.public_key(), event_id).expect("bookmark edit");
    assert_source_refusals(&actor, &edit);
    assert_codec_and_target_refusals(&actor, event_id, &edit);
    assert_size_and_timestamp_refusals(&actor, &edit);
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
        materialize(edit, Some(&wrong_actor), 2),
        Err(WriteIntentError::InvalidEvent(_))
    ));
    assert!(matches!(
        materialize(edit, Some(&wrong_kind), 2),
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
        materialize(edit, Some(&tampered), 2),
        Err(WriteIntentError::InvalidEvent(_))
    ));
}

fn assert_codec_and_target_refusals(actor: &Keys, event_id: EventId, edit: &ReplaceableEventEdit) {
    let wrong_format = ReplaceableEventEdit::new(
        actor.public_key(),
        edit.coordinate().clone(),
        edit.format() + 1,
        edit.change().to_vec(),
        edit.inverse_change().to_vec(),
    )
    .expect("structural edit");
    assert!(!materializer().supports(&wrong_format));
    assert!(materialize(&wrong_format, None, 1).is_err());
    let malformed = ReplaceableEventEdit::new(
        actor.public_key(),
        edit.coordinate().clone(),
        edit.format(),
        vec![255],
        edit.inverse_change().to_vec(),
    )
    .expect("bounded malformed edit");
    assert!(!materializer().supports(&malformed));
    assert!(matches!(
        materialize(&malformed, None, 1),
        Err(WriteIntentError::Encoding(_))
    ));

    let invalid_coordinate = EventCoordinate::Event(event_id);
    assert!(matches!(
        bookmark_coordinate(actor.public_key(), invalid_coordinate),
        Err(WriteIntentError::InvalidEvent(_))
    ));
    let oversized_coordinate = EventCoordinate::Replaceable {
        author: actor.public_key(),
        kind: Kind::from_u16(30_023),
        identifier: Some("x".repeat(70_000)),
    };
    assert!(matches!(
        bookmark_coordinate(actor.public_key(), oversized_coordinate),
        Err(WriteIntentError::TooLarge { .. })
    ));
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
    assert!(matches!(
        materialize(edit, Some(&too_many), 2),
        Err(WriteIntentError::TooLarge { .. })
    ));
    let too_large = source(
        actor,
        Kind::from_u16(BOOKMARK_KIND),
        1,
        &"x".repeat(140_000),
        Vec::new(),
    );
    assert!(matches!(
        materialize(edit, Some(&too_large), 2),
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
        materialize(edit, Some(&latest), u64::MAX),
        Err(WriteIntentError::InvalidEvent(_))
    ));
}
