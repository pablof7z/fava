//! Public-facade evidence for semantic materialization and publication.
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use fava::{
    EventBuilder, EventCoordinate, EventValue, Kind, MaterializationId, ReceiptOutcome,
    ReplaceableEventEdit, ReplaceableEventMaterializer, Timestamp, WriteIntent, WriteRouting,
};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_state::{CacheMutation, CachedEvent};
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::key::Keys;

#[path = "semantic_write_publication/interleavings.rs"]
mod interleavings;
#[path = "semantic_write_publication/shared_capacity.rs"]
mod shared_capacity;
#[path = "support/semantic_write.rs"]
mod support;

use support::*;
#[tokio::test(flavor = "current_thread")]
async fn first_value_edit_publishes_through_public_fava() {
    let keys = Keys::generate();
    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList, EDIT_FORMAT));
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

    let accepted = fava
        .publish(intent(keys.public_key(), Kind::ContactList, EDIT_FORMAT))
        .expect("first semantic value accepts");
    let visible = tokio::time::timeout(Duration::from_secs(1), observation.changed())
        .await
        .expect("local materialization arrives")
        .expect("observation stays open");
    let receipt = fava
        .wait_terminal(accepted.receipt_id)
        .await
        .expect("ordinary receipt settles");

    assert_eq!(accepted.write_id, receipt.write_id);
    assert_eq!(accepted.receipt_id, receipt.receipt_id);
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
    assert_eq!(publisher.attempts()[0].receipt_id, accepted.receipt_id);
    assert_eq!(
        publisher.attempts()[0].materialization_id,
        accepted.current.publication.materialization_id
    );
    assert!(cache.is_empty().expect("cache remains readable"));
    assert_eq!(store.len().expect("store remains readable"), 1);
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
            .publish(intent(keys.public_key(), Kind::ContactList, EDIT_FORMAT))
            .is_err()
    );
    assert_no_effects(&empty_store, &empty_signer, &empty_publisher, 0);

    let selected = Arc::new(TestMaterializer::new(Kind::ContactList, EDIT_FORMAT));
    let (unsupported, _, unsupported_store, unsupported_signer, unsupported_publisher) = assembly(
        Arc::new(MemoryWriteStore::default()),
        keys.clone(),
        vec![selected],
    );
    assert!(
        unsupported
            .publish(intent(keys.public_key(), Kind::Custom(10_003), EDIT_FORMAT,))
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
        Arc::new(TestMaterializer::new(Kind::ContactList, 1))
            as Arc<dyn ReplaceableEventMaterializer>,
        Arc::new(TestMaterializer::new(Kind::ContactList, 2)),
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
        Arc::new(TestMaterializer::new(
            Kind::Custom(10_000 + offset),
            EDIT_FORMAT,
        )) as Arc<dyn ReplaceableEventMaterializer>
    }))
    .build();
    assert!(overflow.is_err());

    let bounded_store = Arc::new(MemoryWriteStore::bounded(NonZeroUsize::new(1).unwrap()));
    let bounded_materializer = Arc::new(TestMaterializer::new(Kind::ContactList, EDIT_FORMAT));
    let (bounded, _, bounded_store, bounded_signer, bounded_publisher) = assembly(
        bounded_store,
        keys.clone(),
        vec![Arc::clone(&bounded_materializer)],
    );
    bounded
        .accept_event(EventValue::Unsigned(
            EventBuilder::new(keys.public_key(), Kind::TextNote)
                .created_at(Timestamp::from(1))
                .build()
                .unwrap(),
        ))
        .expect("one existing active write occupies capacity");
    assert!(
        bounded
            .publish(intent(keys.public_key(), Kind::ContactList, EDIT_FORMAT))
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
    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList, EDIT_FORMAT));
    let (fava, _, _, _, publisher) = assembly_with_cache(
        cache,
        Arc::new(MemoryWriteStore::default()),
        keys,
        vec![materializer.clone()],
    );

    let accepted = fava
        .publish(intent(source.pubkey, Kind::ContactList, EDIT_FORMAT))
        .expect("source-backed edit accepts");
    let receipt = fava
        .wait_terminal(accepted.receipt_id)
        .await
        .expect("publication settles");
    let calls = materializer.calls();

    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].source.as_ref().map(|event| event.id),
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
    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList, EDIT_FORMAT));
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

    let accepted = fava
        .publish(intent(keys.public_key(), Kind::ContactList, EDIT_FORMAT))
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

    let receipt = wait_for_materialization(&fava, accepted.receipt_id, 2).await;
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
            .map(|event| event.id),
        Some(newer.id)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn own_local_materialization_does_not_create_a_second_generation() {
    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(MemoryWriteStore::default());
    let signer = Arc::new(BlockingSigner::new(keys.public_key()));
    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList, EDIT_FORMAT));
    let fava = publication_builder(
        cache,
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::new(RecordingPublisher::default()),
    )
    .materializer(Arc::clone(&materializer))
    .build()
    .expect("semantic publication assembly");

    let accepted = fava
        .publish(intent(keys.public_key(), Kind::ContactList, EDIT_FORMAT))
        .expect("edit accepts");
    wait_for_signer(&signer, 1).await;
    assert_no_receipt_change(&store).await;

    let receipt = fava
        .receipt(accepted.receipt_id)
        .expect("receipt read")
        .expect("receipt exists");
    assert_eq!(
        receipt.current.publication.materialization_id,
        MaterializationId::from_u64(1)
    );
    assert_eq!(materializer.calls().len(), 1);
    assert_eq!(signer.calls(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn equal_older_unqualified_and_duplicate_sources_are_inert() {
    let keys = Keys::generate();
    let other = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(MemoryWriteStore::default());
    let left = signed_source(&keys, Kind::ContactList, u64::MAX - 2, "left", &[]);
    let right = signed_source(&keys, Kind::ContactList, u64::MAX - 2, "right", &[]);
    let (base, equal) = if left.id > right.id {
        (left, right)
    } else {
        (right, left)
    };
    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            base.clone(),
            relay_evidence(),
        ))])
        .expect("base source enters cache");
    let signer = Arc::new(BlockingSigner::new(keys.public_key()));
    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList, EDIT_FORMAT));
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::new(RecordingPublisher::default()),
    )
    .materializer(Arc::clone(&materializer))
    .build()
    .expect("semantic publication assembly");
    let accepted = fava
        .publish(intent(keys.public_key(), Kind::ContactList, EDIT_FORMAT))
        .expect("edit accepts");
    wait_for_signer(&signer, 1).await;

    let older = signed_source(&keys, Kind::ContactList, u64::MAX - 3, "older", &[]);
    let wrong_actor = signed_source(&other, Kind::ContactList, u64::MAX - 1, "wrong actor", &[]);
    let wrong_kind = signed_source(&keys, Kind::TextNote, u64::MAX - 1, "wrong kind", &[]);
    cache
        .commit(vec![
            CacheMutation::Upsert(CachedEvent::new(equal, relay_evidence())),
            CacheMutation::Upsert(CachedEvent::new(older, relay_evidence())),
            CacheMutation::Upsert(CachedEvent::new(wrong_actor, relay_evidence())),
            CacheMutation::Upsert(CachedEvent::new(wrong_kind, relay_evidence())),
            CacheMutation::Upsert(CachedEvent::new(base, relay_evidence())),
        ])
        .expect("inert source facts enter cache");
    assert_no_receipt_change(&store).await;

    let receipt = fava
        .receipt(accepted.receipt_id)
        .expect("receipt read")
        .expect("receipt exists");
    assert_eq!(
        receipt.current.publication.materialization_id,
        MaterializationId::from_u64(1)
    );
    assert_eq!(materializer.calls().len(), 1);
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
    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList, EDIT_FORMAT));
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::new(RecordingPublisher::default()),
    )
    .materializer(Arc::clone(&materializer))
    .build()
    .expect("semantic publication assembly");
    let accepted = fava
        .publish(intent(keys.public_key(), Kind::ContactList, EDIT_FORMAT))
        .expect("edit accepts");
    wait_for_signer(&signer, 1).await;

    cache
        .commit(vec![CacheMutation::Retract(current.id)])
        .expect("current source retracts");
    let receipt = wait_for_materialization(&fava, accepted.receipt_id, 2).await;
    wait_for_signer(&signer, 2).await;
    assert!(receipt.current.publication.materialization_source.is_none());
    assert!(materializer.calls()[1].source.is_none());

    cache
        .commit(vec![CacheMutation::Retract(current.id)])
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
    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList, EDIT_FORMAT));
    let router = Arc::new(CountingRouter::new(relay_url()));
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
    let intent = automatic_intent(keys.public_key(), Kind::ContactList, EDIT_FORMAT);
    let mut receipt_changes = store.receipt_changes();

    let preview = fava
        .preview_write_routes(&intent)
        .expect("semantic preview");
    assert_eq!(store.len().expect("store readable"), 0);
    assert_eq!(signer.calls(), 0);
    assert!(publisher.attempts().is_empty());
    assert_eq!(router.previews(), 1);
    assert_eq!(router.opens(), 0);
    assert!(matches!(
        receipt_changes.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
    assert!(
        fava.preview_write_routes(&automatic_intent(
            keys.public_key(),
            Kind::ContactList,
            EDIT_FORMAT + 1,
        ))
        .is_err()
    );
    let addressable = ReplaceableEventEdit::new(
        keys.public_key(),
        EventCoordinate::Replaceable {
            author: keys.public_key(),
            kind: Kind::Custom(30_001),
            identifier: Some("addressable".to_owned()),
        },
        EDIT_FORMAT,
        vec![1],
        vec![2],
    )
    .expect("bounded addressable edit value");
    assert!(WriteIntent::edit(addressable, WriteRouting::Automatic).is_err());
    assert_eq!(store.len().expect("store readable"), 0);

    let accepted = fava.publish(intent).expect("same edit accepts");
    let receipt = wait_for_materialization(&fava, accepted.receipt_id, 1).await;
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
