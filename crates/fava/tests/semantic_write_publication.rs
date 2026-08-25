//! Public-facade evidence for semantic materialization and publication.
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use fava::{
    EventBuilder, EventValue, Kind, MaterializationId, ReceiptOutcome, ReplaceableEventEdit,
    ReplaceableEventMaterializer, Tag, Timestamp, all,
};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_state::{CacheMutation, CachedEvent, RetractionCause};
use fava_write::{WriteIntent, WriteIntentError, WriteRouting};
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
use nostr::key::Keys;

#[path = "semantic_write_publication/author.rs"]
mod author;
#[path = "semantic_write_publication/interleavings.rs"]
mod interleavings;
#[path = "semantic_write_publication/restart.rs"]
mod restart;
#[path = "semantic_write_publication/route_revision.rs"]
mod route_revision;
#[path = "semantic_write_publication/shared_capacity.rs"]
mod shared_capacity;
#[path = "support/semantic_write.rs"]
mod support;
#[path = "semantic_write_publication/winner_order.rs"]
mod winner_order;

use support::*;

fn edit(kind: Kind) -> ReplaceableEventEdit {
    ReplaceableEventEdit::new(kind, None, vec![1]).expect("bounded edit")
}

#[test]
fn support_materializer_preserves_the_event_builder_tag_refusal() {
    let keys = Keys::generate();
    let source = NostrEventBuilder::new(Kind::ContactList, "source")
        .tags(
            (0..2_001)
                .map(|index| Tag::parse(["x", &index.to_string()]).expect("ordinary source tag")),
        )
        .custom_created_at(Timestamp::from(1))
        .finalize(&keys)
        .expect("source signs");
    let materializer = TestMaterializer::new(Kind::ContactList);

    assert_eq!(
        materializer.materialize(
            &edit(Kind::ContactList),
            keys.public_key(),
            Some(&EventValue::Signed(source.clone())),
            Timestamp::from(2),
        ),
        Err(WriteIntentError::TooManyTags {
            actual: 2_001,
            maximum: 2_000,
        })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn first_value_edit_publishes_through_public_fava() {
    let keys = Keys::generate();
    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList));
    let (fava, cache, store, signer, publisher) = assembly(
        Arc::new(MemoryWriteStore::default()),
        keys.clone(),
        vec![materializer.clone()],
    );
    let mut observation = fava
        .observe(
            fava::Query::events()
                .authors([keys.public_key()])
                .kind(Kind::ContactList)
                .cache_only(),
        )
        .await
        .expect("semantic query opens");

    let write = fava
        .by(keys.public_key())
        .to([relay_url()])
        .expect("route validates")
        .publish(edit(Kind::ContactList))
        .expect("first semantic value accepts");
    let visible = tokio::time::timeout(Duration::from_secs(1), observation.changed())
        .await
        .expect("local materialization arrives")
        .expect("observation stays open");
    let receipt = write
        .settled(all())
        .await
        .expect("ordinary receipt settles");

    assert_eq!(write.write_id(), receipt.write_id);
    assert_eq!(write.receipt_id(), receipt.receipt_id);
    assert_eq!(receipt.outcome, ReceiptOutcome::Complete);
    assert_eq!(
        receipt.current.publication.materialization_id,
        MaterializationId::from_u64(1)
    );
    assert_eq!(visible.events.len(), 1);
    assert_eq!(visible.events[0].event.author(), keys.public_key());
    assert_eq!(visible.events[0].event.kind(), Kind::ContactList);
    assert_eq!(materializer.calls().len(), 1);
    assert!(materializer.calls()[0].source.is_none());
    assert_eq!(signer.calls(), 1);
    assert_eq!(publisher.attempts().len(), 1);
    assert_eq!(publisher.attempts()[0].receipt_id, write.receipt_id());
    assert_eq!(
        publisher.attempts()[0].materialization_id,
        receipt.current.publication.materialization_id
    );
    assert!(cache.is_empty().expect("cache remains readable"));
    assert_eq!(store.len().expect("store remains readable"), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn distinct_unsigned_edits_compose_under_one_exact_operation() {
    let keys = Keys::generate();
    let store = Arc::new(MemoryWriteStore::bounded(NonZeroUsize::new(1).unwrap()));
    let signer = Arc::new(BlockingSigner::new(keys.public_key()));
    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList));
    let cache = Arc::new(MemoryEventCache::default());
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::new(RecordingPublisher::default()),
    )
    .materializer(Arc::clone(&materializer))
    .build()
    .expect("semantic publication assembly");

    let first_edit =
        ReplaceableEventEdit::new(Kind::ContactList, None, vec![1]).expect("first bounded edit");
    let second_edit =
        ReplaceableEventEdit::new(Kind::ContactList, None, vec![2]).expect("distinct bounded edit");
    let first = fava
        .by(keys.public_key())
        .to([relay_url()])
        .expect("first route validates")
        .publish(first_edit)
        .expect("first edit accepts");
    wait_for_signer(&signer, 1).await;
    let generation_one = first.receipt().expect("first receipt");

    let second = fava
        .by(keys.public_key())
        .to([relay_url()])
        .expect("second route validates")
        .publish(second_edit)
        .expect("second edit composes at the occupied coordinate");
    let generation_two = wait_for_materialization(&fava, second.receipt_id(), 2).await;
    wait_for_signer(&signer, 2).await;

    assert_eq!(second.write_id(), first.write_id());
    assert_eq!(second.receipt_id(), first.receipt_id());
    assert_eq!(store.len().expect("store readable"), 1);
    let EventValue::Unsigned(composed) = &generation_two.current.event else {
        panic!("blocking signer keeps the composed generation unsigned");
    };
    assert_eq!(composed.content, "edit|edit");
    assert_eq!(
        generation_two
            .current
            .publication
            .retired_materializations
            .len(),
        1
    );
    assert_eq!(
        generation_two.current.publication.retired_materializations[0],
        (
            MaterializationId::from_u64(1),
            generation_one.current.id(),
            None,
            None,
        )
    );
    assert!(
        store
            .record_signer_refusal(
                first.write_id(),
                first.receipt_id(),
                MaterializationId::from_u64(1),
                generation_one.current.id(),
                "late generation-one refusal".to_owned(),
            )
            .is_err(),
        "retired signer completion mutated the composed generation"
    );
    assert_eq!(
        first
            .receipt()
            .expect("current receipt after stale completion"),
        generation_two
    );

    let newer = signed_source(&keys, Kind::ContactList, u64::MAX - 1, "newer", &[]);
    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            newer.clone(),
            relay_evidence(),
        ))])
        .expect("newer source enters the canonical cache");
    let generation_three = wait_for_materialization(&fava, first.receipt_id(), 3).await;
    wait_for_signer(&signer, 3).await;
    let EventValue::Unsigned(replayed) = &generation_three.current.event else {
        panic!("blocking signer keeps replayed composition unsigned");
    };
    assert_eq!(replayed.content, "newer|edit|edit");
    assert_eq!(
        generation_three.current.publication.materialization_source,
        Some(newer.id)
    );
    assert_eq!(generation_three.write_id, first.write_id());
    assert_eq!(generation_three.receipt_id, first.receipt_id());
}

