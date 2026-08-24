use fava_write::{Event, EventBuilder as FavaEventBuilder, EventValue, Kind, Tag, Timestamp};
use nostr::event::{EventBuilder, FinalizeEvent};
use nostr::key::Keys;

use crate::{
    SimpleGroupAdmins, SimpleGroupError, SimpleGroupMembers, SimpleGroupMetadata, SimpleGroupParticipants, SimpleGroupPins, SimpleGroupRoles,
    PinnedItem, SavedSimpleGroup, SavedRelay,
};

fn tag(values: &[&str]) -> Tag {
    Tag::parse(values.iter().copied()).expect("test tag")
}

fn source(keys: &Keys, kind: u16, tags: Vec<Tag>, content: &str) -> Event {
    EventBuilder::new(Kind::from_u16(kind), content)
        .tags(tags)
        .custom_created_at(Timestamp::from(7))
        .finalize(keys)
        .expect("test event signs")
}

#[test]
fn metadata_parser_conserves_complete_record() {
    let keys = Keys::generate();
    let event = source(
        &keys,
        39_000,
        vec![
            tag(&["d", " photos "]),
            tag(&["name", "Pizza Lovers"]),
            tag(&["picture", "https://groups.example/picture.png"]),
            tag(&["banner", "https://groups.example/banner.png"]),
            tag(&["about", "A\u{301} exact"]),
            tag(&["private"]),
            tag(&["restricted"]),
            tag(&["hidden"]),
            tag(&["closed"]),
            tag(&["livekit"]),
            tag(&["supported_kinds", "11", "9", "11"]),
            tag(&["parent", " root "]),
            tag(&["child", "second"]),
            tag(&["x", "uninterpreted"]),
            tag(&["child", "first"]),
        ],
        "opaque",
    );
    let metadata = SimpleGroupMetadata::from_event(&EventValue::Signed(event)).expect("metadata");

    assert_eq!(metadata.id(), " photos ");
    assert_eq!(metadata.author(), keys.public_key());
    assert_eq!(metadata.name(), Some("Pizza Lovers"));
    assert_eq!(
        metadata.picture(),
        Some("https://groups.example/picture.png")
    );
    assert_eq!(metadata.banner(), Some("https://groups.example/banner.png"));
    assert_eq!(metadata.about(), Some("A\u{301} exact"));
    assert!(metadata.is_private());
    assert!(metadata.is_restricted());
    assert!(metadata.is_hidden());
    assert!(metadata.is_closed());
    assert!(metadata.has_livekit());
    assert_eq!(
        metadata.supported_kinds(),
        Some([Kind::from_u16(11), Kind::from_u16(9), Kind::from_u16(11)].as_slice())
    );
    assert_eq!(metadata.parent(), Some(" root "));
    assert_eq!(metadata.children(), ["second", "first"]);

    let absent = source(&keys, 39_000, vec![tag(&["d", "g"])], "");
    let empty = source(
        &keys,
        39_000,
        vec![tag(&["d", "g"]), tag(&["supported_kinds"])],
        "",
    );
    assert_eq!(
        SimpleGroupMetadata::from_event(&EventValue::Signed(absent))
            .expect("absent supported kinds")
            .supported_kinds(),
        None
    );
    assert_eq!(
        SimpleGroupMetadata::from_event(&EventValue::Signed(empty))
            .expect("present-empty supported kinds")
            .supported_kinds(),
        Some([].as_slice())
    );
}

