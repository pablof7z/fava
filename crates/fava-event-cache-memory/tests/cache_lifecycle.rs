//! Bounded-cache deletion, expiry sweep, serialization, and local-write exclusion.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::Barrier;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_relay::Authority;
use fava_state::RelayEvent;
use nostr::event::Event;
use nostr::event::{EventBuilder, FinalizeEvent, Kind, Tag};
use nostr::key::Keys;
use nostr::types::{RelayUrl, Timestamp};

fn cached(event: Event, at: u64) -> RelayEvent {
    RelayEvent::new(
        event,
        RelayUrl::parse("wss://relay.example").expect("relay url"),
        Authority::Unauthenticated,
        Timestamp::from(at),
    )
}

fn note(keys: &Keys, at: u64, content: &str, tags: Vec<Tag>) -> Event {
    EventBuilder::new(Kind::TextNote, content)
        .tags(tags)
        .custom_created_at(Timestamp::from(at))
        .finalize(keys)
        .expect("event signs")
}

#[test]
fn a_full_cache_still_applies_a_deletion() {
    let cache = MemoryEventCache::bounded(NonZeroUsize::new(2).expect("non-zero"));
    let keys = Keys::generate();
    let first = note(&keys, 10, "first", Vec::new());
    let second = note(&keys, 11, "second", Vec::new());
    for event in [first.clone(), second.clone()] {
        assert!(
            cache
                .admit(cached(event, 12), Timestamp::from(12))
                .expect("admission commits")
        );
    }
    assert_eq!(cache.len().expect("cache readable"), 2);

    let deletion = EventBuilder::new(Kind::EventDeletion, "")
        .tag(Tag::event(first.id))
        .custom_created_at(Timestamp::from(20))
        .finalize(&keys)
        .expect("deletion signs");
    let deletion_id = deletion.id;

    assert!(
        cache
            .admit(cached(deletion, 20), Timestamp::from(20))
            .expect("a deletion is always applicable"),
    );

    assert!(
        cache.event(first.id).expect("cache readable").is_none(),
        "the deleted target must be gone"
    );
    assert!(
        cache.event(deletion_id).expect("cache readable").is_some(),
        "the kind-5 event is the retained tombstone that blocks resurrection"
    );
    assert!(
        !cache
            .admit(cached(first, 21), Timestamp::from(21))
            .expect("readmission decides"),
        "the retained deletion must refuse resurrection"
    );

    // A deletion that retracts nothing retained still has to land, or a full
    // cache silently loses the tombstone that blocks a later resurrection.
    let stranger = note(&keys, 5, "never observed", Vec::new());
    let untargeted = EventBuilder::new(Kind::EventDeletion, "")
        .tag(Tag::event(stranger.id))
        .custom_created_at(Timestamp::from(22))
        .finalize(&keys)
        .expect("deletion signs");
    let untargeted_id = untargeted.id;
    assert!(
        cache
            .admit(cached(untargeted, 22), Timestamp::from(22))
            .expect("a deletion is always applicable at capacity"),
    );
    assert!(
        cache
            .event(untargeted_id)
            .expect("cache readable")
            .is_some()
    );
    assert!(
        !cache
            .admit(cached(stranger, 23), Timestamp::from(23))
            .expect("readmission decides"),
        "the tombstone admitted under capacity pressure must still block resurrection"
    );
    assert_eq!(cache.len().expect("cache readable"), 2);
}

#[test]
fn admission_sweeps_expired_events() {
    let cache = MemoryEventCache::default();
    let keys = Keys::generate();
    let expiring = note(
        &keys,
        10,
        "temporary",
        vec![Tag::expiration(Timestamp::from(20))],
    );
    assert!(
        cache
            .admit(cached(expiring.clone(), 11), Timestamp::from(11))
            .expect("admission commits")
    );

    let later = note(&keys, 30, "later", Vec::new());
    assert!(
        cache
            .admit(cached(later, 30), Timestamp::from(30))
            .expect("admission commits")
    );

    assert!(
        cache.event(expiring.id).expect("cache readable").is_none(),
        "every production admission must sweep NIP-40 expiry"
    );
    assert_eq!(cache.len().expect("cache readable"), 1);
}

