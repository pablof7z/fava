use fava_write::{Event, EventBuilder as FavaEventBuilder, EventValue, Kind, Tag, Timestamp};
use nostr::event::{EventBuilder, FinalizeEvent};
use nostr::key::Keys;

use crate::{GroupAdmins, GroupError, GroupMembers, GroupMetadata, GroupParticipants, GroupRoles};

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
    let metadata = GroupMetadata::from_event(&EventValue::Signed(event)).expect("metadata");

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
        GroupMetadata::from_event(&EventValue::Signed(absent))
            .expect("absent supported kinds")
            .supported_kinds(),
        None
    );
    assert_eq!(
        GroupMetadata::from_event(&EventValue::Signed(empty))
            .expect("present-empty supported kinds")
            .supported_kinds(),
        Some([].as_slice())
    );
}

#[test]
fn record_boundary_refuses_ambiguous_or_oversized_input() {
    let keys = Keys::generate();
    let minimal = source(&keys, 39_000, vec![tag(&["d", "g"])], "");
    assert_eq!(
        GroupMetadata::from_event(&EventValue::Signed(minimal))
            .expect("narrow valid record")
            .id(),
        "g"
    );

    let wrong_kind = source(&keys, 39_001, vec![tag(&["d", "g"])], "");
    assert!(matches!(
        GroupMetadata::from_event(&EventValue::Signed(wrong_kind)),
        Err(GroupError::WrongRecordKind {
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
        GroupMetadata::from_event(&EventValue::Unsigned(unsigned)),
        Err(GroupError::UnsignedRecord)
    );

    let mut invalid_id = source(&keys, 39_000, vec![tag(&["d", "g"])], "before");
    invalid_id.content = "after".to_owned();
    assert_eq!(
        GroupMetadata::from_event(&EventValue::Signed(invalid_id)),
        Err(GroupError::InvalidRecordId)
    );

    let valid = source(&keys, 39_000, vec![tag(&["d", "g"])], "");
    let mut invalid_signature = valid.clone();
    invalid_signature.sig = "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
        .parse()
        .expect("shape-valid signature");
    assert_eq!(
        GroupMetadata::from_event(&EventValue::Signed(invalid_signature)),
        Err(GroupError::InvalidRecordSignature)
    );

    let cases = [
        (
            source(&keys, 39_000, vec![], ""),
            GroupError::MissingRecordId,
        ),
        (
            source(&keys, 39_000, vec![tag(&["d"])], ""),
            GroupError::EmptyRecordId,
        ),
        (
            source(&keys, 39_000, vec![tag(&["d", ""])], ""),
            GroupError::EmptyRecordId,
        ),
        (
            source(&keys, 39_000, vec![tag(&["d", "g"]), tag(&["d", "g"])], ""),
            GroupError::DuplicateRecordId,
        ),
        (
            source(
                &keys,
                39_000,
                vec![tag(&["d", "g"]), tag(&["d", "other"])],
                "",
            ),
            GroupError::ConflictingRecordId,
        ),
    ];
    for (event, expected) in cases {
        assert_eq!(
            GroupMetadata::from_event(&EventValue::Signed(event)),
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
        GroupMetadata::from_event(&EventValue::Signed(duplicate_name)),
        Err(GroupError::AmbiguousRecordField("name"))
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
        GroupMetadata::from_event(&EventValue::Signed(too_many_tags)),
        Err(GroupError::TooManyRecordTags {
            actual: 2_001,
            maximum: 2_000
        })
    ));

    let oversized_event = source(&keys, 39_000, vec![tag(&["d", "g"])], &"x".repeat(131_073));
    assert!(matches!(
        GroupMetadata::from_event(&EventValue::Signed(oversized_event)),
        Err(GroupError::RecordTooLarge {
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
        GroupMetadata::from_event(&EventValue::Signed(oversized_value)),
        Err(GroupError::RecordValueTooLong {
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
        GroupMetadata::from_event(&EventValue::Signed(oversized_row)),
        Err(GroupError::TooManyRecordTagValues {
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
        GroupAdmins::from_event(&EventValue::Signed(admins))
            .expect("admins")
            .admins(),
        [Ok((subject, vec!["admin".to_owned()]))]
    );
    assert_eq!(
        GroupMembers::from_event(&EventValue::Signed(members))
            .expect("members")
            .members(),
        [Ok(subject)]
    );
    assert_eq!(
        GroupRoles::from_event(&EventValue::Signed(roles))
            .expect("roles")
            .roles(),
        [Ok(("admin".to_owned(), Some("all".to_owned())))]
    );
    assert_eq!(
        GroupParticipants::from_event(&EventValue::Signed(participants))
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
    let admins = GroupAdmins::from_event(&EventValue::Signed(admins_event)).expect("admins");
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
    let members = GroupMembers::from_event(&EventValue::Signed(members_event)).expect("members");
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
    let roles = GroupRoles::from_event(&EventValue::Signed(roles_event)).expect("roles");
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
        GroupParticipants::from_event(&EventValue::Signed(participants_event.clone()))
            .expect("participants");
    assert_eq!(participants.id(), "g");
    assert_eq!(participants.author(), keys.public_key());
    assert_eq!(participants.participants(), [Ok(bob), Ok(alice)]);
    assert_eq!(
        GroupParticipants::from_event(&EventValue::Signed(participants_event))
            .expect("repeated parsing is inert"),
        participants
    );

    let empty = source(&keys, 39_002, vec![tag(&["d", "g"])], "");
    assert!(
        GroupMembers::from_event(&EventValue::Signed(empty))
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
    let members = GroupMembers::from_event(&EventValue::Signed(members_event)).expect("members");
    assert!(matches!(
        &members.members()[0],
        Err(GroupError::MalformedRecordRow { tag_index: 1, .. })
    ));
    assert_eq!(members.members()[1], Ok(member));
    assert_eq!(
        members.members()[2],
        Err(GroupError::DuplicateRecordRow { tag_index: 3 })
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
    let participants = GroupParticipants::from_event(&EventValue::Signed(participants_event))
        .expect("participants");
    assert!(matches!(
        &participants.participants()[0],
        Err(GroupError::MalformedRecordRow { tag_index: 1, .. })
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
    let roles = GroupRoles::from_event(&EventValue::Signed(roles_event)).expect("roles");
    assert!(matches!(
        &roles.roles()[0],
        Err(GroupError::MalformedRecordRow { tag_index: 1, .. })
    ));
    assert_eq!(
        roles.roles()[1],
        Ok(("admin".to_owned(), Some("two".to_owned())))
    );
    assert_eq!(
        roles.roles()[2],
        Err(GroupError::DuplicateRecordRow { tag_index: 3 })
    );
}
