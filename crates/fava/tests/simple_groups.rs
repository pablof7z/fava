//! Public-facade evidence for the simple-group value, ordinary observations, and writes.

use std::sync::Arc;
use std::time::Duration;

use fava::{
    EventBuilder, EventValue, Fava, Kind, Query, SingleLetterTag, Tag, Timestamp, WriteRouting, all,
};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_signer_local::LocalSigner;
use fava_simple_groups::{
    SavedGroupList, SimpleGroup, SimpleGroupEventBuilder, SimpleGroupMetadata,
    SimpleGroupStateEventKind, save_simple_group, saved_group_list_materializer,
};
use fava_state::{EventStateMutation, RelayEvent};
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{Event, EventBuilder as NostrEventBuilder, FinalizeEvent};
use nostr::key::Keys;
use nostr::types::RelayUrl;

#[allow(dead_code)]
#[path = "support/semantic_write.rs"]
mod support;

use support::{RecordingPublisher, publication_builder};

#[test]
fn group_content_composition_stays_exact_through_the_public_facade() {
    let group = group();
    let h = SingleLetterTag::from_char('h').expect("lowercase h");
    let query = Query::events()
        .tag_values(h, ["another-group", "group-29"])
        .and_then(|query| group.events(query))
        .expect("group query composition");
    assert_eq!(
        query.selection().tag_values.get(&h),
        Some(&std::collections::BTreeSet::from(["group-29".to_owned()])),
    );

    let disjoint = Query::events()
        .tag_values(h, ["another-group"])
        .and_then(|query| group.events(query))
        .expect("disjoint group composition is match-nothing");
    assert_eq!(
        disjoint.selection().tag_values.get(&h),
        Some(&std::collections::BTreeSet::new()),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn grouped_builder_uses_the_ordinary_observation_and_write_doors() {
    let keys = Keys::generate();
    let (fava, cache) = assembly(&keys);
    let group = group();
    let query = group
        .events(Query::events().cache_only())
        .expect("group query");
    let mut observation = fava.observe(query).await.expect("query opens");

    let builder = EventBuilder::new(keys.public_key(), Kind::from_u16(9_007))
        .created_at(Timestamp::from(10))
        .content("local group content")
        .simple_group(&group)
        .expect("group composes");
    let write = fava.publish(builder).expect("ordinary custody accepts");
    let receipt = write.receipt().expect("receipt");
    let id = receipt.current.id();
    assert_eq!(
        receipt.routing,
        WriteRouting::Explicit(group.relays().collect())
    );

    let current = wait_for(&mut observation, |snapshot| {
        snapshot.events.iter().any(|record| record.id() == id)
    })
    .await;
    assert_eq!(current.events.len(), 1);
    assert!(current.events[0].publication().is_some());
    assert!(current.events[0].relay_occurrences().is_empty());
    assert!(cache.event(id).expect("cache readable").is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn state_query_returns_generic_records_for_event_local_decoding() {
    let keys = Keys::generate();
    let (fava, cache) = assembly(&keys);
    let group = group();
    let query = group
        .meta_events([SimpleGroupStateEventKind::Metadata])
        .expect("state query")
        .cache_only();
    let mut observation = fava.observe(query).await.expect("query opens");
    let event = NostrEventBuilder::new(Kind::from_u16(39_000), "")
        .tags([
            tag(&["d", "group-29"]),
            tag(&["name", "Facade group"]),
            tag(&["private"]),
        ])
        .custom_created_at(Timestamp::from(20))
        .finalize(&keys)
        .expect("metadata signs");
    cache
        .commit(vec![EventStateMutation::Upsert(observed(
            event,
            group.relays().next().expect("relay"),
            21,
        ))])
        .expect("cache commit");

    let current = wait_for(&mut observation, |snapshot| !snapshot.events.is_empty()).await;
    let metadata = SimpleGroupMetadata::from_event(current.events[0].event())
        .expect("ordinary event value decodes");
    assert_eq!(metadata.id(), "group-29");
    assert_eq!(metadata.name(), Some("Facade group"));
    assert!(metadata.is_private());
}

#[tokio::test(flavor = "current_thread")]
async fn saved_group_edit_materializes_through_the_ordinary_semantic_write_lifecycle() {
    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(MemoryWriteStore::default());
    let publisher = Arc::new(RecordingPublisher::default());
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::new(LocalSigner::new(keys.clone())),
        publisher,
    )
    .materializers([saved_group_list_materializer()])
    .build()
    .expect("facade assembly");
    let group = group();
    let edit = save_simple_group(&group, Some("Photos")).expect("bounded saved-group edit");
    let write = fava
        .by(keys.public_key())
        .to(group.relays())
        .expect("explicit route")
        .publish(edit)
        .expect("semantic custody accepts");
    let receipt = write.settled(all()).await.expect("write settles");

    assert!(matches!(receipt.current.event, EventValue::Signed(_)));
    let list =
        SavedGroupList::from_event(&receipt.current.event).expect("materialized list decodes");
    assert_eq!(list.author(), keys.public_key());
    assert_eq!(list.simple_groups().len(), group.relays().count());
    for (entry, relay) in list.simple_groups().iter().zip(group.relays()) {
        let saved = entry.as_ref().expect("saved group entry");
        assert_eq!(saved.id(), "group-29");
        assert_eq!(saved.display_name(), Some("Photos"));
        assert_eq!(saved.relay(), relay.as_str());
    }
}

fn assembly(keys: &Keys) -> (Fava, Arc<MemoryEventCache>) {
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(MemoryWriteStore::default());
    let publisher = Arc::new(RecordingPublisher::default());
    let fava = publication_builder(
        Arc::clone(&cache),
        store,
        Arc::new(LocalSigner::new(keys.clone())),
        publisher,
    )
    .build()
    .expect("facade assembly");
    (fava, cache)
}

fn group() -> SimpleGroup {
    SimpleGroup::new(
        "group-29",
        vec![relay("a"), relay("b"), relay("contacted-but-not-serving")],
    )
    .expect("non-empty group")
}

fn relay(name: &str) -> RelayUrl {
    RelayUrl::parse(&format!("wss://{name}.example")).expect("relay URL")
}

fn tag(values: &[&str]) -> Tag {
    Tag::parse(values.iter().copied()).expect("tag")
}

fn observed(event: Event, relay: RelayUrl, observed_at: u64) -> RelayEvent {
    RelayEvent::new(
        event,
        RelaySessionKey {
            relay,
            access: RelayAccess::Public,
        },
        Timestamp::from(observed_at),
    )
}

async fn wait_for(
    observation: &mut fava::Observation,
    predicate: impl Fn(&fava::QuerySnapshot) -> bool,
) -> Arc<fava::QuerySnapshot> {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let current = observation.current();
            if predicate(&current) {
                return current;
            }
            observation
                .changed()
                .await
                .expect("observation remains open");
        }
    })
    .await
    .expect("snapshot deadline")
}
