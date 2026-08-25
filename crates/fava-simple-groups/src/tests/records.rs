use fava_state::EventCoordinate;
use fava_write::{EventId, Kind, PublicKey};

use crate::{
    SimpleGroupAdmins, SimpleGroupDecodeError, SimpleGroupLivekitParticipants, SimpleGroupMembers,
    SimpleGroupMetadata, SimpleGroupPins, SimpleGroupRoles,
};

use super::{public_key, tag, value};

fn second_key() -> PublicKey {
    PublicKey::from_hex("c6047f9441ed7d6d3045406e95c07cd85a9f4e76aef81f0253d7c3b1a51b2055")
        .expect("second public key")
}

#[test]
fn event_boundary_uses_first_d_value_and_ignores_later_material() {
    let metadata = SimpleGroupMetadata::from_event(&value(
        39_000,
        vec![
            tag(&["d", "", "unused"]),
            tag(&["d", "later"]),
            tag(&["name"]),
            tag(&["name", "first", "unused"]),
            tag(&["name", "later"]),
        ],
    ))
    .expect("tolerant metadata");
    assert_eq!(metadata.id(), "");
    assert_eq!(metadata.name(), Some("first"));

    assert_eq!(
        SimpleGroupMetadata::from_event(&value(39_001, vec![tag(&["d", "g"])])),
        Err(SimpleGroupDecodeError::WrongEventKind {
            expected: Kind::from_u16(39_000),
            actual: Kind::from_u16(39_001),
        })
    );
    assert_eq!(
        SimpleGroupMetadata::from_event(&value(39_000, vec![])),
        Err(SimpleGroupDecodeError::MissingIdentifierTag)
    );
    assert_eq!(
        SimpleGroupMetadata::from_event(&value(39_000, vec![tag(&["d"])])),
        Err(SimpleGroupDecodeError::MissingTagValue {
            tag_index: 0,
            value_index: 1,
        })
    );
}

#[test]
fn metadata_conserves_value_local_failures_and_repetitions() {
    let metadata = SimpleGroupMetadata::from_event(&value(
        39_000,
        vec![
            tag(&["d", "g"]),
            tag(&["private", "ignored"]),
            tag(&["supported_kinds", "1", "bad", "1"]),
            tag(&["supported_kinds", "2"]),
            tag(&["child", "one", "ignored"]),
            tag(&["child"]),
            tag(&["child", "one"]),
            tag(&["unknown", "junk"]),
        ],
    ))
    .expect("metadata");
    assert!(metadata.is_private());
    let kinds = metadata.supported_kinds().expect("present");
    assert_eq!(kinds[0], Ok(Kind::from_u16(1)));
    assert!(matches!(
        kinds[1],
        Err(SimpleGroupDecodeError::InvalidKind {
            tag_index: 2,
            value_index: 2
        })
    ));
    assert_eq!(kinds[2], Ok(Kind::from_u16(1)));
    assert_eq!(metadata.children()[0], Ok("one".to_owned()));
    assert!(matches!(
        metadata.children()[1],
        Err(SimpleGroupDecodeError::MissingTagValue {
            tag_index: 5,
            value_index: 1
        })
    ));
    assert_eq!(metadata.children()[2], Ok("one".to_owned()));
}