#[test]
#[allow(clippy::too_many_lines)] // One hostile boundary matrix keeps E25 causality explicit.
fn record_boundary_refuses_ambiguous_or_oversized_input() {
    let keys = Keys::generate();
    let minimal = source(&keys, 39_000, vec![tag(&["d", "g"])], "");
    assert_eq!(
        SimpleGroupMetadata::from_event(&EventValue::Signed(minimal))
            .expect("narrow valid record")
            .id(),
        "g"
    );

    let wrong_kind = source(&keys, 39_001, vec![tag(&["d", "g"])], "");
    assert!(matches!(
        SimpleGroupMetadata::from_event(&EventValue::Signed(wrong_kind)),
        Err(SimpleGroupError::WrongRecordKind {
            expected: 39_000,
            actual: 39_001
        })
    ));

    let unsigned = FavaEventBuilder::new(keys.public_key(), Kind::from_u16(39_000))
        .created_at(Timestamp::from(7))
        .tag(tag(&["d", "g"]))
        .build()
        .expect("bounded unsigned record");
    assert_eq!(
        SimpleGroupMetadata::from_event(&EventValue::Unsigned(unsigned)),
        Err(SimpleGroupError::UnsignedRecord)
    );

    let mut invalid_id = source(&keys, 39_000, vec![tag(&["d", "g"])], "before");
    invalid_id.content = "after".to_owned();
    assert_eq!(
        SimpleGroupMetadata::from_event(&EventValue::Signed(invalid_id)),
        Err(SimpleGroupError::InvalidRecordId)
    );

    let valid = source(&keys, 39_000, vec![tag(&["d", "g"])], "");
    let mut invalid_signature = valid.clone();
    invalid_signature.sig = "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
        .parse()
        .expect("shape-valid signature");
    assert_eq!(
        SimpleGroupMetadata::from_event(&EventValue::Signed(invalid_signature)),
        Err(SimpleGroupError::InvalidRecordSignature)
    );

    let cases = [
        (
            source(&keys, 39_000, vec![], ""),
            SimpleGroupError::MissingRecordId,
        ),
        (
            source(&keys, 39_000, vec![tag(&["d"])], ""),
            SimpleGroupError::EmptyRecordId,
        ),
        (
            source(&keys, 39_000, vec![tag(&["d", ""])], ""),
            SimpleGroupError::EmptyRecordId,
        ),
        (
            source(&keys, 39_000, vec![tag(&["d", "g"]), tag(&["d", "g"])], ""),
            SimpleGroupError::DuplicateRecordId,
        ),
        (
            source(
                &keys,
                39_000,
                vec![tag(&["d", "g"]), tag(&["d", "other"])],
                "",
            ),
            SimpleGroupError::ConflictingRecordId,
        ),
    ];
    for (event, expected) in cases {
        assert_eq!(
            SimpleGroupMetadata::from_event(&EventValue::Signed(event)),
            Err(expected)
        );
    }

    let duplicate_name = source(
        &keys,
        39_000,
        vec![
            tag(&["d", "g"]),
            tag(&["name", "first"]),
            tag(&["name", "second"]),
        ],
        "",
    );
    assert_eq!(
        SimpleGroupMetadata::from_event(&EventValue::Signed(duplicate_name)),
        Err(SimpleGroupError::AmbiguousRecordField("name"))
    );

    let too_many_tags = source(
        &keys,
        39_000,
        std::iter::once(tag(&["d", "g"]))
            .chain((0..2_000).map(|_| tag(&["x"])))
            .collect(),
        "",
    );
    assert!(matches!(
        SimpleGroupMetadata::from_event(&EventValue::Signed(too_many_tags)),
        Err(SimpleGroupError::TooManyRecordTags {
            actual: 2_001,
            maximum: 2_000
        })
    ));

    let oversized_event = source(&keys, 39_000, vec![tag(&["d", "g"])], &"x".repeat(131_073));
    assert!(matches!(
        SimpleGroupMetadata::from_event(&EventValue::Signed(oversized_event)),
        Err(SimpleGroupError::RecordTooLarge {
            maximum: 131_072,
            ..
        })
    ));

    let long = "x".repeat(4_097);
    let oversized_value = source(
        &keys,
        39_000,
        vec![tag(&["d", "g"]), tag(&["name", &long])],
        "",
    );
    assert!(matches!(
        SimpleGroupMetadata::from_event(&EventValue::Signed(oversized_value)),
        Err(SimpleGroupError::RecordValueTooLong {
            tag_index: 1,
            value_index: 1,
            bytes: 4_097,
            maximum: 4_096
        })
    ));

    let many_values = std::iter::once("supported_kinds".to_owned())
        .chain((0..256).map(|index| index.to_string()))
        .collect::<Vec<_>>();
    let oversized_row = source(
        &keys,
        39_000,
        vec![tag(&["d", "g"]), Tag::parse(many_values).expect("test tag")],
        "",
    );
    assert!(matches!(
        SimpleGroupMetadata::from_event(&EventValue::Signed(oversized_row)),
        Err(SimpleGroupError::TooManyRecordTagValues {
            tag_index: 1,
            actual: 257,
            maximum: 256
        })
    ));
}

