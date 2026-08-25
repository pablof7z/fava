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
fn discovery_queries_are_ordinary_canonical_queries() {
    let alice = Keys::generate().public_key();
    let bob = Keys::generate().public_key();
    let canonical = BTreeSet::from([alice, bob]);
    let p = SingleLetterTag::LOWERCASE_P;

    let saved = SimpleGroups::saved_simple_groups([bob, alice, bob]).expect("bounded authors");
    let reordered = SimpleGroups::saved_simple_groups([alice, bob]).expect("bounded authors");
    let relays = SimpleGroups::saved_relays([alice, bob]).expect("bounded authors");
    assert_eq!(saved, reordered);
    assert_eq!(
        saved.selection().kinds,
        Some(BTreeSet::from([Kind::from_u16(10_009)]))
    );
    assert_eq!(saved.selection().authors.as_ref(), Some(&canonical));
    assert_eq!(relays, saved);

    let admins =
        SimpleGroups::simple_groups_where_admin([bob, alice, bob]).expect("bounded subjects");
    let members = SimpleGroups::simple_groups_where_member([alice, bob]).expect("bounded subjects");
    assert_eq!(
        admins.selection().kinds,
        Some(BTreeSet::from([Kind::from_u16(39_001)]))
    );
    assert_eq!(
        members.selection().kinds,
        Some(BTreeSet::from([Kind::from_u16(39_002)]))
    );
    let subject_hex = canonical.iter().map(PublicKey::to_hex).collect();
    assert_eq!(admins.selection().tag_values.get(&p), Some(&subject_hex));
    assert_eq!(members.selection().tag_values.get(&p), Some(&subject_hex));

    let empty_saved =
        SimpleGroups::saved_simple_groups(Vec::<PublicKey>::new()).expect("empty is valid");
    let empty_admins =
        SimpleGroups::simple_groups_where_admin(Vec::<PublicKey>::new()).expect("empty is valid");
    assert_eq!(empty_saved.selection().authors, Some(BTreeSet::new()));
    assert_eq!(
        empty_admins.selection().tag_values.get(&p),
        Some(&BTreeSet::new())
    );
    assert_eq!(
        SimpleGroups::saved_simple_groups([alice]).unwrap(),
        SimpleGroups::saved_simple_groups([alice]).unwrap()
    );

    let signing_keys = Keys::generate();
    let simple_group = SimpleGroup::on([relay("wss://a.example")], "photos").expect("group");
    let snapshot = QuerySnapshot::evaluated(
        vec![record(saved_event(
            &signing_keys,
            9,
            vec![
                tag(&["group", "photos", "wss://a.example"]),
                tag(&["group", "photos", "wss://a.example", "duplicate"]),
            ],
        ))],
        &[],
    );
    assert_eq!(
        SimpleGroups::simple_groups_saved_by(&snapshot, &simple_group).expect("pure projection"),
        [signing_keys.public_key()]
    );
}

struct PanicAfter {
    value: PublicKey,
    pulls: Rc<Cell<usize>>,
}

impl Iterator for PanicAfter {
    type Item = PublicKey;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.pulls.get() + 1;
        assert!(next <= 257, "discovery consumed beyond bound+1");
        self.pulls.set(next);
        Some(self.value)
    }
}

#[test]
fn discovery_refuses_oversized_and_infinite_inputs() {
    let key = Keys::generate().public_key();
    assert!(SimpleGroups::saved_simple_groups(std::iter::repeat_n(key, 256)).is_ok());
    assert_eq!(
        SimpleGroups::saved_simple_groups(std::iter::repeat_n(key, 257)),
        Err(SimpleGroupError::TooManyDiscoveryItems {
            actual: 257,
            maximum: 256
        })
    );

    let pulls = Rc::new(Cell::new(0));
    assert_eq!(
        SimpleGroups::simple_groups_where_member(PanicAfter {
            value: key,
            pulls: Rc::clone(&pulls)
        }),
        Err(SimpleGroupError::TooManyDiscoveryItems {
            actual: 257,
            maximum: 256
        })
    );
    assert_eq!(pulls.get(), 257);
}

