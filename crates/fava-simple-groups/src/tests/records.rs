use fava_write::{Kind, PublicKey};

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
    assert_eq!(kinds[0], "1");
    assert_eq!(kinds[1], "bad");
    assert_eq!(kinds[2], "1");
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
    let foreign_key = "not-a-public-key-ß";
    let admins = SimpleGroupAdmins::from_event(&value(
        39_001,
        vec![
            tag(&["d", "g"]),
            tag(&["p", &alice.to_hex(), "owner", "moderator"]),
            tag(&["p", foreign_key, "owner"]),
            tag(&["p", &alice.to_hex(), "owner"]),
            tag(&["p", &bob.to_hex()]),
        ],
    ))
    .expect("admins");
    assert_eq!(
        admins.admins()[0],
        Ok((alice.to_hex(), vec!["owner".into(), "moderator".into()]))
    );
    assert_eq!(
        admins.admins()[1],
        Ok((foreign_key.to_owned(), vec!["owner".into()]))
    );
    assert_eq!(
        admins.admins()[2],
        Ok((alice.to_hex(), vec!["owner".into()]))
    );
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
            tag(&["p", foreign_key, "ignored-extra"]),
            tag(&["p", &bob.to_hex()]),
            tag(&["p"]),
        ],
    ))
    .unwrap();
    assert_eq!(
        members.members(),
        [
            Ok(bob.to_hex()),
            Ok(foreign_key.to_owned()),
            Ok(bob.to_hex()),
            Err(SimpleGroupDecodeError::MissingTagValue {
                tag_index: 4,
                value_index: 1,
            }),
        ]
    );

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
fn pins_preserve_exact_raw_tags_and_interleaving() {
    let id = "00b6e5d3d2ec995b85fdfe7c46c6639f4f8cc62f79c0e5b57c92f3b75b0e3a68";
    let address = format!("30023:{}:article", public_key().to_hex());
    let expected = [
        tag(&["e", id, "", "ignored-ß"]),
        tag(&["a", &address, "wss://hint.example", "marker"]),
        tag(&["e", "not-an-event-id", "exact-extra"]),
        tag(&["e", id]),
        tag(&["a", "not-a-coordinate", ""]),
    ];
    let pins = SimpleGroupPins::from_event(&value(
        39_005,
        vec![
            tag(&["d", "g"]),
            expected[0].clone(),
            expected[1].clone(),
            expected[2].clone(),
            expected[3].clone(),
            expected[4].clone(),
            tag(&["e"]),
        ],
    ))
    .unwrap();
    for (actual, raw) in pins.pins()[..expected.len()].iter().zip(&expected) {
        assert_eq!(actual, &Ok(raw.clone()));
    }
    assert_eq!(
        pins.pins()[5],
        Err(SimpleGroupDecodeError::MissingTagValue {
            tag_index: 6,
            value_index: 1
        })
    );
}