#[test]
fn people_parser_signatures_accept_one_valid_row() {
    let keys = Keys::generate();
    let subject = Keys::generate().public_key();
    let key = subject.to_hex();
    let admins = source(
        &keys,
        39_001,
        vec![tag(&["d", "g"]), tag(&["p", &key, "admin"])],
        "",
    );
    let members = source(&keys, 39_002, vec![tag(&["d", "g"]), tag(&["p", &key])], "");
    let roles = source(
        &keys,
        39_003,
        vec![tag(&["d", "g"]), tag(&["role", "admin", "all"])],
        "",
    );
    let participants = source(
        &keys,
        39_004,
        vec![tag(&["d", "g"]), tag(&["participant", &key])],
        "",
    );

    assert_eq!(
        SimpleGroupAdmins::from_event(&EventValue::Signed(admins))
            .expect("admins")
            .admins(),
        [Ok((subject, vec!["admin".to_owned()]))]
    );
    assert_eq!(
        SimpleGroupMembers::from_event(&EventValue::Signed(members))
            .expect("members")
            .members(),
        [Ok(subject)]
    );
    assert_eq!(
        SimpleGroupRoles::from_event(&EventValue::Signed(roles))
            .expect("roles")
            .roles(),
        [Ok(("admin".to_owned(), Some("all".to_owned())))]
    );
    assert_eq!(
        SimpleGroupParticipants::from_event(&EventValue::Signed(participants))
            .expect("participants")
            .participants(),
        [Ok(subject)]
    );
}

#[test]
fn people_records_preserve_positive_order_and_attribution() {
    let keys = Keys::generate();
    let alice = Keys::generate().public_key();
    let bob = Keys::generate().public_key();
    let alice_hex = alice.to_hex();
    let bob_hex = bob.to_hex();

    let admins_event = source(
        &keys,
        39_001,
        vec![
            tag(&["d", "g"]),
            tag(&["p", &bob_hex, "secretary", "gardener"]),
            tag(&["x", "foreign"]),
            tag(&["p", &alice_hex, "ceo"]),
        ],
        "",
    );
    let admins = SimpleGroupAdmins::from_event(&EventValue::Signed(admins_event)).expect("admins");
    assert_eq!(admins.id(), "g");
    assert_eq!(admins.author(), keys.public_key());
    assert_eq!(
        admins.admins(),
        [
            Ok((bob, vec!["secretary".to_owned(), "gardener".to_owned()])),
            Ok((alice, vec!["ceo".to_owned()])),
        ]
    );

    let members_event = source(
        &keys,
        39_002,
        vec![
            tag(&["d", "g"]),
            tag(&["p", &alice_hex]),
            tag(&["p", &bob_hex]),
        ],
        "",
    );
    let members = SimpleGroupMembers::from_event(&EventValue::Signed(members_event)).expect("members");
    assert_eq!(members.id(), "g");
    assert_eq!(members.author(), keys.public_key());
    assert_eq!(members.members(), [Ok(alice), Ok(bob)]);

    let roles_event = source(
        &keys,
        39_003,
        vec![
            tag(&["d", "g"]),
            tag(&["role", "moderator", "delete messages"]),
            tag(&["role", "member"]),
            tag(&["role", "speaker", ""]),
        ],
        "",
    );
    let roles = SimpleGroupRoles::from_event(&EventValue::Signed(roles_event)).expect("roles");
    assert_eq!(roles.id(), "g");
    assert_eq!(roles.author(), keys.public_key());
    assert_eq!(
        roles.roles(),
        [
            Ok(("moderator".to_owned(), Some("delete messages".to_owned()))),
            Ok(("member".to_owned(), None)),
            Ok(("speaker".to_owned(), Some(String::new()))),
        ]
    );

    let participants_event = source(
        &keys,
        39_004,
        vec![
            tag(&["d", "g"]),
            tag(&["participant", &bob_hex]),
            tag(&["participant", &alice_hex]),
        ],
        "",
    );
    let participants =
        SimpleGroupParticipants::from_event(&EventValue::Signed(participants_event.clone()))
            .expect("participants");
    assert_eq!(participants.id(), "g");
    assert_eq!(participants.author(), keys.public_key());
    assert_eq!(participants.participants(), [Ok(bob), Ok(alice)]);
    assert_eq!(
        SimpleGroupParticipants::from_event(&EventValue::Signed(participants_event))
            .expect("repeated parsing is inert"),
        participants
    );

    let empty = source(&keys, 39_002, vec![tag(&["d", "g"])], "");
    assert!(
        SimpleGroupMembers::from_event(&EventValue::Signed(empty))
            .expect("empty positive evidence")
            .members()
            .is_empty()
    );
}

