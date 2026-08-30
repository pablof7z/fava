//! Shared public-facade evidence for independently selected semantic capabilities.

use std::num::NonZeroUsize;
use std::sync::Arc;

use fava::{
    BuildError, EditApplier, Event, EventBuilder, EventEdit, EventValue, Fava, Kind, PublicKey,
    PublicationError, PublishError, Timestamp, WriteIntentError, WriteStoreError,
};
use fava_bookmarks::Bookmarks;
use fava_event_cache_memory::MemoryEventCache;
use fava_nip02::Nip02;
use fava_query_standard::StandardQueryEvaluator;
use fava_write::{WriteIntent, WriteRouting};
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{EventBuilder as NostrEventBuilder, EventId, FinalizeEvent, Tag};
use nostr::key::Keys;

#[path = "support/semantic_write_capability_lifecycle.rs"]
mod capability_lifecycle;
#[path = "support/semantic_write_capability_protocol.rs"]
mod capability_protocol;
#[path = "support/semantic_write_capability_signer.rs"]
mod capability_signer;
#[allow(dead_code)]
#[path = "support/semantic_write.rs"]
mod support;

use capability_protocol::Enable;
use support::{
    BlockingSigner, Capture, CountingRouter, CountingSigner, NoopTransport, RecordingPublisher,
    TestApplier, captured_applier, publication_builder, publication_owner, relay_url,
    wait_for_revision,
};

type EditResult = Result<EventEdit, WriteIntentError>;

fn signed(keys: &Keys, kind: Kind, created_at: u64, content: &str, tags: Vec<Tag>) -> Event {
    NostrEventBuilder::new(kind, content)
        .tags(tags)
        .custom_created_at(Timestamp::from(created_at))
        .finalize(keys)
        .expect("corpus source signs")
}

fn explicit_intent(edit: EventEdit, author: PublicKey) -> WriteIntent {
    WriteIntent::edit_as(edit, author, WriteRouting::Explicit(vec![relay_url()]))
        .expect("corpus edit validates")
}

