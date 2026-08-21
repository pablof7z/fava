use fava_state::EventCoordinate;
use fava_write::{Kind, ReplaceableEventEdit, Tag, Timestamp, WriteIntentError};
use nostr::event::{EventBuilder, FinalizeEvent};
use nostr::key::{Keys, PublicKey};

use super::{follow, materializer, unfollow};

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

fn target_tags(event: &fava_write::UnsignedEvent, target: PublicKey) -> usize {
    event
        .tags
        .iter()
        .filter(|tag| {
            let values = tag.as_slice();
            values.first().map(String::as_str) == Some("p")
                && values
                    .get(1)
                    .and_then(|value| PublicKey::from_hex(value).ok())
                    == Some(target)
        })
        .count()
}

#[test]
fn follow_empty_and_unfollow_inverse() {
    let actor = Keys::generate();
    let target = Keys::generate().public_key();
    let follow_edit = follow(actor.public_key(), target).expect("follow edit");
    let unfollow_edit = unfollow(actor.public_key(), target).expect("unfollow edit");

    assert_eq!(follow_edit.actor(), actor.public_key());
    assert_eq!(follow_edit.inverse(), unfollow_edit);
    assert_eq!(unfollow_edit.inverse(), follow_edit);
    assert_eq!(
        follow_edit.coordinate(),
        &EventCoordinate::Replaceable {
            author: actor.public_key(),
            kind: Kind::ContactList,
            identifier: None,
        }
    );
    let first = materialize(&follow_edit, None, 7).expect("first list");
    assert_eq!(first.pubkey, actor.public_key());
    assert_eq!(first.kind, Kind::ContactList);
    assert_eq!(first.created_at, Timestamp::from(7));
    assert_eq!(first.content, "");
    assert_eq!(first.tags.as_slice(), &[tag(&["p", &target.to_hex()])]);

    let signed = source(&actor, Kind::ContactList, 7, "opaque", first.tags.to_vec());
    let empty = materialize(&unfollow_edit, Some(&signed), 8).expect("inverse applies");
    assert!(empty.tags.is_empty());
    assert_eq!(empty.content, "opaque");
}

#[test]
fn follow_preserves_unrelated_state_and_orders_deterministically() {
    let actor = Keys::generate();
    let target = Keys::generate().public_key();
    let other = Keys::generate().public_key();
    let kept = tag(&["p", &target.to_hex(), "wss://hint.example", "petname"]);
    let tags = vec![
        tag(&["t", "first"]),
        kept.clone(),
        tag(&["x", "future", "bytes"]),
        tag(&["p", &other.to_hex(), "wss://other.example"]),
        tag(&["p", &target.to_hex(), "wss://duplicate.example"]),
        tag(&["p", "not-a-public-key", "malformed"]),
    ];
    let source = source(
        &actor,
        Kind::ContactList,
        10,
        "opaque-content",
        tags.clone(),
    );
    let edit = follow(actor.public_key(), target).expect("follow edit");

    let first = materialize(&edit, Some(&source), 11).expect("follow applies");
    let second = materialize(&edit, Some(&source), 11).expect("same input is deterministic");
    assert_eq!(first, second);
    assert_eq!(first.content, "opaque-content");
    assert_eq!(
        first.tags.as_slice(),
        &[
            tags[0].clone(),
            kept,
            tags[2].clone(),
            tags[3].clone(),
            tags[5].clone(),
        ]
    );
}

#[test]
fn follow_duplicate_and_adjacent_edits_are_idempotent() {
    let actor = Keys::generate();
    let target = Keys::generate().public_key();
    let target_hex = target.to_hex();
    let tags = vec![
        tag(&["p", &target_hex, "first"]),
        tag(&["p", &target_hex, "middle"]),
        tag(&["x", "kept"]),
        tag(&["p", &target_hex, "last"]),
    ];
    let original = source(&actor, Kind::ContactList, 20, "", tags);
    let add = follow(actor.public_key(), target).expect("follow edit");
    let once = materialize(&add, Some(&original), 21).expect("deduplicates");
    assert_eq!(target_tags(&once, target), 1);
    assert_eq!(once.tags[0].as_slice(), &["p", &target_hex, "first"]);

    let signed_once = source(
        &actor,
        Kind::ContactList,
        21,
        "",
        once.tags.clone().to_vec(),
    );
    let twice = materialize(&add, Some(&signed_once), 22).expect("repeat is idempotent");
    assert_eq!(once.tags, twice.tags);

    let remove = unfollow(actor.public_key(), target).expect("unfollow edit");
    let removed = materialize(&remove, Some(&original), 21).expect("all adjacent matches removed");
    assert_eq!(removed.tags.as_slice(), &[tag(&["x", "kept"])]);
    let signed_removed = source(
        &actor,
        Kind::ContactList,
        21,
        "",
        removed.tags.clone().to_vec(),
    );
    let removed_twice = materialize(&remove, Some(&signed_removed), 22).expect("repeat removal");
    assert_eq!(removed.tags, removed_twice.tags);
}

#[test]
fn follow_bounds_and_invalid_sources_are_typed_refusals() {
    let actor = Keys::generate();
    let other_actor = Keys::generate();
    let target = Keys::generate().public_key();
    let edit = follow(actor.public_key(), target).expect("follow edit");
    let wrong_actor = source(&other_actor, Kind::ContactList, 1, "", Vec::new());
    let wrong_kind = source(&actor, Kind::from_u16(10_003), 1, "", Vec::new());
    assert!(matches!(
        materialize(&edit, Some(&wrong_actor), 2),
        Err(WriteIntentError::InvalidEvent(_))
    ));
    assert!(matches!(
        materialize(&edit, Some(&wrong_kind), 2),
        Err(WriteIntentError::InvalidEvent(_))
    ));

    let mut tampered = source(&actor, Kind::ContactList, 1, "original", Vec::new());
    tampered.content = "tampered".to_owned();
    assert!(matches!(
        materialize(&edit, Some(&tampered), 2),
        Err(WriteIntentError::InvalidEvent(_))
    ));

    let coordinate = edit.coordinate().clone();
    let wrong_format = ReplaceableEventEdit::new(
        actor.public_key(),
        coordinate.clone(),
        edit.format() + 1,
        edit.change().to_vec(),
        edit.inverse_change().to_vec(),
    )
    .expect("structural edit");
    assert!(!materializer().supports(&wrong_format));
    assert!(materialize(&wrong_format, None, 1).is_err());
    let malformed = ReplaceableEventEdit::new(
        actor.public_key(),
        coordinate,
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

    let too_many = source(
        &actor,
        Kind::ContactList,
        1,
        "",
        (0..2_001)
            .map(|index| tag(&["x", &index.to_string()]))
            .collect(),
    );
    assert!(matches!(
        materialize(&edit, Some(&too_many), 2),
        Err(WriteIntentError::TooLarge { .. })
    ));
    let too_large = source(
        &actor,
        Kind::ContactList,
        1,
        &"x".repeat(140_000),
        Vec::new(),
    );
    assert!(matches!(
        materialize(&edit, Some(&too_large), 2),
        Err(WriteIntentError::TooLarge { .. })
    ));
    let latest = source(&actor, Kind::ContactList, u64::MAX, "", Vec::new());
    assert!(matches!(
        materialize(&edit, Some(&latest), u64::MAX),
        Err(WriteIntentError::InvalidEvent(_))
    ));
}