#[test]
fn invalid_people_rows_do_not_reserve_later_valid_rows() {
    let keys = Keys::generate();
    let member = Keys::generate().public_key();
    let member_hex = member.to_hex();
    let members_event = source(
        &keys,
        39_002,
        vec![
            tag(&["d", "g"]),
            tag(&["p", &member_hex, "extra"]),
            tag(&["p", &member_hex]),
            tag(&["p", &member_hex]),
        ],
        "",
    );
    let members = SimpleGroupMembers::from_event(&EventValue::Signed(members_event)).expect("members");
    assert!(matches!(
        &members.members()[0],
        Err(SimpleGroupError::MalformedRecordRow { tag_index: 1, .. })
    ));
    assert_eq!(members.members()[1], Ok(member));
    assert_eq!(
        members.members()[2],
        Err(SimpleGroupError::DuplicateRecordRow { tag_index: 3 })
    );

    let participant = Keys::generate().public_key();
    let lower = participant.to_hex();
    let upper = lower.to_uppercase();
    let participants_event = source(
        &keys,
        39_004,
        vec![
            tag(&["d", "g"]),
            tag(&["participant", &upper]),
            tag(&["participant", &lower]),
        ],
        "",
    );
    let participants = SimpleGroupParticipants::from_event(&EventValue::Signed(participants_event))
        .expect("participants");
    assert!(matches!(
        &participants.participants()[0],
        Err(SimpleGroupError::MalformedRecordRow { tag_index: 1, .. })
    ));
    assert_eq!(participants.participants()[1], Ok(participant));

    let roles_event = source(
        &keys,
        39_003,
        vec![
            tag(&["d", "g"]),
            tag(&["role", "admin", "one", "extra"]),
            tag(&["role", "admin", "two"]),
            tag(&["role", "admin", "three"]),
        ],
        "",
    );
    let roles = SimpleGroupRoles::from_event(&EventValue::Signed(roles_event)).expect("roles");
    assert!(matches!(
        &roles.roles()[0],
        Err(SimpleGroupError::MalformedRecordRow { tag_index: 1, .. })
    ));
    assert_eq!(
        roles.roles()[1],
        Ok(("admin".to_owned(), Some("two".to_owned())))
    );
    assert_eq!(
        roles.roles()[2],
        Err(SimpleGroupError::DuplicateRecordRow { tag_index: 3 })
    );
}

#[test]
fn pin_and_saved_parser_signatures_accept_one_valid_row() {
    let keys = Keys::generate();
    let pinned_id = source(&keys, 1, vec![], "").id.to_hex();
    let pins = source(
        &keys,
        39_005,
        vec![tag(&["d", "g"]), tag(&["e", &pinned_id])],
        "",
    );
    let groups = source(
        &keys,
        10_009,
        vec![tag(&["group", "g", "wss://a.example", "name"])],
        "",
    );
    let relays = source(&keys, 10_009, vec![tag(&["r", "wss://a.example"])], "");

    assert!(matches!(
        SimpleGroupPins::from_event(&EventValue::Signed(pins))
            .expect("pins")
            .items(),
        [Ok(PinnedItem::Event(_))]
    ));
    assert_eq!(
        SavedSimpleGroup::from_event(&EventValue::Signed(groups)).expect("saved groups")[0]
            .as_ref()
            .expect("valid group")
            .id(),
        "g"
    );
    assert_eq!(
        SavedRelay::from_event(&EventValue::Signed(relays)).expect("saved relays")[0]
            .as_ref()
            .expect("valid relay")
            .relay()
            .to_string(),
        "wss://a.example"
    );
}

