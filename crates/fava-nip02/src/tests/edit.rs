use fava_state::RelayUrl;
use fava_write::{Kind, ReplaceableEventEdit, WriteIntentError};

use super::{materialize, tag};
use crate::{follow, follow_with, materializer, unfollow};

#[test]
fn edit_codec_accepts_keys_and_supported_key_strings() {
    let actor = nostr::key::Keys::generate();
    let target = nostr::key::Keys::generate().public_key();
    let hex = target.to_hex();

    assert_eq!(
        follow(target).expect("key"),
        follow(hex.as_str()).expect("hex")
    );
    assert_eq!(
        unfollow(target).expect("key"),
        unfollow(hex).expect("owned hex")
    );

    let refused = follow("raw-secret-invalid-key").expect_err("invalid key refuses");
    assert!(matches!(refused, WriteIntentError::InvalidEvent(_)));
    assert!(!refused.to_string().contains("raw-secret-invalid-key"));

    let followed = materialize(actor.public_key(), &follow(target).expect("edit"), None, 1)
        .expect("key edit materializes");
    assert_eq!(followed.tags.as_slice(), &[tag(&["p", &target.to_hex()])]);
}

#[test]
fn edit_codec_encodes_optional_metadata_without_collapsing_petname_presence() {
    let actor = nostr::key::Keys::generate();
    let target = nostr::key::Keys::generate().public_key();
    let relay = RelayUrl::parse("wss://relay.example").expect("relay");

    assert_eq!(
        follow_with(target, None, None).expect("plain metadata edit"),
        follow(target).expect("plain follow")
    );

    let relay_only = follow_with(target, Some(relay.clone()), None).expect("relay edit");
    let relay_event = materialize(actor.public_key(), &relay_only, None, 1).expect("relay row");
    assert_eq!(
        relay_event.tags.as_slice(),
        &[tag(&["p", &target.to_hex(), relay.as_str()])]
    );

    let empty_petname = follow_with(target, None, Some("")).expect("present empty petname");
    let empty_event = materialize(actor.public_key(), &empty_petname, None, 1).expect("empty name");
    assert_eq!(
        empty_event.tags.as_slice(),
        &[tag(&["p", &target.to_hex(), "", ""])]
    );

    let exact_petname = "a\u{301}";
    let complete = follow_with(target, Some(relay.clone()), Some(exact_petname))
        .expect("complete metadata edit");
    let complete_event = materialize(actor.public_key(), &complete, None, 1).expect("complete row");
    assert_eq!(
        complete_event.tags.as_slice(),
        &[tag(&["p", &target.to_hex(), relay.as_str(), exact_petname])]
    );
}

#[test]
fn edit_codec_refuses_malformed_or_over_bound_metadata_without_raw_input() {
    let actor = nostr::key::Keys::generate();
    let target = nostr::key::Keys::generate().public_key();
    let refused = follow_with(target, None, Some(&"x".repeat(140_000)))
        .expect_err("over-bound petname refuses");
    assert!(matches!(refused, WriteIntentError::TooLarge { .. }));

    let raw_relay = "raw-secret-not-a-relay";
    let mut malformed = vec![3];
    malformed.extend_from_slice(target.as_bytes());
    malformed.extend_from_slice(&(raw_relay.len() as u32).to_be_bytes());
    malformed.extend_from_slice(raw_relay.as_bytes());
    malformed.push(0);
    let malformed = ReplaceableEventEdit::new(Kind::ContactList, None, malformed)
        .expect("neutral edit accepts opaque bounded bytes");
    assert!(!materializer().supports(&malformed));
    let error = materialize(actor.public_key(), &malformed, None, 1)
        .expect_err("invalid encoded relay refuses");
    assert!(matches!(error, WriteIntentError::Encoding(_)));
    assert!(!error.to_string().contains(raw_relay));
}
