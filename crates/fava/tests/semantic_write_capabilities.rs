//! Shared public-facade evidence for independently selected semantic capabilities.

use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::sync::{Arc, Barrier, Mutex};

use fava::{
    Event, EventBuilder, EventCoordinate, EventValue, Fava, Kind, MaterializationId, PublicKey,
    ReplaceableEventEdit, ReplaceableEventMaterializer, Timestamp, WriteIntent, WriteIntentError,
    WriteRouting,
};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_state::CachedEvent;
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{EventBuilder as NostrEventBuilder, EventId, FinalizeEvent, Tag};
use nostr::key::Keys;

#[allow(dead_code)]
#[path = "support/semantic_write.rs"]
mod support;

use support::{
    BlockingSigner, CountingRouter, CountingSigner, NoopTransport, RecordingPublisher,
    assert_no_receipt_change, publication_builder, relay_evidence, relay_url,
    wait_for_materialization, wait_for_signer,
};

type EditResult = Result<ReplaceableEventEdit, WriteIntentError>;

fn signed(keys: &Keys, kind: Kind, created_at: u64, content: &str, tags: Vec<Tag>) -> Event {
    NostrEventBuilder::new(kind, content)
        .tags(tags)
        .custom_created_at(Timestamp::from(created_at))
        .finalize(keys)
        .expect("corpus source signs")
}

fn explicit_intent(edit: ReplaceableEventEdit) -> WriteIntent {
    WriteIntent::edit(edit, WriteRouting::Explicit(BTreeSet::from([relay_url()])))
        .expect("corpus edit validates")
}

fn automatic_intent(edit: ReplaceableEventEdit) -> WriteIntent {
    WriteIntent::edit(edit, WriteRouting::Automatic).expect("corpus edit validates")
}

fn target_count(event: &Event, tag_name: &str, target: &str) -> usize {
    event
        .tags
        .iter()
        .filter(|tag| {
            let values = tag.as_slice();
            values.first().map(String::as_str) == Some(tag_name)
                && values.get(1).map(String::as_str) == Some(target)
        })
        .count()
}