#[test]
fn pins_preserve_interleaved_source_order() {
    let keys = Keys::generate();
    let first = source(&keys, 1, vec![], "first").id;
    let second = source(&keys, 1, vec![], "second").id;
    let address_author = Keys::generate().public_key();
    let address = format!("30023:{}:article:one", address_author.to_hex());
    let event = source(
        &keys,
        39_005,
        vec![
            tag(&["d", "g"]),
            tag(&["e", &first.to_hex()]),
            tag(&["a", &address]),
            tag(&["e", "invalid"]),
            tag(&["e", &second.to_hex()]),
        ],
        "",
    );
    let pins = SimpleGroupPins::from_event(&EventValue::Signed(event.clone())).expect("pins");

    assert_eq!(pins.id(), "g");
    assert_eq!(pins.author(), keys.public_key());
    assert_eq!(pins.items()[0], Ok(PinnedItem::Event(first)));
    assert_eq!(
        pins.items()[1],
        Ok(PinnedItem::Address(
            fava_state::EventCoordinate::Replaceable {
                author: address_author,
                kind: Kind::from_u16(30_023),
                identifier: Some("article:one".to_owned()),
            }
        ))
    );
    assert!(matches!(
        &pins.items()[2],
        Err(SimpleGroupError::MalformedRecordRow { tag_index: 3, .. })
    ));
    assert_eq!(pins.items()[3], Ok(PinnedItem::Event(second)));
    assert_eq!(
        SimpleGroupPins::from_event(&EventValue::Signed(event)).expect("repeated parse"),
        pins
    );
}

#[test]
fn saved_rows_preserve_host_and_author_evidence() {
    let keys = Keys::generate();
    let event = source(
        &keys,
        10_009,
        vec![
            tag(&["x", "foreign"]),
            tag(&["group", "photos", "wss://a.example"]),
            tag(&["group", "photos", "wss://b.example", "", "extra"]),
            tag(&["group", "photos", "wss://b.example", ""]),
            tag(&["group", "photos", "wss://b.example", "duplicate"]),
            tag(&["group", "", "wss://c.example"]),
            tag(&["group", "broken", "not-a-relay"]),
            tag(&["group"]),
            tag(&["r"]),
            tag(&["r", ""]),
            tag(&["r", "wss://relay.example", "extra"]),
            tag(&["r", "not-a-relay"]),
            tag(&["r", "wss://relay.example"]),
            tag(&["r", "wss://relay.example"]),
        ],
        "opaque encrypted content",
    );
    let value = EventValue::Signed(event.clone());
    let groups = SavedSimpleGroup::from_event(&value).expect("saved groups");
    let relays = SavedRelay::from_event(&value).expect("saved relays");

    let first = groups[0].as_ref().expect("first host");
    assert_eq!(first.id(), "photos");
    assert_eq!(first.relay().to_string(), "wss://a.example");
    assert_eq!(first.name(), None);
    assert_eq!(first.author(), keys.public_key());
    assert!(matches!(
        &groups[1],
        Err(SimpleGroupError::MalformedRecordRow { tag_index: 2, .. })
    ));
    let second = groups[2].as_ref().expect("second host after invalid row");
    assert_eq!(second.id(), "photos");
    assert_eq!(second.relay().to_string(), "wss://b.example");
    assert_eq!(second.name(), Some(""));
    assert_eq!(second.author(), keys.public_key());
    assert_eq!(
        groups[3],
        Err(SimpleGroupError::DuplicateRecordRow { tag_index: 4 })
    );
    assert!(groups[4..].iter().all(Result::is_err));

    assert!(matches!(
        &relays[0],
        Err(SimpleGroupError::MalformedRecordRow { tag_index: 8, .. })
    ));
    assert!(matches!(
        &relays[1],
        Err(SimpleGroupError::MalformedRecordRow { tag_index: 9, .. })
    ));
    assert!(matches!(
        &relays[2],
        Err(SimpleGroupError::MalformedRecordRow { tag_index: 10, .. })
    ));
    assert!(matches!(
        &relays[3],
        Err(SimpleGroupError::MalformedRecordRow { tag_index: 11, .. })
    ));
    let relay = relays[4].as_ref().expect("valid relay after invalid rows");
    assert_eq!(relay.relay().to_string(), "wss://relay.example");
    assert_eq!(relay.author(), keys.public_key());
    assert_eq!(
        relays[5],
        Err(SimpleGroupError::DuplicateRecordRow { tag_index: 13 })
    );
    assert_eq!(
        SavedSimpleGroup::from_event(&EventValue::Signed(event)).expect("repeated parse"),
        groups
    );
}
