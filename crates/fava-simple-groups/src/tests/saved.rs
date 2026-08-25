use fava_state::RelayUrl;
use fava_write::{EventValue, Kind, ReplaceableEventMaterializer, Tag, Timestamp};
use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
use nostr::key::Keys;

use crate::{
    SavedGroupList, SavedGroupListDecodeError, SimpleGroup, remove_saved_relay,
    remove_saved_simple_group, rename_saved_simple_group, save_relay, save_simple_group,
    saved_group_list_materializer,
};

use super::{public_key, tag, value};

#[test]
fn saved_list_decodes_both_tag_families_once_and_tolerantly() {
    let list = SavedGroupList::from_event(&value(
        10_009,
        vec![
            tag(&["group", "", "wss://a.example", "", "ignored"]),
            tag(&["r", "wss://b.example", "ignored"]),
            tag(&["group", "missing-relay"]),
            tag(&["r"]),
            tag(&["group", "", "wss://a.example"]),
            tag(&["x", "ignored"]),
        ],
    ))
    .expect("saved list");

    assert_eq!(list.author(), public_key());
    assert_eq!(list.simple_groups().len(), 3);
    let first = list.simple_groups()[0].as_ref().expect("first group");
    assert_eq!(first.id(), "");
    assert_eq!(first.relay().as_str(), "wss://a.example");
    assert_eq!(first.display_name(), Some(""));
    assert!(matches!(
        list.simple_groups()[1],
        Err(SavedGroupListDecodeError::MissingTagValue {
            tag_index: 2,
            value_index: 2
        })
    ));
    assert_eq!(
        list.simple_groups()[2].as_ref().unwrap().relay().as_str(),
        "wss://a.example"
    );
    assert_eq!(list.relays().len(), 2);
    assert_eq!(
        list.relays()[0].as_ref().unwrap().as_str(),
        "wss://b.example"
    );
    assert!(matches!(
        list.relays()[1],
        Err(SavedGroupListDecodeError::MissingTagValue {
            tag_index: 3,
            value_index: 1
        })
    ));
}

#[test]
fn saved_list_wrong_kind_is_a_whole_event_failure() {
    assert_eq!(
        SavedGroupList::from_event(&value(1, vec![])),
        Err(SavedGroupListDecodeError::WrongEventKind {
            expected: Kind::from_u16(10_009),
            actual: Kind::from_u16(1),
        })
    );
}

fn source(keys: &Keys) -> fava_write::Event {
    NostrEventBuilder::new(Kind::from_u16(10_009), "opaque")
        .tags([
            Tag::parse(["x", "preserved"]).unwrap(),
            Tag::parse(["group", "photos", "wss://a.example", "Old", "tail"]).unwrap(),
            Tag::parse(["group", "photos", "wss://a.example", "duplicate"]).unwrap(),
            Tag::parse(["r", "wss://a.example", "tail"]).unwrap(),
            Tag::parse(["r", "wss://a.example"]).unwrap(),
        ])
        .custom_created_at(Timestamp::from(1))
        .finalize(keys)
        .expect("signed source")
}

fn materialize(
    materializer: &dyn ReplaceableEventMaterializer,
    edit: &fava_write::ReplaceableEventEdit,
    source: Option<&fava_write::Event>,
) -> fava_write::UnsignedEvent {
    let author = source.map_or_else(public_key, |event| event.pubkey);
    materializer
        .materialize(edit, author, source, Timestamp::from(2))
        .expect("materialized edit")
}

fn has_tag(tags: &[Tag], expected: &[&str]) -> bool {
    tags.iter().any(|tag| {
        tag.as_slice()
            .iter()
            .map(String::as_str)
            .eq(expected.iter().copied())
    })
}