fn automatic_intent(edit: EventEdit, author: PublicKey) -> WriteIntent {
    WriteIntent::edit_as(edit, author, WriteRouting::Automatic).expect("corpus edit validates")
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

async fn shared_preview_bounds_and_failure<Add>(enable: Enable, capture: Capture, add: Add)
where
    Add: Fn() -> EditResult,
{
    let keys = Keys::generate();
    let actor = keys.public_key();
    let edit = add().expect("add edit");
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(MemoryWriteStore::default());
    let signer = Arc::new(BlockingSigner::new(actor));
    let publisher = Arc::new(RecordingPublisher::default());
    let router = Arc::new(CountingRouter::new(relay_url()));
    // `Publication::new` (below `publication_owner`) is fava-publication's
    // own door, not the sink: it takes a concrete `Arc<dyn EditApplier>`
    // directly and has no notion of a protocol's enabling call.
    // `captured_applier(capture)` recovers the real protocol applier through
    // its own enabling call, so this still exercises the shipped applier.
    let owner = publication_owner(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::clone(&publisher),
        vec![captured_applier(capture)],
        vec![router.clone()],
    );
    let fava = enable(
        publication_builder(cache, Arc::clone(&store), Arc::clone(&signer), publisher)
            .router(Arc::clone(&router)),
    )
    .build()
    .expect("preview assembly");
    let intent = automatic_intent(edit.clone(), actor);
    let preview = owner
        .preview_semantic_routes(&intent)
        .expect("publication-provider preview succeeds");
    assert_eq!(store.len().unwrap(), 0);
    assert_eq!(router.previews(), 1);
    assert_eq!(router.opens(), 0);
    let write = fava
        .by(actor)
        .publish(edit.clone())
        .expect("live write accepts");
    let receipt = wait_for_revision(&fava, write.receipt_id(), 1).await;
    assert_eq!(
        receipt.desired_destinations,
        preview.destinations.keys().cloned().collect()
    );
    assert_selection_and_capacity_refusals(keys, actor, edit, enable);
}

fn assert_selection_and_capacity_refusals(
    keys: Keys,
    actor: PublicKey,
    edit: EventEdit,
    enable: Enable,
) {
    let empty = publication_builder(
        Arc::new(MemoryEventCache::default()),
        Arc::new(MemoryWriteStore::default()),
        Arc::new(CountingSigner::new(keys.clone())),
        Arc::new(RecordingPublisher::default()),
    )
    .build()
    .expect("publication without appliers is valid");
    assert!(matches!(
        empty
            .by(actor)
            .to([relay_url()])
            .expect("route validates")
            .publish(edit.clone()),
        Err(PublishError::Publication(PublicationError::Routing(_)))
    ));
    let duplicate = enable(enable(publication_builder(
        Arc::new(MemoryEventCache::default()),
        Arc::new(MemoryWriteStore::default()),
        Arc::new(CountingSigner::new(keys.clone())),
        Arc::new(RecordingPublisher::default()),
    )))
    .build();
    assert!(matches!(duplicate, Err(BuildError::Publication(_))));
    let overflow = publication_builder(
        Arc::new(MemoryEventCache::default()),
        Arc::new(MemoryWriteStore::default()),
        Arc::new(CountingSigner::new(keys.clone())),
        Arc::new(RecordingPublisher::default()),
    )
    .appliers((0..65).map(|offset| {
        Arc::new(TestApplier::new(Kind::Custom(20_000 + offset))) as Arc<dyn EditApplier>
    }))
    .build();
    assert!(matches!(overflow, Err(BuildError::Publication(_))));

    let bounded_store = Arc::new(MemoryWriteStore::bounded(NonZeroUsize::new(1).unwrap()));
    let bounded = enable(
        Fava::builder()
            .event_cache(Arc::new(MemoryEventCache::default()))
            .write_store(Arc::clone(&bounded_store))
            .query_evaluator(Arc::new(StandardQueryEvaluator))
            .transport(Arc::new(NoopTransport))
            .signer(Arc::new(CountingSigner::new(keys)))
            .publisher(Arc::new(RecordingPublisher::default()))
            .delivery_policy(Arc::new(
                fava_delivery_standard::StandardDeliveryPolicy::default(),
            )),
    )
    .build()
    .expect("bounded assembly");
    bounded_store
        .accept_applied(EventValue::Unsigned(
            EventBuilder::new(Kind::TextNote).by(actor).build().unwrap(),
        ))
        .expect("one active write fills capacity");
    assert!(matches!(
        bounded
            .by(actor)
            .to([relay_url()])
            .expect("route validates")
            .publish(edit),
        Err(PublishError::Publication(PublicationError::Store(
            WriteStoreError::Refused(_)
        )))
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn nip02_passes_public_semantic_write_corpus() {
    let target = Keys::generate().public_key();
    let adjacent = Keys::generate().public_key();
    let target_hex = target.to_hex();
    let adjacent_hex = adjacent.to_hex();
    capability_protocol::exercise_public_lifecycle(
        Kind::ContactList,
        Nip02::with_nip02,
        || fava_nip02::follow(target),
        || fava_nip02::unfollow(target),
        || fava_nip02::follow(adjacent),
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
    capability_protocol::exercise_public_lifecycle(
        Kind::Custom(10_003),
        Bookmarks::with_bookmarks,
        || fava_bookmarks::bookmark_event(target),
        || fava_bookmarks::unbookmark_event(target),
        || fava_bookmarks::bookmark_event(adjacent),
        ("e", &target_hex, &adjacent_hex),
    )
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn capabilities_share_preview_bounds_and_failure_behavior() {
    let follow_target = Keys::generate().public_key();
    shared_preview_bounds_and_failure(Nip02::with_nip02, Nip02::with_nip02, || {
        fava_nip02::follow(follow_target)
    })
    .await;
    let bookmark_target = EventId::from_byte_array([10; 32]);
    shared_preview_bounds_and_failure(Bookmarks::with_bookmarks, Bookmarks::with_bookmarks, || {
        fava_bookmarks::bookmark_event(bookmark_target)
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn capabilities_share_concurrency_and_retired_completion_behavior() {
    let follow_target = Keys::generate().public_key();
    let follow_adjacent = Keys::generate().public_key();
    capability_lifecycle::exercise(
        Kind::ContactList,
        Nip02::with_nip02,
        Nip02::with_nip02,
        || fava_nip02::follow(follow_target),
        || fava_nip02::follow(follow_adjacent),
        ("p", &follow_target.to_hex()),
    )
    .await;
    let bookmark_target = EventId::from_byte_array([11; 32]);
    let bookmark_adjacent = EventId::from_byte_array([12; 32]);
    capability_lifecycle::exercise(
        Kind::Custom(10_003),
        Bookmarks::with_bookmarks,
        Bookmarks::with_bookmarks,
        || fava_bookmarks::bookmark_event(bookmark_target),
        || fava_bookmarks::bookmark_event(bookmark_adjacent),
        ("e", &bookmark_target.to_hex()),
    )
    .await;
}