async fn public_first_value_and_inverse<Add, Remove, Adjacent>(
    kind: Kind,
    materializer: Arc<dyn ReplaceableEventMaterializer>,
    add: Add,
    remove: Remove,
    adjacent: Adjacent,
    tags: (&str, &str, &str),
) where
    Add: Fn(PublicKey) -> EditResult,
    Remove: Fn(PublicKey) -> EditResult,
    Adjacent: Fn(PublicKey) -> EditResult,
{
    let (tag_name, target, adjacent_target) = tags;
    let keys = Keys::generate();
    let actor = keys.public_key();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(MemoryWriteStore::default());
    let signer = Arc::new(CountingSigner::new(keys.clone()));
    let publisher = Arc::new(RecordingPublisher::default());
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::clone(&publisher),
    )
    .materializers([Arc::clone(&materializer)])
    .build()
    .expect("public capability assembly");
    let mut observation = fava
        .observe(
            fava::Query::events()
                .authors([actor])
                .kind(kind)
                .cache_only(),
        )
        .await
        .expect("public semantic query opens");

    let accepted = fava
        .publish(explicit_intent(add(actor).expect("add edit")))
        .expect("first semantic value accepts");
    let visible = tokio::time::timeout(std::time::Duration::from_secs(1), observation.changed())
        .await
        .expect("first value becomes visible")
        .expect("observation stays open");
    let receipt = fava
        .wait_terminal(accepted.receipt_id)
        .await
        .expect("first value settles");
    let attempt = publisher.attempts().pop().expect("one publication attempt");

    assert_eq!(accepted.write_id, receipt.write_id);
    assert_eq!(accepted.receipt_id, receipt.receipt_id);
    assert_eq!(attempt.receipt_id, accepted.receipt_id);
    assert_eq!(attempt.materialization_id, MaterializationId::from_u64(1));
    assert_eq!(attempt.event.id, receipt.current.event.id().unwrap());
    assert_eq!(visible.events.len(), 1);
    assert_eq!(target_count(&attempt.event, tag_name, target), 1);
    assert_eq!(signer.calls(), 1);
    assert!(
        cache
            .is_empty()
            .expect("unpublished event never enters cache")
    );
    assert_eq!(store.len().expect("write store remains readable"), 1);

    let mut source_tags = attempt.event.tags.to_vec();
    source_tags.push(Tag::parse(["x", "unrelated", "bytes"]).unwrap());
    let source = signed(&keys, kind, 20, "opaque", source_tags);
    let duplicate = materializer
        .materialize(&add(actor).unwrap(), Some(&source), Timestamp::from(21))
        .expect("duplicate add materializes");
    let duplicate_again = materializer
        .materialize(&add(actor).unwrap(), Some(&source), Timestamp::from(21))
        .expect("equal input materializes");
    assert_eq!(duplicate, duplicate_again);
    let duplicate = duplicate.finalize(&keys).expect("duplicate output signs");
    assert_eq!(duplicate.content, "opaque");
    assert_eq!(target_count(&duplicate, tag_name, target), 1);
    assert_eq!(
        duplicate.tags.last().unwrap().as_slice(),
        &["x", "unrelated", "bytes"]
    );

    let adjacent = materializer
        .materialize(
            &adjacent(actor).unwrap(),
            Some(&duplicate),
            Timestamp::from(22),
        )
        .expect("adjacent add materializes")
        .finalize(&keys)
        .expect("adjacent output signs");
    assert_eq!(target_count(&adjacent, tag_name, target), 1);
    assert_eq!(target_count(&adjacent, tag_name, adjacent_target), 1);

    let removed = materializer
        .materialize(
            &remove(actor).unwrap(),
            Some(&adjacent),
            Timestamp::from(23),
        )
        .expect("inverse materializes")
        .finalize(&keys)
        .expect("inverse output signs");
    assert_eq!(target_count(&removed, tag_name, target), 0);
    assert_eq!(target_count(&removed, tag_name, adjacent_target), 1);
    assert_eq!(removed.content, "opaque");
    let empty = materializer
        .materialize(&remove(actor).unwrap(), None, Timestamp::from(1))
        .expect("empty inverse is valid");
    assert!(empty.tags.is_empty());
}

async fn shared_preview_bounds_and_failure<Add>(
    kind: Kind,
    materializer: Arc<dyn ReplaceableEventMaterializer>,
    add: Add,
) where
    Add: Fn(PublicKey) -> EditResult,
{
    let keys = Keys::generate();
    let actor = keys.public_key();
    let edit = add(actor).expect("add edit");
    let malformed = ReplaceableEventEdit::new(
        actor,
        edit.coordinate().clone(),
        edit.format(),
        Vec::new(),
        edit.inverse_change().to_vec(),
    )
    .expect("bounded malformed edit");
    assert!(!materializer.supports(&malformed));
    assert!(WriteIntent::edit(edit.clone(), WriteRouting::Explicit(BTreeSet::new())).is_err());
    let addressable = ReplaceableEventEdit::new(
        actor,
        EventCoordinate::Replaceable {
            author: actor,
            kind: Kind::Custom(30_001),
            identifier: Some("addressable".to_owned()),
        },
        edit.format(),
        edit.change().to_vec(),
        edit.inverse_change().to_vec(),
    )
    .unwrap();
    assert!(WriteIntent::edit(addressable, WriteRouting::Automatic).is_err());

    let mut hostile = signed(
        &keys,
        kind,
        1,
        "hostile",
        vec![Tag::parse(["x", "valid"]).unwrap()],
    );
    hostile.tags = (0..2_001)
        .map(|index| Tag::parse(["x", &index.to_string()]).unwrap())
        .collect();
    assert!(
        materializer
            .materialize(&edit, Some(&hostile), Timestamp::from(2))
            .is_err()
    );

    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(MemoryWriteStore::default());
    let signer = Arc::new(BlockingSigner::new(actor));
    let publisher = Arc::new(RecordingPublisher::default());
    let router = Arc::new(CountingRouter::new(relay_url()));
    let fava = publication_builder(cache, Arc::clone(&store), Arc::clone(&signer), publisher)
        .router(Arc::clone(&router))
        .materializers([Arc::clone(&materializer)])
        .build()
        .expect("preview assembly");
    let intent = automatic_intent(edit.clone());
    let preview = fava
        .preview_write_routes(&intent)
        .expect("preview succeeds");
    assert_eq!(store.len().unwrap(), 0);
    assert_eq!(router.previews(), 1);
    assert_eq!(router.opens(), 0);
    let accepted = fava.publish(intent).expect("live write accepts");
    let receipt = wait_for_materialization(&fava, accepted.receipt_id, 1).await;
    assert_eq!(
        receipt.desired_destinations,
        preview.destinations.keys().cloned().collect()
    );
    assert_selection_and_capacity_refusals(keys, actor, edit, materializer);
}