#[tokio::test(flavor = "current_thread")]
async fn materializer_selection_bounds_refuse_before_custody() {
    let keys = Keys::generate();

    let (empty, _, empty_store, empty_signer, empty_publisher) = assembly(
        Arc::new(MemoryWriteStore::default()),
        keys.clone(),
        Vec::new(),
    );
    assert!(
        empty
            .by(keys.public_key())
            .to([relay_url()])
            .expect("route validates")
            .publish(edit(Kind::ContactList))
            .is_err()
    );
    assert_no_effects(&empty_store, &empty_signer, &empty_publisher, 0);

    let selected = Arc::new(TestMaterializer::new(Kind::ContactList));
    let (unsupported, _, unsupported_store, unsupported_signer, unsupported_publisher) = assembly(
        Arc::new(MemoryWriteStore::default()),
        keys.clone(),
        vec![selected],
    );
    assert!(
        unsupported
            .by(keys.public_key())
            .to([relay_url()])
            .expect("route validates")
            .publish(edit(Kind::Custom(10_003)))
            .is_err()
    );
    assert_no_effects(
        &unsupported_store,
        &unsupported_signer,
        &unsupported_publisher,
        0,
    );

    let duplicate = publication_builder(
        Arc::new(MemoryEventCache::default()),
        Arc::new(MemoryWriteStore::default()),
        Arc::new(CountingSigner::new(keys.clone())),
        Arc::new(RecordingPublisher::default()),
    )
    .materializers([
        Arc::new(TestMaterializer::new(Kind::ContactList)) as Arc<dyn ReplaceableEventMaterializer>,
        Arc::new(TestMaterializer::new(Kind::ContactList)),
    ])
    .build();
    assert!(duplicate.is_err());

    let overflow = publication_builder(
        Arc::new(MemoryEventCache::default()),
        Arc::new(MemoryWriteStore::default()),
        Arc::new(CountingSigner::new(keys.clone())),
        Arc::new(RecordingPublisher::default()),
    )
    .materializers((0..65).map(|offset| {
        Arc::new(TestMaterializer::new(Kind::Custom(10_000 + offset)))
            as Arc<dyn ReplaceableEventMaterializer>
    }))
    .build();
    assert!(overflow.is_err());

    let bounded_store = Arc::new(MemoryWriteStore::bounded(NonZeroUsize::new(1).unwrap()));
    let bounded_materializer = Arc::new(TestMaterializer::new(Kind::ContactList));
    let (bounded, _, bounded_store, bounded_signer, bounded_publisher) = assembly(
        bounded_store,
        keys.clone(),
        vec![Arc::clone(&bounded_materializer)],
    );
    bounded_store
        .accept_materialized(EventValue::Unsigned(
            EventBuilder::new(keys.public_key(), Kind::TextNote)
                .created_at(Timestamp::from(1))
                .build()
                .unwrap(),
        ))
        .expect("one existing active write occupies capacity");
    assert!(
        bounded
            .by(keys.public_key())
            .to([relay_url()])
            .expect("route validates")
            .publish(edit(Kind::ContactList))
            .is_err()
    );
    assert_no_effects(&bounded_store, &bounded_signer, &bounded_publisher, 1);
    assert_eq!(
        bounded_materializer.calls().len(),
        0,
        "capacity refusal precedes arbitrary materializer code"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn first_value_receives_exact_injected_timestamp() {
    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let source = signed_source(
        &keys,
        Kind::ContactList,
        u64::MAX - 1,
        "remote base",
        &["remote"],
    );
    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            source.clone(),
            relay_evidence(),
        ))])
        .expect("source enters canonical cache");
    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList));
    let (fava, _, _, _, publisher) = assembly_with_cache(
        cache,
        Arc::new(MemoryWriteStore::default()),
        keys,
        vec![materializer.clone()],
    );

    let write = fava
        .by(source.pubkey)
        .to([relay_url()])
        .expect("route validates")
        .publish(edit(Kind::ContactList))
        .expect("source-backed edit accepts");
    let receipt = write.settled(all()).await.expect("publication settles");
    let calls = materializer.calls();

    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].source.as_ref().and_then(EventValue::id),
        Some(source.id)
    );
    assert_eq!(calls[0].created_at, Timestamp::max());
    assert_eq!(receipt.current.event.created_at(), Timestamp::max());
    assert_eq!(publisher.attempts()[0].event.created_at, Timestamp::max());
}

