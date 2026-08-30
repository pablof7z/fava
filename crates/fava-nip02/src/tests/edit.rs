use std::cell::Cell;
use std::fmt;

use fava_write::{Kind, EventEdit, WriteIntentError};
use nostr::nips::nip19::ToBech32;
use nostr::nips::nip21::ToNostrUri;
use nostr::types::RelayUrl;

use super::{apply, source, tag, target_tags};
use crate::edit::applier;
use crate::{follow, follow_with, unfollow};

#[test]
fn edit_codec_accepts_keys_and_supported_key_strings() {
    let actor = nostr::key::Keys::generate();
    let target = nostr::key::Keys::generate().public_key();
    let hex = target.to_hex();
    let npub = target.to_bech32().expect("npub");
    let nip21 = target.to_nostr_uri().expect("NIP-21 URI");

    assert_eq!(
        follow(target).expect("key"),
        follow(hex.as_str()).expect("hex")
    );
    assert_eq!(
        unfollow(target).expect("key"),
        unfollow(hex).expect("owned hex")
    );
    assert_eq!(follow(target).expect("key"), follow(npub).expect("npub"));
    assert_eq!(
        follow(target).expect("key"),
        follow(nip21).expect("NIP-21 URI")
    );

    let refused = follow("raw-secret-invalid-key").expect_err("invalid key refuses");
    assert!(matches!(refused, WriteIntentError::InvalidEvent(_)));
    assert!(!refused.to_string().contains("raw-secret-invalid-key"));

    let followed = apply(actor.public_key(), &follow(target).expect("edit"), None, 1)
        .expect("key edit applies");
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
    let relay_event = apply(actor.public_key(), &relay_only, None, 1).expect("relay row");
    assert_eq!(
        relay_event.tags.as_slice(),
        &[tag(&["p", &target.to_hex(), relay.as_str()])]
    );

    let empty_petname = follow_with(target, None, Some("")).expect("present empty petname");
    let empty_event = apply(actor.public_key(), &empty_petname, None, 1).expect("empty name");
    assert_eq!(
        empty_event.tags.as_slice(),
        &[tag(&["p", &target.to_hex(), "", ""])]
    );

    let exact_petname = "a\u{301}";
    let complete = follow_with(target, Some(relay.clone()), Some(exact_petname))
        .expect("complete metadata edit");
    let complete_event = apply(actor.public_key(), &complete, None, 1).expect("complete row");
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
    assert!(matches!(
        refused,
        WriteIntentError::TooLarge {
            bytes: 140_042,
            maximum: 131_072
        }
    ));

    let raw_relay = "raw-secret-not-a-relay";
    let mut malformed = vec![3];
    malformed.extend_from_slice(target.as_bytes());
    malformed.extend_from_slice(
        &u32::try_from(raw_relay.len())
            .expect("fixture relay length fits u32")
            .to_be_bytes(),
    );
    malformed.extend_from_slice(raw_relay.as_bytes());
    malformed.push(0);
    let malformed = EventEdit::new(Kind::ContactList, None, malformed)
        .expect("neutral edit accepts opaque bounded bytes");
    assert!(!applier().supports(&malformed));
    let error = apply(actor.public_key(), &malformed, None, 1)
        .expect_err("invalid encoded relay refuses");
    assert!(matches!(error, WriteIntentError::Encoding(_)));
    assert!(!error.to_string().contains(raw_relay));
}

#[test]
fn edit_codec_stops_hostile_target_formatting_at_the_public_key_bound() {
    struct HostileTarget<'a>(&'a Cell<usize>);

    impl fmt::Display for HostileTarget<'_> {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            for _ in 0..1_000_000 {
                self.0.set(self.0.get() + 1);
                formatter.write_str("a")?;
            }
            Ok(())
        }
    }

    let writes = Cell::new(0);
    let refused = follow(HostileTarget(&writes)).expect_err("over-bound target refuses");
    assert_eq!(
        writes.get(),
        70,
        "formatting must stop at the first excess byte"
    );
    assert_eq!(
        refused,
        WriteIntentError::TooLarge {
            bytes: 70,
            maximum: 69,
        }
    );
}

