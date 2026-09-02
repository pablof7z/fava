//! Universal event-state model behavior.

use fava_state::{
    EventCoordinate, EventStateMutation, RelayEvent, RetractionCause, event_coordinate,
    event_is_newer, mutations_for_event, mutations_for_expiration, relay_occurrences_for_event,
};
use nostr::event::{Event, EventBuilder, FinalizeEvent, Kind, Tag};
use nostr::key::Keys;
use nostr::types::{RelayUrl, Timestamp};

fn session(raw: &str) -> RelayUrl {
    RelayUrl::parse(raw).expect("relay URL")
}

fn event(keys: &Keys, kind: Kind, at: u64, content: &str, tags: Vec<Tag>) -> Event {
    EventBuilder::new(kind, content)
        .tags(tags)
        .custom_created_at(Timestamp::from(at))
        .finalize(keys)
        .expect("signed event")
}

fn observed(event: Event, session: &RelayUrl, at: u64) -> RelayEvent {
    RelayEvent::new(
        event,
        session.clone(),
        fava_relay::Authority::Unauthenticated,
        Timestamp::from(at),
    )
}

#[test]
fn coordinate_and_equal_time_order_are_universal() {
    let keys = Keys::generate();
    let left = event(&keys, Kind::Metadata, 10, "left", Vec::new());
    let right = event(&keys, Kind::Metadata, 10, "right", Vec::new());
    let (lower, higher) = if left.id < right.id {
        (left, right)
    } else {
        (right, left)
    };
    assert!(event_is_newer(
        (lower.created_at, lower.id),
        (higher.created_at, higher.id),
    ));
    assert_eq!(
        event_coordinate(lower.id, lower.pubkey, lower.kind, lower.tags.as_slice()),
        EventCoordinate::Replaceable {
            author: keys.public_key(),
            kind: Kind::Metadata,
            identifier: None,
        }
    );
}

#[test]
fn occurrences_are_event_bound_relay_exact_and_keep_earliest_time() {
    let author = Keys::generate();
    let shared = event(&author, Kind::TextNote, 1, "shared", Vec::new());
    let other = event(&author, Kind::TextNote, 2, "other", Vec::new());
    let one = session("wss://one.example");
    let two = session("wss://two.example");
    let contributions = vec![
        observed(shared.clone(), &one, 9),
        observed(shared.clone(), &one, 3),
        observed(shared.clone(), &two, 5),
    ];
    let occurrences = relay_occurrences_for_event(shared.id, &contributions).expect("same id");
    assert_eq!(occurrences.event_id(), shared.id);
    assert_eq!(occurrences.len(), 2);
    assert!(occurrences.len() <= contributions.len());
    assert_eq!(
        occurrences
            .occurrences()
            .find(|item| item.session == one)
            .expect("one")
            .observed_at,
        Timestamp::from(3),
    );
    assert!(
        relay_occurrences_for_event(
            shared.id,
            &[contributions[0].clone(), observed(other, &one, 1)],
        )
        .is_none()
    );
}

#[test]
fn immutable_replay_preserves_the_earliest_live_occurrence() {
    let keys = Keys::generate();
    let one = session("wss://relay.example");
    let immutable = event(&keys, Kind::TextNote, 1, "immutable", Vec::new());
    let earliest = observed(immutable.clone(), &one, 3);
    let later_replay = observed(immutable.clone(), &one, 9);
    assert!(
        mutations_for_event(
            std::slice::from_ref(&earliest),
            later_replay,
            Timestamp::from(9),
        )
        .is_empty()
    );

    let earlier_replay = observed(immutable, &one, 1);
    assert_eq!(
        mutations_for_event(
            std::slice::from_ref(&earliest),
            earlier_replay.clone(),
            Timestamp::from(9),
        ),
        vec![EventStateMutation::Upsert(earlier_replay)]
    );
}

#[test]
fn replacement_is_partitioned_by_exact_session_and_names_successor() {
    let keys = Keys::generate();
    let one = session("wss://one.example");
    let two = session("wss://two.example");
    let old = event(&keys, Kind::Metadata, 1, "old", Vec::new());
    let new = event(&keys, Kind::Metadata, 2, "new", Vec::new());
    let old_one = observed(old.clone(), &one, 1);
    let old_two = observed(old.clone(), &two, 1);
    let incoming = observed(new.clone(), &one, 2);
    assert_eq!(
        mutations_for_event(&[old_one, old_two], incoming.clone(), Timestamp::from(3),),
        vec![
            EventStateMutation::Retract {
                event_id: old.id,
                session: one,
                cause: RetractionCause::Superseded { by: new.id },
            },
            EventStateMutation::Upsert(incoming),
        ]
    );
}

#[test]
fn deletion_and_expiration_retract_every_exact_contribution() {
    let keys = Keys::generate();
    let a = session("wss://a.example");
    let b = session("wss://b.example");
    let target = event(&keys, Kind::TextNote, 10, "target", Vec::new());
    let deletion = event(
        &keys,
        Kind::EventDeletion,
        20,
        "",
        vec![Tag::event(target.id)],
    );
    let current = vec![
        observed(target.clone(), &a, 10),
        observed(target.clone(), &b, 11),
    ];
    let incoming = observed(deletion.clone(), &a, 20);
    let mutations = mutations_for_event(&current, incoming.clone(), Timestamp::from(20));
    assert_eq!(mutations.len(), 3);
    assert!(mutations.ends_with(&[EventStateMutation::Upsert(incoming)]));

    let expiring = event(
        &keys,
        Kind::TextNote,
        30,
        "temporary",
        vec![Tag::expiration(Timestamp::from(40))],
    );
    assert_eq!(
        mutations_for_expiration(
            &[
                observed(expiring.clone(), &a, 30),
                observed(expiring.clone(), &b, 31)
            ],
            Timestamp::from(40),
        )
        .len(),
        2,
    );
}
