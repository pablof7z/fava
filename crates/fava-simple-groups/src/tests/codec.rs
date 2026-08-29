use fava_write::{Kind, PublicKey, EventEdit, Timestamp, WriteIntentError};

use crate::saved_group_list_applier;

const SAVE_SIMPLE_GROUP: u8 = 1;
const SAVE_RELAY: u8 = 4;

fn text(value: &[u8]) -> Vec<u8> {
    let mut encoded = u32::try_from(value.len())
        .expect("fixture length")
        .to_be_bytes()
        .to_vec();
    encoded.extend_from_slice(value);
    encoded
}

fn edit(kind: Kind, identifier: Option<&str>, change: Vec<u8>) -> EventEdit {
    EventEdit::new(kind, identifier.map(str::to_owned), change)
        .expect("neutral edit shape")
}

fn assert_refusal(edit: &EventEdit, expected: &str) {
    let applier = saved_group_list_applier();
    assert!(!applier.supports(edit));
    assert_eq!(
        applier.apply(
            edit,
            PublicKey::from_slice(&[2; 32]).unwrap(),
            None,
            Timestamp::from(1),
        ),
        Err(WriteIntentError::Encoding(expected.to_owned()))
    );
}

#[test]
fn private_codec_rejects_every_malformed_frame_without_partial_acceptance() {
    let relay = text(b"wss://relay.example");

    let mut trailing = vec![SAVE_RELAY];
    trailing.extend_from_slice(&relay);
    trailing.push(0);

    let mut invalid_utf8 = vec![SAVE_RELAY];
    invalid_utf8.extend_from_slice(&text(&[0xff]));

    let mut zero_relays = vec![SAVE_SIMPLE_GROUP];
    zero_relays.extend_from_slice(&text(b"g"));
    zero_relays.extend_from_slice(&0u16.to_be_bytes());
    zero_relays.push(0);

    let mut duplicate_relays = vec![SAVE_SIMPLE_GROUP];
    duplicate_relays.extend_from_slice(&text(b"g"));
    duplicate_relays.extend_from_slice(&2u16.to_be_bytes());
    duplicate_relays.extend_from_slice(&relay);
    duplicate_relays.extend_from_slice(&relay);
    duplicate_relays.push(0);

    let mut invalid_flag = vec![SAVE_SIMPLE_GROUP];
    invalid_flag.extend_from_slice(&text(b"g"));
    invalid_flag.extend_from_slice(&1u16.to_be_bytes());
    invalid_flag.extend_from_slice(&relay);
    invalid_flag.push(2);

    let mut invalid_relay = vec![SAVE_RELAY];
    invalid_relay.extend_from_slice(&text(b"not-a-relay"));

    for (change, expected) in [
        (Vec::new(), "truncated saved-list edit"),
        (vec![255], "unknown saved-list edit operation"),
        (vec![SAVE_RELAY, 0, 0], "truncated saved-list text length"),
        (invalid_utf8, "saved-list text is not UTF-8"),
        (zero_relays, "invalid saved-list relay count"),
        (duplicate_relays, "duplicate saved-list simple-group relay"),
        (invalid_flag, "invalid saved-list optional text flag"),
        (invalid_relay, "invalid saved-list relay"),
        (trailing, "trailing saved-list edit bytes"),
    ] {
        assert_refusal(&edit(Kind::from_u16(10_009), None, change), expected);
    }
}

#[test]
fn private_codec_rejects_wrong_replaceable_coordinates_exactly() {
    let mut change = vec![SAVE_RELAY];
    change.extend_from_slice(&text(b"wss://relay.example"));
    let wrong_kind = edit(Kind::from_u16(3), None, change.clone());
    let wrong_identifier = edit(Kind::from_u16(30_000), Some("g"), change);
    for edit in [&wrong_kind, &wrong_identifier] {
        assert_refusal(
            edit,
            "saved-list edit requires a non-addressable kind-10009 coordinate",
        );
    }
}
