//! Nostr event-state corpus shared by state and cache providers.

use fava_state::{
    CacheMutation, CachedEvent, RelayAccess, RelayEvidence, RelaySessionKey, RelayUrl, Timestamp,
    admission_mutations, candidate_is_newer, expiration_mutations,
};
use nostr::event::{Event, EventBuilder, FinalizeEvent, Kind, Tag};
use nostr::key::Keys;

fn event(keys: &Keys, kind: Kind, at: u64, content: &str, tags: Vec<Tag>) -> Event {
    EventBuilder::new(kind, content)
        .tags(tags)
        .custom_created_at(Timestamp::from(at))
        .finalize(keys)
        .expect("event signs")
}

fn cached(event: Event) -> CachedEvent {
    CachedEvent::new(
        event,
        RelayEvidence::one(
            RelaySessionKey::new(
                RelayUrl::parse("wss://relay.example").expect("relay url"),
                RelayAccess::public(),
            ),
            Timestamp::from(100),
        ),
    )
}

#[test]
fn replaceable_tie_keeps_the_lowest_event_id() {
    let keys = Keys::generate();
    let left = event(&keys, Kind::Metadata, 10, "left", Vec::new());
    let right = event(&keys, Kind::Metadata, 10, "right", Vec::new());
    let (lower, higher) = if left.id < right.id {
        (left, right)
    } else {
        (right, left)
    };

    assert!(candidate_is_newer(&lower, &higher));
    assert!(!candidate_is_newer(&higher, &lower));
}

#[test]
fn authorized_deletion_retracts_and_prevents_resurrection() {
    let alice = Keys::generate();
    let target = event(&alice, Kind::TextNote, 10, "target", Vec::new());
    let deletion = event(
        &alice,
        Kind::EventDeletion,
        20,
        "",
        vec![Tag::event(target.id)],
    );
    let target_cached = cached(target.clone());
    let deletion_cached = cached(deletion.clone());

    assert_eq!(
        admission_mutations(
            std::slice::from_ref(&target_cached),
            deletion_cached.clone(),
            Timestamp::from(20)
        ),
        vec![
            CacheMutation::Upsert(deletion_cached.clone()),
            CacheMutation::Retract(target.id),
        ]
    );
    assert!(admission_mutations(&[deletion_cached], target_cached, Timestamp::from(21)).is_empty());
}

#[test]
fn another_author_cannot_delete_an_event() {
    let alice = Keys::generate();
    let mallory = Keys::generate();
    let target = event(&alice, Kind::TextNote, 10, "target", Vec::new());
    let deletion = cached(event(
        &mallory,
        Kind::EventDeletion,
        20,
        "",
        vec![Tag::event(target.id)],
    ));

    assert_eq!(
        admission_mutations(&[cached(target)], deletion.clone(), Timestamp::from(20)),
        vec![CacheMutation::Upsert(deletion)]
    );
}

#[test]
fn expiration_refuses_and_retracts_at_the_declared_timestamp() {
    let keys = Keys::generate();
    let expiring = cached(event(
        &keys,
        Kind::TextNote,
        10,
        "temporary",
        vec![Tag::expiration(Timestamp::from(20))],
    ));

    assert!(admission_mutations(&[], expiring.clone(), Timestamp::from(20)).is_empty());
    assert_eq!(
        expiration_mutations(std::slice::from_ref(&expiring), Timestamp::from(20)),
        vec![CacheMutation::Retract(expiring.event.id)]
    );
}