fn assert_selection_and_capacity_refusals(
    keys: Keys,
    actor: PublicKey,
    edit: ReplaceableEventEdit,
    materializer: Arc<dyn ReplaceableEventMaterializer>,
) {
    let empty = publication_builder(
        Arc::new(MemoryEventCache::default()),
        Arc::new(MemoryWriteStore::default()),
        Arc::new(CountingSigner::new(keys.clone())),
        Arc::new(RecordingPublisher::default()),
    )
    .build()
    .expect("publication without materializers is valid");
    assert!(empty.publish(explicit_intent(edit.clone())).is_err());
    let duplicate = publication_builder(
        Arc::new(MemoryEventCache::default()),
        Arc::new(MemoryWriteStore::default()),
        Arc::new(CountingSigner::new(keys.clone())),
        Arc::new(RecordingPublisher::default()),
    )
    .materializers([Arc::clone(&materializer), Arc::clone(&materializer)])
    .build();
    assert!(duplicate.is_err());

    let bounded_store = Arc::new(MemoryWriteStore::bounded(NonZeroUsize::new(1).unwrap()));
    let bounded = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::clone(&bounded_store))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .signer(Arc::new(CountingSigner::new(keys)))
        .publisher(Arc::new(RecordingPublisher::default()))
        .delivery_policy(Arc::new(
            fava_delivery_standard::StandardDeliveryPolicy::default(),
        ))
        .materializers([materializer])
        .build()
        .expect("bounded assembly");
    bounded
        .accept_event(EventValue::Unsigned(
            EventBuilder::new(actor, Kind::TextNote).build().unwrap(),
        ))
        .expect("one active write fills capacity");
    assert!(bounded.publish(explicit_intent(edit)).is_err());
}

async fn concurrent_duplicate_source_is_one_successor<Add, Adjacent>(
    kind: Kind,
    materializer: Arc<dyn ReplaceableEventMaterializer>,
    add: Add,
    adjacent: Adjacent,
) where
    Add: Fn(PublicKey) -> EditResult,
    Adjacent: Fn(PublicKey) -> EditResult,
{
    let keys = Keys::generate();
    let actor = keys.public_key();
    let initial = materializer
        .materialize(&add(actor).unwrap(), None, Timestamp::from(u64::MAX - 3))
        .unwrap()
        .finalize(&keys)
        .unwrap();
    let cache = Arc::new(MemoryEventCache::default());
    cache
        .admit(
            CachedEvent::new(initial.clone(), relay_evidence()),
            Timestamp::from(1),
        )
        .expect("initial source admits");
    let store = Arc::new(MemoryWriteStore::default());
    let signer = Arc::new(BlockingSigner::new(actor));
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::new(RecordingPublisher::default()),
    )
    .materializers([Arc::clone(&materializer)])
    .build()
    .expect("concurrency assembly");
    let accepted = fava
        .publish(explicit_intent(add(actor).unwrap()))
        .expect("semantic write accepts");
    wait_for_signer(&signer, 1).await;
    let successor = materializer
        .materialize(
            &adjacent(actor).unwrap(),
            Some(&initial),
            Timestamp::from(u64::MAX - 1),
        )
        .unwrap()
        .finalize(&keys)
        .unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let admissions = Arc::new(Mutex::new(0usize));
    std::thread::scope(|scope| {
        for _ in 0..2 {
            let cache = Arc::clone(&cache);
            let successor = successor.clone();
            let barrier = Arc::clone(&barrier);
            let admissions = Arc::clone(&admissions);
            scope.spawn(move || {
                barrier.wait();
                cache
                    .admit(
                        CachedEvent::new(successor, relay_evidence()),
                        Timestamp::from(2),
                    )
                    .expect("concurrent source admission remains readable");
                *admissions.lock().unwrap() += 1;
            });
        }
        barrier.wait();
    });
    assert_eq!(*admissions.lock().unwrap(), 2);
    let receipt = wait_for_materialization(&fava, accepted.receipt_id, 2).await;
    wait_for_signer(&signer, 2).await;
    assert_eq!(receipt.write_id, accepted.write_id);
    assert_eq!(receipt.receipt_id, accepted.receipt_id);
    assert_eq!(
        receipt.current.publication.materialization_source,
        Some(successor.id)
    );
    assert_eq!(receipt.current.event.kind(), kind);
    assert_no_receipt_change(&store).await;
}

