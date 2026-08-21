//! Public-facade acceptance evidence for the first local-source vertical slice.

use std::sync::Arc;
use std::time::Duration;

use fava::{Fava, Query, SingleLetterTag};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_state::{
    CacheMutation, CachedEvent, RelayAccess, RelayEvidence, RelaySessionKey, RelayUrl, Timestamp,
};
use fava_write::EventValue;
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{
    Event, EventBuilder, FinalizeEvent, FinalizeUnsignedEvent, Kind, Tag, UnsignedEvent,
};
use nostr::key::Keys;
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

fn evidence(relay: &str, observed_at: u64) -> RelayEvidence {
    RelayEvidence::one(
        RelaySessionKey::new(
            RelayUrl::parse(relay).expect("test relay url"),
            RelayAccess::public(),
        ),
        Timestamp::from(observed_at),
    )
}

async fn next_snapshot(feed: &mut fava_observe::Observation) -> Arc<fava::QuerySnapshot> {
    timeout(Duration::from_secs(1), feed.changed())
        .await
        .expect("observation update arrives within bound")
        .expect("observation remains open")
}

#[tokio::test(flavor = "current_thread")]
async fn accepted_local_event_is_visible_without_cache_pollution() {
    let (fava, cache, _writes) = assembly();
    let keys = Keys::generate();
    let unsigned = unsigned_event(&keys, Kind::TextNote, 10, "local");
    let id = unsigned.id.expect("builder computes id");
    let mut feed = fava
        .observe(Query::events().cache_only())
        .await
        .expect("query opens from local sources");
    assert!(feed.current().events.is_empty());

    let accepted = fava
        .accept_event(EventValue::Unsigned(unsigned))
        .expect("facade accepts finalized local event");
    let visible = next_snapshot(&mut feed).await;

    assert_eq!(visible.events.len(), 1);
    assert_eq!(visible.events[0].id(), id);
    assert_eq!(
        visible.events[0]
            .publication
            .as_ref()
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
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            predecessor.clone(),
            evidence("wss://relay.example", 1),
        ))])
        .expect("cache mutation commits");
    let successor = unsigned_event(&keys, Kind::ContactList, 20, "local");
    let successor_id = successor.id.expect("builder computes id");
    let accepted = writes
        .accept_materialized(EventValue::Unsigned(successor))
        .expect("local successor accepts");
    let query = Query::events().kind(Kind::ContactList).cache_only();
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
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            event.clone(),
            evidence("wss://relay-a.example", 1),
        ))])
        .expect("first relay evidence commits");
    let mut feed = fava
        .observe(Query::events().cache_only())
        .await
        .expect("query opens");
    assert_eq!(feed.current().events.len(), 1);
    assert_eq!(feed.current().events[0].relay_evidence.len(), 1);

    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            event,
            evidence("wss://relay-b.example", 2),
        ))])
        .expect("second relay evidence merges");
    let enriched = next_snapshot(&mut feed).await;

    assert_eq!(enriched.events.len(), 1);
    assert_eq!(enriched.events[0].relay_evidence.len(), 2);
    assert_eq!(
        enriched.events[0]
            .publication
            .as_ref()
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
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            event.clone(),
            evidence("wss://other.example", 1),
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
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            event,
            evidence("wss://asked.example", 2),
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
                    CachedEvent::new(event, evidence("wss://relay.example", 15)),
                    Timestamp::from(15),
                )
                .expect("event admission commits")
        );
    }
    let mut feed = fava
        .observe(Query::events().kind(Kind::TextNote).cache_only())
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
            CachedEvent::new(deletion, evidence("wss://relay.example", 20)),
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
async fn literal_tag_selection_preserves_exact_sources_through_public_observation() {
    let (fava, cache, _writes) = assembly();
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
    let relay = evidence("wss://relay.example", 1);
    cache
        .commit(vec![
            CacheMutation::Upsert(CachedEvent::new(signed.clone(), relay.clone())),
            CacheMutation::Upsert(CachedEvent::new(opposite_key.clone(), relay.clone())),
            CacheMutation::Upsert(CachedEvent::new(missing_conjunct.clone(), relay)),
        ])
        .expect("signed cache corpus commits");
    let accepted = fava
        .accept_event(EventValue::Unsigned(unsigned))
        .expect("exact unsigned event accepts");
    fava.accept_event(EventValue::Unsigned(wrong_value_case))
        .expect("wrong-case decoy accepts");
    fava.accept_event(EventValue::Unsigned(later_cell_only))
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
        .authors([keys.public_key()])
        .kind(Kind::TextNote)
        .tag_values(e, ["café", "東京"])
        .tag_values(upper_p, ["CaseSensitive"])
        .cache_only();

    let feed = fava.observe(query).await.expect("query opens");
    let snapshot = feed.current();

    assert_eq!(snapshot.events.len(), 2);
    let cached_record = snapshot
        .events
        .iter()
        .find(|record| record.id() == signed.id)
        .expect("signed cache match remains visible");
    assert_eq!(cached_record.relay_evidence.len(), 1);
    assert!(cached_record.publication.is_none());
    let local_record = snapshot
        .events
        .iter()
        .find(|record| record.id() == unsigned_id)
        .expect("unsigned write-store match remains visible");
    assert!(local_record.relay_evidence.is_empty());
    assert_eq!(
        local_record
            .publication
            .as_ref()
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
                .cache_only(),
        )
        .await
        .expect("present-empty query opens");
    assert!(empty.current().events.is_empty());
}