#[test]
fn transact_holds_exclusive_write_authority_across_its_whole_decision() {
    let cache = Arc::new(MemoryEventCache::default());
    let keys = Keys::generate();
    let other = note(&keys, 10, "other writer", Vec::new());
    let released = Arc::new(Barrier::new(2));
    let committed = Arc::new(AtomicBool::new(false));

    let writer = {
        let cache = Arc::clone(&cache);
        let released = Arc::clone(&released);
        let committed = Arc::clone(&committed);
        thread::spawn(move || {
            released.wait();
            cache
                .admit(cached(other, 11), Timestamp::from(11))
                .expect("admission commits");
            committed.store(true, Ordering::SeqCst);
        })
    };

    cache
        .transact(&|_current| {
            released.wait();
            thread::sleep(Duration::from_millis(200));
            assert!(
                !committed.load(Ordering::SeqCst),
                "a second writer committed between this decision's read and its commit"
            );
            Vec::new()
        })
        .expect("decision completes");
    writer.join().expect("writer finishes");
    assert!(committed.load(Ordering::SeqCst));
}

#[test]
fn concurrent_admissions_keep_one_replaceable_winner() {
    let cache = Arc::new(MemoryEventCache::default());
    let keys = Arc::new(Keys::generate());
    let kind = Kind::from_u16(30_001);
    let admitted = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();

    for worker in 0..8_u64 {
        let cache = Arc::clone(&cache);
        let keys = Arc::clone(&keys);
        let admitted = Arc::clone(&admitted);
        handles.push(thread::spawn(move || {
            for round in 0..40_u64 {
                let at = 100 + worker * 40 + round;
                let event = EventBuilder::new(kind, format!("{worker}:{round}"))
                    .tag(Tag::identifier("same"))
                    .custom_created_at(Timestamp::from(at))
                    .finalize(keys.as_ref())
                    .expect("event signs");
                if cache
                    .admit(cached(event, at), Timestamp::from(at))
                    .expect("admission commits")
                {
                    admitted.fetch_add(1, Ordering::Relaxed);
                }
            }
        }));
    }
    for handle in handles {
        handle.join().expect("worker finishes");
    }

    assert!(admitted.load(Ordering::Relaxed) > 0);
    assert_eq!(
        cache.len().expect("cache readable"),
        1,
        "one serialized event-state writer must leave exactly one winner per coordinate"
    );
    let committed = cache
        .transact(&|current| {
            assert_eq!(current.len(), 1, "transact reads the serialized state");
            Vec::new()
        })
        .expect("an empty decision commits nothing");
    assert_eq!(committed, 0);
}

/// A relay is one occurrence now, whatever authority observed it under:
/// access stopped being part of a relay's identity. A later admission of the
/// same event at the same relay only replaces the occurrence when it is
/// strictly earlier; an equal-time admission under a different authority
/// changes nothing, and the earlier authority survives.
#[test]
fn same_relay_admissions_under_different_authority_are_one_occurrence() {
    let cache = MemoryEventCache::default();
    let keys = Keys::generate();
    let event = note(&keys, 10, "served under two authorities", Vec::new());
    let relay = RelayUrl::parse("wss://same.example").unwrap();
    let authenticated = Keys::generate().public_key();

    assert!(
        cache
            .admit(
                RelayEvent::new(
                    event.clone(),
                    relay.clone(),
                    Authority::Unauthenticated,
                    Timestamp::from(11),
                ),
                Timestamp::from(11),
            )
            .unwrap()
    );
    assert!(
        !cache
            .admit(
                RelayEvent::new(
                    event,
                    relay,
                    Authority::As(authenticated),
                    Timestamp::from(11),
                ),
                Timestamp::from(11),
            )
            .unwrap(),
        "an equal-time admission at an already-occupied relay changes nothing"
    );
    cache
        .transact(&|current| {
            assert_eq!(current.len(), 1, "one relay is one occurrence");
            assert_eq!(
                current[0].occurrence().authority,
                Authority::Unauthenticated
            );
            Vec::new()
        })
        .unwrap();
}