#[tokio::test(flavor = "current_thread")]
async fn nip02_passes_public_semantic_write_corpus() {
    let target = Keys::generate().public_key();
    let adjacent = Keys::generate().public_key();
    let target_hex = target.to_hex();
    let adjacent_hex = adjacent.to_hex();
    public_first_value_and_inverse(
        Kind::ContactList,
        fava_nip02::materializer(),
        |actor| fava_nip02::follow(actor, target),
        |actor| fava_nip02::unfollow(actor, target),
        |actor| fava_nip02::follow(actor, adjacent),
        ("p", &target_hex, &adjacent_hex),
    )
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn bookmarks_pass_public_semantic_write_corpus() {
    let target = EventId::from_byte_array([8; 32]);
    let adjacent = EventId::from_byte_array([9; 32]);
    let target_hex = target.to_hex();
    let adjacent_hex = adjacent.to_hex();
    public_first_value_and_inverse(
        Kind::Custom(10_003),
        fava_bookmarks::materializer(),
        |actor| fava_bookmarks::bookmark_event(actor, target),
        |actor| fava_bookmarks::unbookmark_event(actor, target),
        |actor| fava_bookmarks::bookmark_event(actor, adjacent),
        ("e", &target_hex, &adjacent_hex),
    )
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn capabilities_share_preview_bounds_and_failure_behavior() {
    let follow_target = Keys::generate().public_key();
    shared_preview_bounds_and_failure(Kind::ContactList, fava_nip02::materializer(), |actor| {
        fava_nip02::follow(actor, follow_target)
    })
    .await;
    let bookmark_target = EventId::from_byte_array([10; 32]);
    shared_preview_bounds_and_failure(
        Kind::Custom(10_003),
        fava_bookmarks::materializer(),
        |actor| fava_bookmarks::bookmark_event(actor, bookmark_target),
    )
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn capabilities_share_concurrency_and_retired_completion_behavior() {
    let follow_target = Keys::generate().public_key();
    let follow_adjacent = Keys::generate().public_key();
    concurrent_duplicate_source_is_one_successor(
        Kind::ContactList,
        fava_nip02::materializer(),
        |actor| fava_nip02::follow(actor, follow_target),
        |actor| fava_nip02::follow(actor, follow_adjacent),
    )
    .await;
    let bookmark_target = EventId::from_byte_array([11; 32]);
    let bookmark_adjacent = EventId::from_byte_array([12; 32]);
    concurrent_duplicate_source_is_one_successor(
        Kind::Custom(10_003),
        fava_bookmarks::materializer(),
        |actor| fava_bookmarks::bookmark_event(actor, bookmark_target),
        |actor| fava_bookmarks::bookmark_event(actor, bookmark_adjacent),
    )
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn capabilities_share_public_source_removal_and_processed_stale_success() {
    panic!("RED: public source-removal and processed stale-success proof not implemented");
}