#[test]
fn nip02_preserves_foreign_kind3_bytes() {
    let actor = nostr::key::Keys::generate();
    let target = nostr::key::Keys::generate().public_key();
    let other = nostr::key::Keys::generate().public_key();
    let target_hex = target.to_hex();
    let original = vec![
        tag(&["something-something"]),
        tag(&[
            "p",
            &target_hex,
            "wss://original.example",
            "original-name",
            "foreign-extra-column",
        ]),
        tag(&["t", "nostr"]),
        tag(&["p", "not-a-public-key", "raw-malformed-row"]),
        tag(&["x", "between", "bytes"]),
        tag(&["p", &target_hex, "wss://duplicate.example", "duplicate"]),
        tag(&["p", &other.to_hex(), "wss://other.example"]),
        tag(&["p", &target_hex, "not-a-relay", "still-a-duplicate"]),
        tag(&["tail"]),
    ];
    let legacy_content = "{\"wss://legacy.example\":{\"read\":true}}\nopaque-π";
    let base = source(
        &actor,
        Kind::ContactList,
        10,
        legacy_content,
        original.clone(),
    );

    let followed = apply(
        actor.public_key(),
        &follow_with(
            target,
            Some(RelayUrl::parse("wss://ignored.example").expect("relay")),
            Some("ignored-name"),
        )
        .expect("follow edit"),
        Some(&base),
        11,
    )
    .expect("follow applies losslessly");
    assert_eq!(followed.content.as_bytes(), legacy_content.as_bytes());
    assert_eq!(
        followed.tags.as_slice(),
        &[
            original[0].clone(),
            original[1].clone(),
            original[2].clone(),
            original[3].clone(),
            original[4].clone(),
            original[6].clone(),
            original[8].clone(),
        ]
    );

    let unfollowed = apply(
        actor.public_key(),
        &unfollow(target).expect("unfollow edit"),
        Some(&base),
        11,
    )
    .expect("unfollow applies losslessly");
    assert_eq!(unfollowed.content.as_bytes(), legacy_content.as_bytes());
    assert_eq!(
        unfollowed.tags.as_slice(),
        &[
            original[0].clone(),
            original[2].clone(),
            original[3].clone(),
            original[4].clone(),
            original[6].clone(),
            original[8].clone(),
        ]
    );
}

#[test]
fn follow_edits_are_stable_over_empty_and_newer_qualified_sources() {
    let actor = nostr::key::Keys::generate();
    let target = nostr::key::Keys::generate().public_key();
    let concurrent = nostr::key::Keys::generate().public_key();
    let edit = follow_with(
        target,
        Some(RelayUrl::parse("wss://requested.example").expect("relay")),
        Some("requested"),
    )
    .expect("metadata edit");

    let empty = apply(actor.public_key(), &edit, None, 1).expect("empty source");
    assert_eq!(
        empty.tags.as_slice(),
        &[tag(&[
            "p",
            &target.to_hex(),
            "wss://requested.example",
            "requested",
        ])]
    );

    let newer_tags = vec![
        tag(&["something-something"]),
        tag(&[
            "p",
            &target.to_hex(),
            "wss://newer.example",
            "newer-name",
            "newer-extra",
        ]),
        tag(&["t", "nostr"]),
        tag(&["p", &concurrent.to_hex(), "wss://concurrent.example"]),
    ];
    let newer = source(
        &actor,
        Kind::ContactList,
        20,
        "newer-legacy-content",
        newer_tags.clone(),
    );
    let rebased = apply(actor.public_key(), &edit, Some(&newer), 21).expect("rebase");
    assert_eq!(rebased.content, "newer-legacy-content");
    assert_eq!(rebased.tags.as_slice(), newer_tags.as_slice());

    let signed_rebased = source(
        &actor,
        Kind::ContactList,
        21,
        &rebased.content,
        rebased.tags.clone().to_vec(),
    );
    let repeated =
        apply(actor.public_key(), &edit, Some(&signed_rebased), 22).expect("repeated add");
    assert_eq!(repeated.tags, rebased.tags);
    assert_eq!(target_tags(&repeated, target), 1);

    let remove = unfollow(target).expect("remove");
    let removed =
        apply(actor.public_key(), &remove, Some(&signed_rebased), 22).expect("remove once");
    let signed_removed = source(
        &actor,
        Kind::ContactList,
        22,
        &removed.content,
        removed.tags.clone().to_vec(),
    );
    let removed_again =
        apply(actor.public_key(), &remove, Some(&signed_removed), 23).expect("remove twice");
    assert_eq!(removed_again.tags, removed.tags);
}

#[test]
fn nip02_errors_and_sources_do_not_log_raw_inputs() {
    let actor = nostr::key::Keys::generate();
    let target = nostr::key::Keys::generate().public_key();
    let raw_content = "raw-secret-contact-content";
    let mut tampered = source(
        &actor,
        Kind::ContactList,
        1,
        raw_content,
        vec![tag(&["raw-secret-tag"])],
    );
    tampered.content.push('!');
    let error = apply(
        actor.public_key(),
        &follow(target).expect("edit"),
        Some(&tampered),
        2,
    )
    .expect_err("tampered source refuses");
    assert!(!error.to_string().contains(raw_content));
    assert!(!error.to_string().contains("raw-secret-tag"));

    let manifest = include_str!("../../Cargo.toml");
    for dependency in ["log =", "log.", "tracing =", "tracing."] {
        assert!(
            !manifest.lines().any(|line| {
                let line = line.trim_start();
                !line.starts_with('#') && line.starts_with(dependency)
            }),
            "fava-nip02 must not declare logging dependency {dependency}"
        );
    }
    for source_text in [
        include_str!("../lib.rs"),
        include_str!("../edit.rs"),
        include_str!("../bounds.rs"),
        include_str!("../contact_list.rs"),
        include_str!("../query.rs"),
    ] {
        for logging_macro in ["tracing::", "log::", "println!(", "eprintln!(", "dbg!("] {
            assert!(
                !source_text.contains(logging_macro),
                "fava-nip02 source must not use {logging_macro}"
            );
        }
    }
}