#[test]
fn groups_saved_by_is_bounded_pure_projection() {
    let alice = Keys::generate();
    let bob = Keys::generate();
    let other = Keys::generate();
    let simple_group = SimpleGroup::on(
        [relay("wss://a.example"), relay("wss://b.example")],
        "photos",
    )
    .expect("group");
    let snapshot = QuerySnapshot::evaluated(
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

    let first =
        SimpleGroups::simple_groups_saved_by(&snapshot, &simple_group).expect("bounded projection");
    let second =
        SimpleGroups::simple_groups_saved_by(&snapshot, &simple_group).expect("pure repeat");
    assert_eq!(first, expected);
    assert_eq!(second, first);
}

fn materialize(
    materializer: &dyn ReplaceableEventMaterializer,
    edit: &fava_write::ReplaceableEventEdit,
    source: Option<&fava_write::Event>,
) -> fava_write::UnsignedEvent {
    let source = source.cloned().map(EventValue::Signed);
    SimpleGroups::materializer()
        .materialize(edit, author, source.as_ref(), Timestamp::from(created_at))
        .expect("saved-list materializes")
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
    let group = SimpleGroup::from_relays("photos", vec![relay_a.clone(), relay_b.clone()])
        .expect("non-empty group");
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
    assert!(
        materializer
            .materialize(
                &edit,
                actor.public_key(),
                Some(&EventValue::Signed(source.clone())),
                Timestamp::from(11),
            )
            .is_err()
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
        vec![RelayUrl::parse("wss://a.example").expect("relay")],
    )
    .expect("non-empty group");
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
            Some(&EventValue::Signed(source.clone())),
            Timestamp::from(2),
        ),
        Err(fava_write::WriteIntentError::TooManyTags {
            actual: 2_001,
            maximum: 2_000,
        })
    );
}

fn signed_source(keys: &Keys, created_at: u64, content: &str, tags: Vec<Tag>) -> fava_write::Event {
    EventBuilder::new(Kind::from_u16(10_009), content)
        .tags(tags)
        .custom_created_at(Timestamp::from(created_at))
        .finalize(keys)
        .expect("saved source signs")
}

#[test]
fn multi_host_save_is_deterministic_and_idempotent() {
    let actor = Keys::generate();
    let simple_group = SimpleGroup::on(
        [
            relay("wss://z.example"),
            relay("wss://a.example"),
            relay("wss://z.example"),
            relay("wss://m.example"),
        ],
        "photos",
    )
    .expect("bounded hosts");
    let edit =
        SimpleGroups::save_simple_group(&simple_group, Some("Photos")).expect("bounded edit");
    assert_eq!(edit.kind(), Kind::from_u16(10_009));
    assert_eq!(edit.identifier(), None);
    assert!(SimpleGroups::materializer().supports(&edit));

    let once = materialize(&edit, actor.public_key(), None, 10);
    assert_eq!(
        once.tags.as_slice(),
        &[
            tag(&["group", "photos", "wss://z.example", "Photos"]),
            tag(&["group", "photos", "wss://a.example", "Photos"]),
            tag(&["group", "photos", "wss://m.example", "Photos"]),
        ]
    );
    let source = signed_source(&actor, 10, &once.content, once.tags.clone().to_vec());
    let twice = materialize(&edit, actor.public_key(), Some(&source), 11);
    assert_eq!(twice.tags, once.tags);
    assert_eq!(twice.content, once.content);
}

