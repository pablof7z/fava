//! Hostile sibling tags remain scoped to their own decode failures.

use fava_state::{deletion_applies, event_is_expired};
use nostr::event::{EventBuilder, FinalizeEvent, Kind, Tag};
use nostr::key::Keys;
use nostr::types::Timestamp;

#[test]
fn valid_event_deletion_and_expiration_survive_malformed_siblings()
-> Result<(), Box<dyn std::error::Error>> {
    let keys = Keys::generate();
    let target = EventBuilder::new(Kind::TextNote, "target")
        .tags([
            Tag::parse(["expiration"])?,
            Tag::parse(["expiration", "not-a-timestamp"])?,
            Tag::parse(["x-hostile", "unrelated"])?,
            Tag::parse(["expiration", "40", "ignored-extra"])?,
            Tag::expiration(Timestamp::from(40)),
        ])
        .custom_created_at(Timestamp::from(10))
        .finalize(&keys)?;
    let deletion = EventBuilder::new(Kind::EventDeletion, "")
        .tags([
            Tag::parse(["e"])?,
            Tag::parse(["e", "not-an-event-id"])?,
            Tag::parse(["x-hostile", "unrelated"])?,
            Tag::parse(["e", &target.id.to_hex(), "unused", "ignored-extra"])?,
            Tag::event(target.id),
        ])
        .custom_created_at(Timestamp::from(20))
        .finalize(&keys)?;
    assert!(deletion_applies(
        (
            deletion.pubkey,
            deletion.kind,
            deletion.created_at,
            deletion.tags.as_slice()
        ),
        (
            target.id,
            target.pubkey,
            target.kind,
            target.created_at,
            target.tags.as_slice()
        ),
    ));
    assert!(event_is_expired(
        target.tags.as_slice(),
        Timestamp::from(40)
    ));
    let malformed_only = [
        Tag::parse(["expiration"])?,
        Tag::parse(["expiration", "still-not-a-timestamp"])?,
        Tag::parse(["x-hostile", "unrelated"])?,
    ];
    assert!(!event_is_expired(&malformed_only, Timestamp::from(40)));

    Ok(())
}

#[test]
fn valid_address_deletion_survives_malformed_siblings() -> Result<(), Box<dyn std::error::Error>> {
    let keys = Keys::generate();
    let addressable = EventBuilder::new(Kind::from_u16(30_000), "addressable")
        .tag(Tag::identifier("photos"))
        .custom_created_at(Timestamp::from(10))
        .finalize(&keys)?;
    let coordinate = format!("30000:{}:photos", keys.public_key());
    let address_deletion = EventBuilder::new(Kind::EventDeletion, "")
        .tags([
            Tag::parse(["a"])?,
            Tag::parse(["a", "not-a-coordinate"])?,
            Tag::parse(["x-hostile", "unrelated"])?,
            Tag::parse(["a", &coordinate, "ignored-extra"])?,
        ])
        .custom_created_at(Timestamp::from(20))
        .finalize(&keys)?;
    assert!(deletion_applies(
        (
            address_deletion.pubkey,
            address_deletion.kind,
            address_deletion.created_at,
            address_deletion.tags.as_slice(),
        ),
        (
            addressable.id,
            addressable.pubkey,
            addressable.kind,
            addressable.created_at,
            addressable.tags.as_slice(),
        ),
    ));

    Ok(())
}

#[test]
fn malformed_or_unauthorized_deletions_never_apply() -> Result<(), Box<dyn std::error::Error>> {
    let keys = Keys::generate();
    let target = EventBuilder::new(Kind::TextNote, "target")
        .custom_created_at(Timestamp::from(10))
        .finalize(&keys)?;
    let malformed_deletion = EventBuilder::new(Kind::EventDeletion, "")
        .tags([
            Tag::parse(["e"])?,
            Tag::parse(["e", "not-an-event-id"])?,
            Tag::parse(["a", "not-a-coordinate"])?,
        ])
        .custom_created_at(Timestamp::from(20))
        .finalize(&keys)?;
    assert!(!deletion_applies(
        (
            malformed_deletion.pubkey,
            malformed_deletion.kind,
            malformed_deletion.created_at,
            malformed_deletion.tags.as_slice(),
        ),
        (
            target.id,
            target.pubkey,
            target.kind,
            target.created_at,
            target.tags.as_slice(),
        ),
    ));

    let attacker = Keys::generate();
    let unauthorized = EventBuilder::new(Kind::EventDeletion, "")
        .tag(Tag::event(target.id))
        .custom_created_at(Timestamp::from(20))
        .finalize(&attacker)?;
    assert!(!deletion_applies(
        (
            unauthorized.pubkey,
            unauthorized.kind,
            unauthorized.created_at,
            unauthorized.tags.as_slice(),
        ),
        (
            target.id,
            target.pubkey,
            target.kind,
            target.created_at,
            target.tags.as_slice(),
        ),
    ));
    Ok(())
}
