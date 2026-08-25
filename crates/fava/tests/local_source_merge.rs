//! Public-facade acceptance evidence for the first local-source vertical slice.

use std::sync::Arc;
use std::time::Duration;

use fava::{Fava, Query, SingleLetterTag};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_state::{EventStateMutation, RelayEvent};
use fava_write::EventValue;
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{
    Event, EventBuilder, FinalizeEvent, FinalizeUnsignedEvent, Kind, Tag, UnsignedEvent,
};
use nostr::key::Keys;
use nostr::types::{RelayUrl, Timestamp};
use tokio::time::timeout;

fn assembly() -> (Fava, Arc<MemoryEventCache>, Arc<MemoryWriteStore>) {
    let cache = Arc::new(MemoryEventCache::default());
    let writes = Arc::new(MemoryWriteStore::default());
    let fava = Fava::builder()
        .event_cache(Arc::clone(&cache))
        .write_store(Arc::clone(&writes))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .build()
        .expect("complete local assembly");
    (fava, cache, writes)
}

fn signed_event(keys: &Keys, kind: Kind, created_at: u64, content: &str) -> Event {
    EventBuilder::new(kind, content)
        .custom_created_at(Timestamp::from(created_at))
        .finalize(keys)
        .expect("test event signs")
}

fn unsigned_event(
    keys: &Keys,
    kind: Kind,
    created_at: u64,
    content: impl Into<String>,
) -> UnsignedEvent {
    let mut event = EventBuilder::new(kind, content)
        .custom_created_at(Timestamp::from(created_at))
        .finalize_unsigned(keys.public_key());
    event.ensure_id();
    event
}

fn signed_event_with_tags(keys: &Keys, created_at: u64, content: &str, tags: Vec<Tag>) -> Event {
    EventBuilder::new(Kind::TextNote, content)
        .tags(tags)
        .custom_created_at(Timestamp::from(created_at))
        .finalize(keys)
        .expect("test event signs")
}

fn unsigned_event_with_tags(
    keys: &Keys,
    created_at: u64,
    content: &str,
    tags: Vec<Tag>,
) -> UnsignedEvent {
    let mut event = EventBuilder::new(Kind::TextNote, content)
        .tags(tags)
        .custom_created_at(Timestamp::from(created_at))
        .finalize_unsigned(keys.public_key());
    event.ensure_id();
    event
}

fn literal_tag(key: char, value: &str, later_cells: &[&str]) -> Tag {
    let mut cells = vec![key.to_string(), value.to_owned()];
    cells.extend(later_cells.iter().map(|cell| (*cell).to_owned()));
    Tag::parse(cells).expect("valid literal tag")
}

#[derive(Clone)]
struct TestRelayOccurrence {
    session: RelaySessionKey,
    observed_at: Timestamp,
}

fn occurrence(relay: &str, observed_at: u64) -> TestRelayOccurrence {
    TestRelayOccurrence {
        session: RelaySessionKey {
            relay: RelayUrl::parse(relay).expect("test relay url"),
            access: RelayAccess::Public,
        },
        observed_at: Timestamp::from(observed_at),
    }
}

fn relay_event(event: Event, occurrence: TestRelayOccurrence) -> RelayEvent {
    RelayEvent::new(event, occurrence.session, occurrence.observed_at)
}

async fn next_snapshot(feed: &mut fava_observe::Observation) -> Arc<fava::QuerySnapshot> {
    timeout(Duration::from_secs(1), feed.changed())
        .await
        .expect("observation update arrives within bound")
        .expect("observation remains open")
}

#[tokio::test(flavor = "current_thread")]
async fn accepted_local_event_is_visible_without_cache_pollution() {
    let (fava, cache, writes) = assembly();
    let keys = Keys::generate();
    let unsigned = unsigned_event(&keys, Kind::TextNote, 10, "local");
    let id = unsigned.id.expect("builder computes id");
    let mut feed = fava
        .observe(Query::events().cache_only())
        .await
        .expect("query opens from local sources");
    assert!(feed.current().events.is_empty());

    let accepted = writes
        .accept_materialized(EventValue::Unsigned(unsigned))
        .expect("write-store provider accepts finalized local event");
    let visible = next_snapshot(&mut feed).await;

    assert_eq!(visible.events.len(), 1);
    assert_eq!(visible.events[0].id(), id);
    assert_eq!(
        visible.events[0]
            .publication()
            .map(|publication| publication.receipt_id),
        Some(accepted.receipt_id)
    );
    assert!(cache.is_empty().expect("cache remains readable"));

    assert!(
        fava.cancel_write(accepted.receipt_id)
            .expect("facade cancels")
    );
    assert!(next_snapshot(&mut feed).await.events.is_empty());
    assert!(cache.is_empty().expect("cache remains unchanged"));
}

