//! Nostr event-state corpus shared by state and cache providers.

use fava_state::{
    CacheMutation, CachedEvent, RelayAccess, RelayEvidence, RelaySessionKey, RelayUrl, Timestamp,
    admission_mutations, candidate_is_newer, expiration_mutations,
};
use nostr::event::{Event, EventBuilder, FinalizeEvent, Kind, Tag};
use nostr::key::Keys;

fn cached_at(event: Event, relay: &str, observed_at: u64) -> CachedEvent {
    CachedEvent::new(
        event,
        RelayEvidence::one(
            RelaySessionKey::new(
                RelayUrl::parse(relay).expect("relay url"),
                RelayAccess::public(),
            ),
            Timestamp::from(observed_at),
        ),
    )
}

fn apply_admission(current: &mut Vec<CachedEvent>, incoming: CachedEvent) -> bool {
    let mutations = admission_mutations(current, incoming, Timestamp::from(100));
    let changed = !mutations.is_empty();
    for mutation in mutations {
        match mutation {
            CacheMutation::Upsert(incoming) => {
                if let Some(known) = current
                    .iter_mut()
                    .find(|known| known.event.id == incoming.event.id)
                {
                    known.merge_evidence(&incoming.evidence);
                } else {
                    current.push(incoming);
                }
            }
            CacheMutation::Retract(id) => current.retain(|known| known.event.id != id),
        }
    }
    changed
}

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

#[test]
fn replaceable_winners_are_independent_per_relay_url() {
    let keys = Keys::generate();
    let kind = Kind::from_u16(30_001);
    let coordinate = || vec![Tag::identifier("same")];
    let relay_a = "wss://relay-a.example";
    let relay_b = "wss://relay-b.example";
    let old_a = event(&keys, kind, 10, "old A", coordinate());
    let winner_b = event(&keys, kind, 20, "winner B", coordinate());
    let new_a = event(&keys, kind, 30, "new A", coordinate());
    let mut current = Vec::new();

    assert!(apply_admission(
        &mut current,
        cached_at(old_a.clone(), relay_a, 1)
    ));
    assert!(apply_admission(
        &mut current,
        cached_at(winner_b.clone(), relay_b, 2)
    ));
    assert_eq!(
        current
            .iter()
            .map(|known| known.event.id)
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([old_a.id, winner_b.id])
    );

    assert!(apply_admission(
        &mut current,
        cached_at(new_a.clone(), relay_a, 3)
    ));
    assert_eq!(
        current
            .iter()
            .map(|known| known.event.id)
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([winner_b.id, new_a.id])
    );
    assert!(!apply_admission(
        &mut current,
        cached_at(new_a, relay_a, 3)
    ));
}

#[test]
fn duplicate_id_merges_exact_relay_evidence() {
    let keys = Keys::generate();
    let kind = Kind::from_u16(30_001);
    let shared = event(
        &keys,
        kind,
        20,
        "shared",
        vec![Tag::identifier("same")],
    );
    let newer = event(
        &keys,
        kind,
        30,
        "newer A",
        vec![Tag::identifier("same")],
    );
    let relay_a = "wss://relay-a.example";
    let relay_b = "wss://relay-b.example";
    let mut current = Vec::new();

    assert!(apply_admission(
        &mut current,
        cached_at(shared.clone(), relay_a, 1)
    ));
    assert!(apply_admission(
        &mut current,
        cached_at(shared.clone(), relay_b, 2)
    ));
    assert!(apply_admission(
        &mut current,
        cached_at(newer.clone(), relay_a, 3)
    ));

    let retained_shared = current
        .iter()
        .find(|known| known.event.id == shared.id)
        .expect("the shared id remains B's winner");
    assert_eq!(retained_shared.evidence.len(), 2);
    assert_eq!(current.len(), 2);

    assert!(apply_admission(
        &mut current,
        cached_at(newer.clone(), relay_b, 4)
    ));
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].event.id, newer.id);
    assert_eq!(current[0].evidence.len(), 2);
    assert!(!apply_admission(
        &mut current,
        cached_at(newer, relay_b, 4)
    ));
}