#[test]
fn saved_edit_conserves_foreign_bytes_and_other_hosts() {
    let actor = Keys::generate();
    let source_tags = vec![
        tag(&["x", "before", "exact"]),
        tag(&["group", "photos", "wss://a.example", "old"]),
        tag(&["group", "photos", "wss://c.example", "other-host"]),
        tag(&["group", "photos", "wss://a.example", "duplicate"]),
        tag(&["group", "photos", "wss://a.example", "extension", "kept"]),
        tag(&["r", "wss://saved.example"]),
        tag(&["x", "after"]),
    ];
    let source = signed_source(&actor, 20, "opaque encrypted bytes", source_tags.clone());
    let simple_group = SimpleGroup::on(
        [relay("wss://a.example"), relay("wss://b.example")],
        "photos",
    )
    .expect("group");
    let rename =
        SimpleGroups::rename_saved_simple_group(&simple_group, "renamed").expect("rename edit");
    let renamed = materialize(&rename, actor.public_key(), Some(&source), 21);

    assert_eq!(renamed.content, "opaque encrypted bytes");
    assert_eq!(
        renamed.tags.as_slice(),
        &[
            source_tags[0].clone(),
            tag(&["group", "photos", "wss://a.example", "renamed"]),
            source_tags[2].clone(),
            source_tags[4].clone(),
            source_tags[5].clone(),
            source_tags[6].clone(),
            tag(&["group", "photos", "wss://b.example", "renamed"]),
        ]
    );

    let save = SimpleGroups::save_simple_group(&simple_group, Some("ignored for existing"))
        .expect("save edit");
    let saved = materialize(&save, actor.public_key(), Some(&source), 21);
    assert_eq!(saved.tags[1], source_tags[1]);
    assert_eq!(
        saved.tags.last(),
        Some(&tag(&[
            "group",
            "photos",
            "wss://b.example",
            "ignored for existing",
        ]))
    );
    assert_eq!(
        saved
            .tags
            .iter()
            .filter(|tag| tag.as_slice().get(2).map(String::as_str) == Some("wss://a.example"))
            .count(),
        2,
        "one valid row plus the malformed extension row survive"
    );

    let remove = SimpleGroups::remove_simple_group(&simple_group).expect("remove edit");
    let removed = materialize(&remove, actor.public_key(), Some(&source), 21);
    assert_eq!(
        removed.tags.as_slice(),
        &[
            source_tags[0].clone(),
            source_tags[2].clone(),
            source_tags[4].clone(),
            source_tags[5].clone(),
            source_tags[6].clone(),
        ]
    );

    let saved_relay = relay("wss://saved.example");
    let remove_relay = SimpleGroups::remove_relay(saved_relay.clone()).expect("relay edit");
    let without_relay = materialize(&remove_relay, actor.public_key(), Some(&source), 21);
    assert!(!without_relay.tags.contains(&source_tags[5]));
    let add_relay = SimpleGroups::save_relay(saved_relay).expect("relay edit");
    let relay_once = materialize(&add_relay, actor.public_key(), Some(&source), 21);
    assert_eq!(relay_once.tags, source.tags);
}

#[test]
fn saved_edit_rebases_on_newer_qualified_source() {
    let actor = Keys::generate();
    let simple_group = SimpleGroup::on([relay("wss://a.example")], "photos").expect("group");
    let edit = SimpleGroups::save_simple_group(&simple_group, None).expect("save edit");
    let older = signed_source(&actor, 30, "older opaque", vec![tag(&["x", "older"])]);
    let newer = signed_source(
        &actor,
        31,
        "newer opaque",
        vec![tag(&["x", "newer"]), tag(&["r", "wss://kept.example"])],
    );
    let first = materialize(&edit, actor.public_key(), Some(&older), 32);
    let rebased = materialize(&edit, actor.public_key(), Some(&newer), 33);

    assert_eq!(first.content, "older opaque");
    assert_eq!(rebased.content, "newer opaque");
    assert_eq!(
        rebased.tags.as_slice(),
        &[
            tag(&["x", "newer"]),
            tag(&["r", "wss://kept.example"]),
            tag(&["group", "photos", "wss://a.example"]),
        ]
    );
    assert!(SimpleGroups::save_simple_group(&simple_group, Some(&"x".repeat(4_097))).is_err());
    assert!(
        SimpleGroups::materializer()
            .materialize(
                &edit,
                Keys::generate().public_key(),
                Some(&EventValue::Signed(newer.clone())),
                Timestamp::from(33),
            )
            .is_err()
    );
    assert!(
        SimpleGroups::materializer()
            .materialize(
                &edit,
                actor.public_key(),
                Some(&EventValue::Signed(newer.clone())),
                Timestamp::from(31),
            )
            .is_err()
    );
}
