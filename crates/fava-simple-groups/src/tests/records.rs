use fava_write::{Event, EventBuilder as FavaEventBuilder, EventValue, Kind, Tag, Timestamp};
use nostr::event::{EventBuilder, FinalizeEvent};
use nostr::key::Keys;

use crate::{GroupError, GroupMetadata};

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