#[tokio::test(flavor = "current_thread")]
async fn newer_source_rematerializes_once_and_preserves_unrelated_fields() {
    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(MemoryWriteStore::default());
    let first = signed_source(
        &keys,
        Kind::ContactList,
        u64::MAX - 3,
        "first source",
        &["first"],
    );
    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            first.clone(),
            relay_evidence(),
        ))])
        .expect("first source enters cache");
    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList));
    let signer = Arc::new(BlockingSigner::new(keys.public_key()));
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::new(RecordingPublisher::default()),
    )
    .materializer(Arc::clone(&materializer))
    .build()
    .expect("semantic publication assembly");

    let write = fava
        .by(keys.public_key())
        .to([relay_url()])
        .expect("route validates")
        .publish(edit(Kind::ContactList))
        .expect("edit accepts");
    wait_for_signer(&signer, 1).await;
    let newer = signed_source(
        &keys,
        Kind::ContactList,
        u64::MAX - 1,
        "newer source",
        &["unrelated"],
    );
    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            newer.clone(),
            relay_evidence(),
        ))])
        .expect("newer source enters cache");

    let receipt = wait_for_materialization(&fava, write.receipt_id(), 2).await;
    wait_for_signer(&signer, 2).await;
    let EventValue::Unsigned(current) = receipt.current.event else {
        panic!("blocked signer keeps current materialization unsigned");
    };
    assert_eq!(current.content, "newer source|edit");
    assert_eq!(current.tags.as_slice(), newer.tags.as_slice());
    assert_eq!(current.created_at, Timestamp::max());
    assert_eq!(materializer.calls().len(), 2);
    assert_eq!(
        materializer.calls()[1]
            .source
            .as_ref()
            .and_then(EventValue::id),
        Some(newer.id)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn own_local_materialization_does_not_create_a_second_generation() {
    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(MemoryWriteStore::default());
    let signer = Arc::new(BlockingSigner::new(keys.public_key()));
    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList));
    let fava = publication_builder(
        cache,
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::new(RecordingPublisher::default()),
    )
    .materializer(Arc::clone(&materializer))
    .build()
    .expect("semantic publication assembly");

    let write = fava
        .by(keys.public_key())
        .to([relay_url()])
        .expect("route validates")
        .publish(edit(Kind::ContactList))
        .expect("edit accepts");
    wait_for_signer(&signer, 1).await;
    assert_no_receipt_change(&store).await;

    let receipt = write.receipt().expect("receipt exists");
    assert_eq!(
        receipt.current.publication.materialization_id,
        MaterializationId::from_u64(1)
    );
    assert_eq!(materializer.calls().len(), 1);
    assert_eq!(signer.calls(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn source_removal_selects_next_or_empty_once() {
    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(MemoryWriteStore::default());
    let older = signed_source(&keys, Kind::ContactList, u64::MAX - 4, "older", &[]);
    let current = signed_source(&keys, Kind::ContactList, u64::MAX - 2, "current", &[]);
    cache
        .commit(vec![
            CacheMutation::Upsert(CachedEvent::new(older, relay_evidence())),
            CacheMutation::Upsert(CachedEvent::new(current.clone(), relay_evidence())),
        ])
        .expect("source history enters cache");
    let signer = Arc::new(BlockingSigner::new(keys.public_key()));
    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList));
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::new(RecordingPublisher::default()),
    )
    .materializer(Arc::clone(&materializer))
    .build()
    .expect("semantic publication assembly");
    let write = fava
        .by(keys.public_key())
        .to([relay_url()])
        .expect("route validates")
        .publish(edit(Kind::ContactList))
        .expect("edit accepts");
    wait_for_signer(&signer, 1).await;

    cache
        .commit(vec![CacheMutation::Retract {
            event_id: current.id,
            cause: RetractionCause::Evicted,
        }])
        .expect("current source retracts");
    let receipt = wait_for_materialization(&fava, write.receipt_id(), 2).await;
    wait_for_signer(&signer, 2).await;
    assert!(receipt.current.publication.materialization_source.is_none());
    assert!(materializer.calls()[1].source.is_none());

    cache
        .commit(vec![CacheMutation::Retract {
            event_id: current.id,
            cause: RetractionCause::Evicted,
        }])
        .expect("duplicate removal is accepted");
    assert_no_receipt_change(&store).await;
    assert_eq!(materializer.calls().len(), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn semantic_preview_matches_initial_route_with_zero_effects() {
    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(MemoryWriteStore::default());
    let source = signed_source(
        &keys,
        Kind::ContactList,
        u64::MAX - 2,
        "preview source",
        &[],
    );
    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            source,
            relay_evidence(),
        ))])
        .expect("preview source enters cache");
    let signer = Arc::new(BlockingSigner::new(keys.public_key()));
    let publisher = Arc::new(RecordingPublisher::default());
    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList));
    let router = Arc::new(CountingRouter::new(relay_url()));
    let owner = publication_owner(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::clone(&publisher),
        vec![materializer.clone()],
        vec![router.clone()],
    );
    let fava = publication_builder(
        cache,
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::clone(&publisher),
    )
    .router(Arc::clone(&router))
    .materializer(Arc::clone(&materializer))
    .build()
    .expect("semantic publication assembly");
    let intent = automatic_intent(keys.public_key(), Kind::ContactList);
    let mut receipt_changes = store.receipt_changes();

    let preview = owner
        .preview_semantic_routes(&intent)
        .expect("publication-provider semantic preview");
    assert_eq!(store.len().expect("store readable"), 0);
    assert_eq!(signer.calls(), 0);
    assert!(publisher.attempts().is_empty());
    assert_eq!(router.previews(), 1);
    assert_eq!(router.opens(), 0);
    assert_eq!(materializer.calls()[0].author, keys.public_key());
    assert!(matches!(
        receipt_changes.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
    assert!(
        owner
            .preview_semantic_routes(&automatic_intent(keys.public_key(), Kind::MuteList))
            .is_err()
    );
    let addressable = ReplaceableEventEdit::new(
        Kind::Custom(30_001),
        Some("addressable".to_owned()),
        vec![1],
    )
    .expect("bounded addressable edit value");
    assert!(WriteIntent::edit_as(addressable, keys.public_key(), WriteRouting::Automatic).is_ok());
    assert_eq!(store.len().expect("store readable"), 0);

    let write = fava
        .by(keys.public_key())
        .publish(edit(Kind::ContactList))
        .expect("same edit accepts");
    let receipt = wait_for_materialization(&fava, write.receipt_id(), 1).await;
    assert_eq!(
        receipt.desired_destinations,
        preview.destinations.keys().cloned().collect()
    );
    assert_eq!(materializer.calls().len(), 2);
    assert_eq!(
        materializer.calls()[0].created_at,
        materializer.calls()[1].created_at
    );
}