#[test]
fn repeated_people_and_role_entries_survive_with_local_failures() {
    let alice = public_key();
    let bob = second_key();
    let admins = SimpleGroupAdmins::from_event(&value(
        39_001,
        vec![
            tag(&["d", "g"]),
            tag(&["p", &alice.to_hex(), "owner", "moderator"]),
            tag(&["p", "bad", "owner"]),
            tag(&["p", &alice.to_hex(), "owner"]),
            tag(&["p", &bob.to_hex()]),
        ],
    ))
    .expect("admins");
    assert_eq!(
        admins.admins()[0],
        Ok((alice, vec!["owner".into(), "moderator".into()]))
    );
    assert!(matches!(
        admins.admins()[1],
        Err(SimpleGroupDecodeError::InvalidPublicKey { tag_index: 2, .. })
    ));
    assert_eq!(admins.admins()[2], Ok((alice, vec!["owner".into()])));
    assert!(matches!(
        admins.admins()[3],
        Err(SimpleGroupDecodeError::MissingTagValue {
            tag_index: 4,
            value_index: 2
        })
    ));

    let members = SimpleGroupMembers::from_event(&value(
        39_002,
        vec![
            tag(&["d", "g"]),
            tag(&["p", &bob.to_hex(), "ignored"]),
            tag(&["p", &bob.to_hex()]),
        ],
    ))
    .unwrap();
    assert_eq!(members.members(), [Ok(bob), Ok(bob)]);

    let roles = SimpleGroupRoles::from_event(&value(
        39_003,
        vec![
            tag(&["d", "g"]),
            tag(&["role", "", "", "ignored"]),
            tag(&["role"]),
        ],
    ))
    .unwrap();
    assert_eq!(roles.roles()[0], Ok((String::new(), Some(String::new()))));
    assert!(matches!(
        roles.roles()[1],
        Err(SimpleGroupDecodeError::MissingTagValue { .. })
    ));
}

#[test]
fn livekit_keys_require_exact_lowercase_hex() {
    let lower = public_key().to_hex();
    let upper = lower.to_uppercase();
    let participants = SimpleGroupLivekitParticipants::from_event(&value(
        39_004,
        vec![
            tag(&["d", "g"]),
            tag(&["participant", &lower, "ignored"]),
            tag(&["participant", &upper]),
            tag(&["participant", &lower]),
        ],
    ))
    .unwrap();
    assert_eq!(participants.participants()[0], Ok(public_key()));
    assert!(matches!(
        participants.participants()[1],
        Err(SimpleGroupDecodeError::InvalidLivekitParticipantPublicKey { tag_index: 2, .. })
    ));
    assert_eq!(participants.participants()[2], Ok(public_key()));
}

#[test]
fn pins_use_event_coordinate_and_preserve_interleaving() {
    let id = EventId::from_hex("00b6e5d3d2ec995b85fdfe7c46c6639f4f8cc62f79c0e5b57c92f3b75b0e3a68")
        .expect("event id");
    let address = format!("30023:{}:article", public_key().to_hex());
    let pins = SimpleGroupPins::from_event(&value(
        39_005,
        vec![
            tag(&["d", "g"]),
            tag(&["e", &id.to_hex(), "ignored"]),
            tag(&["a", &address, "ignored"]),
            tag(&["e", "bad"]),
            tag(&["e", &id.to_hex()]),
        ],
    ))
    .unwrap();
    assert_eq!(pins.pins()[0], Ok(EventCoordinate::Event(id)));
    assert_eq!(
        pins.pins()[1],
        Ok(EventCoordinate::Replaceable {
            author: public_key(),
            kind: Kind::from_u16(30_023),
            identifier: Some("article".to_owned()),
        })
    );
    assert!(matches!(
        pins.pins()[2],
        Err(SimpleGroupDecodeError::InvalidEventId { .. })
    ));
    assert_eq!(pins.pins()[3], Ok(EventCoordinate::Event(id)));
}

#[test]
fn pin_coordinates_use_the_neutral_nostr_coordinate_parser() {
    let key = public_key().to_hex();
    let pins = SimpleGroupPins::from_event(&value(
        39_005,
        vec![
            tag(&["d", "g"]),
            tag(&["a", &format!("30023:{key}:article")]),
            tag(&["a", &format!("1:{key}:")]),
            tag(&["a", "not-a-coordinate"]),
        ],
    ))
    .expect("pin event");

    assert!(matches!(
        &pins.pins()[0],
        Ok(EventCoordinate::Replaceable { kind, identifier, .. })
            if *kind == Kind::from_u16(30_023) && identifier.as_deref() == Some("article")
    ));
    assert!(matches!(
        pins.pins()[1],
        Err(SimpleGroupDecodeError::InvalidEventCoordinate {
            tag_index: 2,
            value_index: 1
        })
    ));
    assert!(matches!(
        pins.pins()[2],
        Err(SimpleGroupDecodeError::InvalidEventCoordinate {
            tag_index: 3,
            value_index: 1
        })
    ));
}
