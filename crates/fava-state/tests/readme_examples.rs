//! Executable examples for every public state value and rule.

use fava_relay::{RelayAccess, RelaySessionKey};
use fava_state::{
    EventCoordinate, EventStateMutation, RelayEvent, RetractionCause, deletion_applies,
    event_coordinate, event_is_expired, event_is_newer, mutations_for_event,
    mutations_for_expiration, relay_occurrences_for_event,
};
use nostr::event::{EventBuilder, FinalizeEvent, Kind, Tag};
use nostr::key::Keys;
use nostr::types::{RelayUrl, Timestamp};

#[test]
fn public_state_examples_cover_the_complete_contract() -> Result<(), Box<dyn std::error::Error>> {
    let keys = Keys::generate();
    let session = RelaySessionKey {
        relay: RelayUrl::parse("wss://relay.example")?,
        access: RelayAccess::Public,
    };
    let old = EventBuilder::new(Kind::Metadata, "old")
        .custom_created_at(Timestamp::from(1))
        .finalize(&keys)?;
    let new = EventBuilder::new(Kind::Metadata, "new")
        .custom_created_at(Timestamp::from(2))
        .finalize(&keys)?;

    assert!(matches!(
        event_coordinate(old.id, old.pubkey, old.kind, old.tags.as_slice()),
        EventCoordinate::Replaceable { .. }
    ));
    assert!(event_is_newer(
        (new.created_at, new.id),
        (old.created_at, old.id)
    ));

    let old_occurrence = RelayEvent::new(old.clone(), session.clone(), Timestamp::from(3));
    let occurrences =
        relay_occurrences_for_event(old.id, std::slice::from_ref(&old_occurrence)).unwrap();
    assert_eq!(occurrences.event_id(), old.id);
    assert!(!occurrences.is_empty());
    assert_eq!(occurrences.len(), 1);
    assert_eq!(occurrences.occurrences().next().unwrap().session, session);

    let incoming = RelayEvent::new(new.clone(), session.clone(), Timestamp::from(4));
    let mutations = mutations_for_event(&[old_occurrence], incoming.clone(), Timestamp::from(4));
    assert!(matches!(
        &mutations[0],
        EventStateMutation::Retract {
            event_id,
            session: removed_session,
            cause: RetractionCause::Superseded { by },
        } if *event_id == old.id && *removed_session == session && *by == new.id
    ));
    assert_eq!(mutations[1], EventStateMutation::Upsert(incoming));

    let expiring = EventBuilder::new(Kind::TextNote, "temporary")
        .tag(Tag::expiration(Timestamp::from(10)))
        .finalize(&keys)?;
    assert!(event_is_expired(
        expiring.tags.as_slice(),
        Timestamp::from(10)
    ));
    let expiring = RelayEvent::new(expiring, session.clone(), Timestamp::from(5));
    assert!(matches!(
        mutations_for_expiration(&[expiring], Timestamp::from(10)).as_slice(),
        [EventStateMutation::Retract {
            cause: RetractionCause::Expired,
            ..
        }]
    ));

    let target = EventBuilder::new(Kind::TextNote, "target")
        .custom_created_at(Timestamp::from(6))
        .finalize(&keys)?;
    let deletion = EventBuilder::new(Kind::EventDeletion, "")
        .tag(Tag::event(target.id))
        .custom_created_at(Timestamp::from(7))
        .finalize(&keys)?;
    assert!(deletion_applies(
        (
            deletion.pubkey,
            deletion.kind,
            deletion.created_at,
            deletion.tags.as_slice(),
        ),
        (
            target.id,
            target.pubkey,
            target.kind,
            target.created_at,
            target.tags.as_slice(),
        ),
    ));
    let causes = [
        RetractionCause::Deleted {
            deletion: deletion.id,
        },
        RetractionCause::Evicted,
    ];
    assert_eq!(causes.len(), 2);
    Ok(())
}