#[test]
fn saved_edits_preserve_unrelated_and_unused_values() {
    let keys = Keys::generate();
    let source = source(&keys);
    let relay_a = RelayUrl::parse("wss://a.example").unwrap();
    let relay_b = RelayUrl::parse("wss://b.example").unwrap();
    let group = SimpleGroup::from_relays("photos", relay_a.clone(), vec![relay_b.clone()]);
    let materializer = saved_group_list_materializer();

    let renamed = materialize(
        materializer.as_ref(),
        &rename_saved_simple_group(&group, "Renamed").unwrap(),
        Some(&source),
    );
    assert_eq!(renamed.content, "opaque");
    assert!(has_tag(&renamed.tags, &["x", "preserved"]));
    assert!(has_tag(
        &renamed.tags,
        &["group", "photos", "wss://a.example", "Renamed", "tail"]
    ));
    assert!(has_tag(
        &renamed.tags,
        &["group", "photos", "wss://b.example", "Renamed"]
    ));
    assert_eq!(
        renamed
            .tags
            .iter()
            .filter(|tag| tag.as_slice().first().map(String::as_str) == Some("group"))
            .count(),
        2
    );

    let saved = materialize(
        materializer.as_ref(),
        &save_simple_group(&group, Some("Ignored for existing")).unwrap(),
        Some(&source),
    );
    assert!(has_tag(
        &saved.tags,
        &["group", "photos", "wss://a.example", "Old", "tail"]
    ));
    assert!(has_tag(
        &saved.tags,
        &["group", "photos", "wss://b.example", "Ignored for existing"]
    ));

    let removed = materialize(
        materializer.as_ref(),
        &remove_saved_simple_group(&group).unwrap(),
        Some(&source),
    );
    assert!(!removed.tags.iter().any(|tag| {
        let values = tag.as_slice();
        values.first().map(String::as_str) == Some("group")
            && values.get(1).map(String::as_str) == Some("photos")
    }));
}

#[test]
fn relay_edits_match_semantic_first_values_and_keep_one() {
    let keys = Keys::generate();
    let source = source(&keys);
    let relay_a = RelayUrl::parse("wss://a.example").unwrap();
    let relay_b = RelayUrl::parse("wss://b.example").unwrap();
    let materializer = saved_group_list_materializer();

    let saved = materialize(
        materializer.as_ref(),
        &save_relay(relay_a.clone()).unwrap(),
        Some(&source),
    );
    assert_eq!(
        saved
            .tags
            .iter()
            .filter(|tag| {
                let values = tag.as_slice();
                values.first().map(String::as_str) == Some("r")
                    && values.get(1).map(String::as_str) == Some("wss://a.example")
            })
            .count(),
        1
    );
    assert!(has_tag(&saved.tags, &["r", "wss://a.example", "tail"]));

    let added = materialize(
        materializer.as_ref(),
        &save_relay(relay_b).unwrap(),
        Some(&source),
    );
    assert!(has_tag(&added.tags, &["r", "wss://b.example"]));

    let removed = materialize(
        materializer.as_ref(),
        &remove_saved_relay(relay_a).unwrap(),
        Some(&source),
    );
    assert!(!removed.tags.iter().any(|tag| {
        tag.as_slice().first().map(String::as_str) == Some("r")
            && tag.as_slice().get(1).map(String::as_str) == Some("wss://a.example")
    }));
}

#[test]
fn materializer_supports_only_its_private_edits() {
    let group = SimpleGroup::from_relays(
        "photos",
        RelayUrl::parse("wss://a.example").expect("relay"),
        Vec::new(),
    );
    let edit = save_simple_group(&group, None).unwrap();
    let materializer = saved_group_list_materializer();
    assert_eq!(materializer.kind(), Kind::from_u16(10_009));
    assert!(materializer.supports(&edit));

    let decoded = SavedGroupList::from_event(&EventValue::Unsigned(materialize(
        materializer.as_ref(),
        &edit,
        None,
    )))
    .unwrap();
    assert_eq!(decoded.simple_groups()[0].as_ref().unwrap().id(), "photos");
}

#[test]
fn materializer_preserves_the_exact_event_builder_tag_refusal() {
    let keys = Keys::generate();
    let source = NostrEventBuilder::new(Kind::from_u16(10_009), "opaque")
        .tags((0..2_001).map(|index| Tag::parse(["x", &index.to_string()]).expect("ordinary tag")))
        .custom_created_at(Timestamp::from(1))
        .finalize(&keys)
        .expect("source signs");
    let relay = RelayUrl::parse("wss://absent.example").expect("relay");
    let edit = remove_saved_relay(relay).expect("remove relay edit");

    assert_eq!(
        saved_group_list_materializer().materialize(
            &edit,
            keys.public_key(),
            Some(&source),
            Timestamp::from(2),
        ),
        Err(fava_write::WriteIntentError::TooManyTags {
            actual: 2_001,
            maximum: 2_000,
        })
    );
}