#[tokio::test(flavor = "current_thread")]
async fn cancelling_local_replacement_reveals_cached_predecessor() {
    let (fava, cache, writes) = assembly();
    let keys = Keys::generate();
    let predecessor = signed_event(&keys, Kind::ContactList, 10, "cached");
    cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            predecessor.clone(),
            occurrence("wss://relay.example", 1),
        ))])
        .expect("cache mutation commits");
    let successor = unsigned_event(&keys, Kind::ContactList, 20, "local");
    let successor_id = successor.id.expect("builder computes id");
    let accepted = writes
        .accept_materialized(EventValue::Unsigned(successor))
        .expect("local successor accepts");
    let query = Query::events()
        .kinds([Kind::ContactList])
        .expect("one kind is bounded")
        .cache_only();
    let mut feed = fava.observe(query).await.expect("query opens");

    assert_eq!(feed.current().events.len(), 1);
    assert_eq!(feed.current().events[0].id(), successor_id);

    writes
        .cancel(accepted.receipt_id)
        .expect("local cancellation commits");
    let revealed = next_snapshot(&mut feed).await;
    assert_eq!(revealed.events.len(), 1);
    assert_eq!(revealed.events[0].id(), predecessor.id);
    assert!(
        cache
            .event(predecessor.id)
            .expect("cache readable")
            .is_some()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn relay_echo_enriches_one_record_without_erasing_receipt() {
    let (fava, cache, writes) = assembly();
    let keys = Keys::generate();
    let event = signed_event(&keys, Kind::TextNote, 10, "echo");
    let accepted = writes
        .accept_materialized(EventValue::Signed(event.clone()))
        .expect("signed local event accepts");
    cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            event.clone(),
            occurrence("wss://relay-a.example", 1),
        ))])
        .expect("first relay evidence commits");
    let mut feed = fava
        .observe(Query::events().cache_only())
        .await
        .expect("query opens");
    assert_eq!(feed.current().events.len(), 1);
    assert_eq!(feed.current().events[0].relay_occurrences().len(), 1);

    cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            event,
            occurrence("wss://relay-b.example", 2),
        ))])
        .expect("second relay evidence merges");
    let enriched = next_snapshot(&mut feed).await;

    assert_eq!(enriched.events.len(), 1);
    assert_eq!(enriched.events[0].relay_occurrences().len(), 2);
    assert_eq!(
        enriched.events[0]
            .publication()
            .map(|publication| publication.receipt_id),
        Some(accepted.receipt_id)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn acquisition_only_and_provenance_constraint_stay_distinct() {
    let (fava, cache, _writes) = assembly();
    let keys = Keys::generate();
    let event = signed_event(&keys, Kind::TextNote, 10, "authority");
    cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            event.clone(),
            occurrence("wss://other.example", 1),
        ))])
        .expect("other relay evidence commits");
    let asked = RelayUrl::parse("wss://asked.example").expect("relay url");
    let from = Query::events()
        .from_relays([asked.clone()])
        .expect("non-empty relay set")
        .cache_only();
    let only = Query::events()
        .only_from_relays([asked])
        .expect("non-empty relay set")
        .cache_only();
    let from_feed = fava
        .observe(from)
        .await
        .expect("acquisition-only query opens");
    let mut only_feed = fava.observe(only).await.expect("authority query opens");

    assert_eq!(from_feed.current().events.len(), 1);
    assert!(only_feed.current().events.is_empty());

    cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            event,
            occurrence("wss://asked.example", 2),
        ))])
        .expect("qualifying relay evidence merges");
    assert_eq!(next_snapshot(&mut only_feed).await.events.len(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn slow_consumer_receives_exact_latest_state_with_bounded_delivery() {
    let (fava, _cache, writes) = assembly();
    let keys = Keys::generate();
    let mut feed = fava
        .observe(Query::events().cache_only())
        .await
        .expect("query opens");

    for index in 1..=3 {
        let event = unsigned_event(&keys, Kind::TextNote, index, format!("event {index}"));
        writes
            .accept_materialized(EventValue::Unsigned(event))
            .expect("local event accepts");
    }

    let latest = next_snapshot(&mut feed).await;
    assert_eq!(latest.events.len(), 3);
    assert_eq!(latest.events[0].created_at(), Timestamp::from(3));
}

#[tokio::test(flavor = "current_thread")]
async fn deletion_and_expiration_update_the_same_open_query() {
    let (fava, cache, _writes) = assembly();
    let keys = Keys::generate();
    let deleted = signed_event(&keys, Kind::TextNote, 10, "deleted");
    let expiring = EventBuilder::new(Kind::TextNote, "expiring")
        .tag(Tag::expiration(Timestamp::from(30)))
        .custom_created_at(Timestamp::from(11))
        .finalize(&keys)
        .expect("event signs");
    for event in [deleted.clone(), expiring.clone()] {
        assert!(
            cache
                .admit(
                    relay_event(event, occurrence("wss://relay.example", 15)),
                    Timestamp::from(15),
                )
                .expect("event admission commits")
        );
    }
    let mut feed = fava
        .observe(
            Query::events()
                .kinds([Kind::TextNote])
                .expect("one kind is bounded")
                .cache_only(),
        )
        .await
        .expect("query opens");
    assert_eq!(feed.current().events.len(), 2);

    let deletion = EventBuilder::new(Kind::EventDeletion, "")
        .tag(Tag::event(deleted.id))
        .custom_created_at(Timestamp::from(20))
        .finalize(&keys)
        .expect("deletion signs");
    cache
        .admit(
            relay_event(deletion, occurrence("wss://relay.example", 20)),
            Timestamp::from(20),
        )
        .expect("deletion admission commits");
    let after_deletion = next_snapshot(&mut feed).await;
    assert_eq!(after_deletion.events.len(), 1);
    assert_eq!(after_deletion.events[0].id(), expiring.id);

    assert_eq!(
        cache.expire(Timestamp::from(30)).expect("expiry commits"),
        1
    );
    assert!(next_snapshot(&mut feed).await.events.is_empty());
}

#[tokio::test(flavor = "current_thread")]
// Keep the public cache-only result and source-evidence counterexamples in one
// observation lifecycle so the facade proof cannot pass on isolated fixtures.
#[allow(clippy::too_many_lines)]
async fn literal_tag_selection_preserves_exact_sources_through_public_observation() {
    let (fava, cache, writes) = assembly();
    let keys = Keys::generate();
    let signed = signed_event_with_tags(
        &keys,
        40,
        "signed exact",
        vec![
            literal_tag('e', "café", &["ignored"]),
            literal_tag('P', "CaseSensitive", &[]),
        ],
    );
    let unsigned = unsigned_event_with_tags(
        &keys,
        41,
        "unsigned exact",
        vec![
            literal_tag('e', "東京", &[]),
            literal_tag('P', "CaseSensitive", &[]),
        ],
    );
    let unsigned_id = unsigned.id.expect("builder computes id");
    let opposite_key = signed_event_with_tags(
        &keys,
        42,
        "opposite key",
        vec![
            literal_tag('E', "café", &[]),
            literal_tag('P', "CaseSensitive", &[]),
        ],
    );
    let missing_conjunct =
        signed_event_with_tags(&keys, 43, "missing P", vec![literal_tag('e', "café", &[])]);
    let wrong_value_case = unsigned_event_with_tags(
        &keys,
        44,
        "wrong value case",
        vec![
            literal_tag('e', "CAFÉ", &[]),
            literal_tag('P', "CaseSensitive", &[]),
        ],
    );
    let wrong_value_case_id = wrong_value_case.id.expect("builder computes id");
    let later_cell_only = unsigned_event_with_tags(
        &keys,
        45,
        "later cell decoy",
        vec![
            literal_tag('e', "wrong", &["café"]),
            literal_tag('P', "CaseSensitive", &[]),
        ],
    );
    let later_cell_only_id = later_cell_only.id.expect("builder computes id");
    let relay = occurrence("wss://relay.example", 1);
    cache
        .commit(vec![
            EventStateMutation::Upsert(relay_event(signed.clone(), relay.clone())),
            EventStateMutation::Upsert(relay_event(opposite_key.clone(), relay.clone())),
            EventStateMutation::Upsert(relay_event(missing_conjunct.clone(), relay)),
        ])
        .expect("signed cache corpus commits");
    let accepted = writes
        .accept_materialized(EventValue::Unsigned(unsigned))
        .expect("exact unsigned event accepts");
    writes
        .accept_materialized(EventValue::Unsigned(wrong_value_case))
        .expect("wrong-case decoy accepts");
    writes
        .accept_materialized(EventValue::Unsigned(later_cell_only))
        .expect("later-cell decoy accepts");
    let all_ids = [
        signed.id,
        unsigned_id,
        opposite_key.id,
        missing_conjunct.id,
        wrong_value_case_id,
        later_cell_only_id,
    ];
    let e = SingleLetterTag::from_char('e').expect("lowercase tag key");
    let upper_p = SingleLetterTag::from_char('P').expect("uppercase tag key");
    let query = Query::events()
        .ids(all_ids)
        .expect("six ids are bounded")
        .authors([keys.public_key()])
        .expect("one author is bounded")
        .kinds([Kind::TextNote])
        .expect("one kind is bounded")
        .tag_values(e, ["café", "東京"])
        .expect("two tag values are bounded")
        .tag_values(upper_p, ["CaseSensitive"])
        .expect("one tag value is bounded")
        .cache_only();

    let feed = fava.observe(query).await.expect("query opens");
    let snapshot = feed.current();

    assert_eq!(snapshot.events.len(), 2);
    let cached_record = snapshot
        .events
        .iter()
        .find(|record| record.id() == signed.id)
        .expect("signed cache match remains visible");
    assert_eq!(cached_record.relay_occurrences().len(), 1);
    assert!(cached_record.publication().is_none());
    let local_record = snapshot
        .events
        .iter()
        .find(|record| record.id() == unsigned_id)
        .expect("unsigned write-store match remains visible");
    assert!(local_record.relay_occurrences().is_empty());
    assert_eq!(
        local_record
            .publication()
            .map(|publication| publication.receipt_id),
        Some(accepted.receipt_id)
    );
    assert_eq!(snapshot.evidence.sources.len(), 2);
    assert!(
        cache
            .event(unsigned_id)
            .expect("cache remains readable")
            .is_none()
    );

    let empty = fava
        .observe(
            Query::events()
                .tag_values(e, std::iter::empty::<String>())
                .expect("an empty tag value collection is bounded")
                .cache_only(),
        )
        .await
        .expect("present-empty query opens");
    assert!(empty.current().events.is_empty());
}

/// An application watching a query has to be able to tell an event that was
/// deleted from one the provider merely stopped retaining: only one of the two
/// may be re-acquired from a relay.
#[tokio::test(flavor = "current_thread")]
async fn a_removed_event_reaches_the_application_with_the_rule_that_removed_it() {
    let (fava, cache, _writes) = assembly();
    let keys = Keys::generate();
    let doomed = signed_event(&keys, Kind::TextNote, 10, "doomed");
    cache
        .admit(
            relay_event(doomed.clone(), occurrence("wss://relay.example", 11)),
            Timestamp::from(11),
        )
        .expect("event admits");

    let mut feed = fava
        .observe(Query::events().cache_only())
        .await
        .expect("query opens from local sources");
    assert_eq!(feed.current().events.len(), 1);

    let deletion = EventBuilder::new(Kind::EventDeletion, "")
        .tag(Tag::event(doomed.id))
        .custom_created_at(Timestamp::from(12))
        .finalize(&keys)
        .expect("test event signs");
    cache
        .admit(
            relay_event(deletion.clone(), occurrence("wss://relay.example", 12)),
            Timestamp::from(12),
        )
        .expect("deletion admits");

    let after = next_snapshot(&mut feed).await;
    assert!(
        !after.events.iter().any(|record| record.id() == doomed.id),
        "the deleted event is gone from the result"
    );
    let source = after
        .evidence
        .source(&fava_query::SourceKind::EventCache)
        .expect("the event cache contributed to this result");
    assert_eq!(
        source.retraction(&doomed.id),
        Some(&fava_state::RetractionCause::Deleted {
            deletion: deletion.id
        }),
        "the removal must reach the application as a NIP-09 deletion: {:?}",
        source.retractions
    );
}
