use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use fava::{
    Event, EventBuilder, EventValue, Fava, Kind, PublicKey, PublicationError, PublishError,
    Receipt, ReceiptOutcome, ReplaceableEventEdit, ReplaceableEventMaterializer, Write,
    WriteIntentError, WriteStoreError, all,
};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_state::{CacheMutation, CachedEvent};
use fava_write::{WriteIntent, WriteRouting};
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{EventId, Tag};
use nostr::key::Keys;

use super::support::{CountingSigner, RecordingPublisher, publication_builder, relay_evidence};
use super::{EditResult, explicit_intent, signed, target_count};

pub fn assert_source_removal(
    accepted: &Write,
    accepted_receipt: &Receipt,
    removed: &Receipt,
    selected_source: EventId,
    kind: Kind,
    actor: PublicKey,
    target: (&str, &str),
) {
    assert_eq!(
        accepted_receipt.current.publication.materialization_source,
        Some(selected_source)
    );
    assert_eq!(removed.write_id, accepted.write_id());
    assert_eq!(removed.receipt_id, accepted.receipt_id());
    assert_ne!(removed.current.id(), accepted_receipt.current.id());
    assert!(removed.current.event.created_at() > accepted_receipt.current.event.created_at());
    assert!(removed.current.publication.materialization_source.is_none());
    assert_eq!(
        removed.current.publication.retired_materializations.len(),
        1
    );
    assert_eq!(removed.current.event.kind(), kind);
    let EventValue::Unsigned(output) = &removed.current.event else {
        panic!("blocked signer must preserve exact unsigned removal output")
    };
    assert_eq!(output.pubkey, actor);
    assert_eq!(output.content, "");
    assert_eq!(output.tags.len(), 1);
    assert_eq!(output.tags[0].as_slice(), &[target.0, target.1]);
    assert_eq!(output.id, Some(removed.current.id()));
}

pub async fn exercise_public_lifecycle<Add, Remove, Adjacent>(
    kind: Kind,
    materializer: Arc<dyn ReplaceableEventMaterializer>,
    add: Add,
    remove: Remove,
    adjacent: Adjacent,
    tags: (&str, &str, &str),
) where
    Add: Fn() -> EditResult,
    Remove: Fn() -> EditResult,
    Adjacent: Fn() -> EditResult,
{
    prove_first_value(kind, Arc::clone(&materializer), &add, tags.0, tags.1).await;
    prove_composed_writes(
        kind,
        Arc::clone(&materializer),
        &add,
        &remove,
        &adjacent,
        tags,
    )
    .await;
    prove_public_refusals(kind, materializer, add);
}

async fn prove_first_value<Add>(
    kind: Kind,
    materializer: Arc<dyn ReplaceableEventMaterializer>,
    add: &Add,
    tag_name: &str,
    target: &str,
) where
    Add: Fn() -> EditResult,
{
    let keys = Keys::generate();
    let actor = keys.public_key();
    let (empty, empty_cache, empty_store, empty_signer, empty_publisher) =
        assembly(keys.clone(), Arc::clone(&materializer));
    let mut observation = empty
        .observe(
            fava::Query::events()
                .authors([actor])
                .kind(kind)
                .cache_only(),
        )
        .await
        .expect("public semantic query opens");
    let (accepted, receipt, event) = publish_terminal(&empty, add().unwrap(), actor).await;
    let visible = tokio::time::timeout(Duration::from_secs(1), observation.changed())
        .await
        .expect("first value observation is bounded")
        .expect("first value observation remains open");
    assert_stable(&accepted, &receipt);
    assert!(
        accepted
            .receipt()
            .expect("accepted receipt remains readable")
            .current
            .publication
            .materialization_source
            .is_none()
    );
    assert_eq!(target_count(&event, tag_name, target), 1);
    assert_eq!(visible.events.len(), 1);
    assert_eq!(empty_signer.calls(), 1);
    assert_eq!(empty_publisher.attempts().len(), 1);
    assert!(empty_cache.is_empty().unwrap());
    assert_eq!(empty_store.len().unwrap(), 1);
}

async fn prove_composed_writes<Add, Remove, Adjacent>(
    kind: Kind,
    materializer: Arc<dyn ReplaceableEventMaterializer>,
    add: &Add,
    remove: &Remove,
    adjacent: &Adjacent,
    tags: (&str, &str, &str),
) where
    Add: Fn() -> EditResult,
    Remove: Fn() -> EditResult,
    Adjacent: Fn() -> EditResult,
{
    let (tag_name, target, adjacent_target) = tags;
    let keys = Keys::generate();
    let actor = keys.public_key();
    let base = signed(
        &keys,
        kind,
        10,
        "opaque",
        vec![Tag::parse(["x", "unrelated", "bytes"]).unwrap()],
    );
    let cache = Arc::new(MemoryEventCache::default());
    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            base.clone(),
            relay_evidence(),
        ))])
        .unwrap();
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
    .unwrap();

    let (added_write, added_receipt, added) = publish_terminal(&fava, add().unwrap(), actor).await;
    assert_stable(&added_write, &added_receipt);
    assert_eq!(
        added_receipt.current.publication.materialization_source,
        Some(base.id)
    );
    assert_eq!(target_count(&added, tag_name, target), 1);
    assert_preserved(&added);

    let (duplicate_write, duplicate_receipt, duplicate) =
        publish_terminal(&fava, add().unwrap(), actor).await;
    assert_stable(&duplicate_write, &duplicate_receipt);
    assert_eq!(
        duplicate_receipt.current.publication.materialization_source,
        Some(added.id)
    );
    assert_eq!(duplicate.content, added.content);
    assert_eq!(duplicate.tags, added.tags);
    assert_eq!(target_count(&duplicate, tag_name, target), 1);

    let (adjacent_write, adjacent_receipt, adjacent_event) =
        publish_terminal(&fava, adjacent().unwrap(), actor).await;
    assert_stable(&adjacent_write, &adjacent_receipt);
    assert_eq!(
        adjacent_receipt.current.publication.materialization_source,
        Some(duplicate.id)
    );
    assert_eq!(target_count(&adjacent_event, tag_name, target), 1);
    assert_eq!(target_count(&adjacent_event, tag_name, adjacent_target), 1);
    assert_preserved(&adjacent_event);

    let (removed_write, removed_receipt, removed) =
        publish_terminal(&fava, remove().unwrap(), actor).await;
    assert_stable(&removed_write, &removed_receipt);
    assert_eq!(
        removed_receipt.current.publication.materialization_source,
        Some(adjacent_event.id)
    );
    assert_eq!(target_count(&removed, tag_name, target), 0);
    assert_eq!(target_count(&removed, tag_name, adjacent_target), 1);
    assert_preserved(&removed);
    assert_eq!(signer.calls(), 4);
    assert_eq!(publisher.attempts().len(), 4);

    let (inverse_empty, _, _, _, _) = assembly(keys, materializer);
    let (_, _, empty_event) = publish_terminal(&inverse_empty, remove().unwrap(), actor).await;
    assert!(empty_event.tags.is_empty());
}

fn prove_public_refusals<Add>(
    kind: Kind,
    materializer: Arc<dyn ReplaceableEventMaterializer>,
    add: Add,
) where
    Add: Fn() -> EditResult,
{
    let keys = Keys::generate();
    let actor = keys.public_key();
    let edit = add().unwrap();
    let malformed = ReplaceableEventEdit::new(edit.kind(), None, Vec::new()).unwrap();
    let (fava, _, store, signer, publisher) = assembly(keys.clone(), Arc::clone(&materializer));
    assert!(matches!(
        fava.by(actor)
            .to([super::support::relay_url()])
            .expect("route validates")
            .publish(malformed),
        Err(PublishError::Publication(PublicationError::Routing(_)))
    ));
    assert_no_effects(&store, &signer, &publisher, 0);

    let hostile = signed(
        &keys,
        kind,
        10,
        "hostile",
        (0..2_001)
            .map(|index| Tag::parse(["x", &index.to_string()]).unwrap())
            .collect(),
    );
    let cache = Arc::new(MemoryEventCache::default());
    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            hostile,
            relay_evidence(),
        ))])
        .unwrap();
    let store = Arc::new(MemoryWriteStore::default());
    let signer = Arc::new(CountingSigner::new(keys.clone()));
    let publisher = Arc::new(RecordingPublisher::default());
    let bounded = publication_builder(
        cache,
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::clone(&publisher),
    )
    .materializers([Arc::clone(&materializer)])
    .build()
    .unwrap();
    assert!(matches!(
        bounded
            .by(actor)
            .to([super::support::relay_url()])
            .expect("route validates")
            .publish(edit.clone()),
        Err(PublishError::Publication(PublicationError::Routing(_)))
    ));
    assert_no_effects(&store, &signer, &publisher, 0);

    let capacity_store = Arc::new(MemoryWriteStore::bounded(NonZeroUsize::new(1).unwrap()));
    let capacity = publication_builder(
        Arc::new(MemoryEventCache::default()),
        Arc::clone(&capacity_store),
        Arc::new(CountingSigner::new(keys)),
        Arc::new(RecordingPublisher::default()),
    )
    .materializers([materializer])
    .build()
    .unwrap();
    capacity_store
        .accept_materialized(EventValue::Unsigned(
            EventBuilder::new(actor, Kind::TextNote).build().unwrap(),
        ))
        .unwrap();
    assert!(matches!(
        capacity
            .by(actor)
            .to([super::support::relay_url()])
            .expect("route validates")
            .publish(edit),
        Err(PublishError::Publication(PublicationError::Store(
            WriteStoreError::Refused(_)
        )))
    ));
    assert!(matches!(
        WriteIntent::edit_as(add().unwrap(), actor, WriteRouting::Explicit(Vec::new()),),
        Err(WriteIntentError::EmptyExplicitRelays)
    ));
    let neutral_fixture = explicit_intent(add().unwrap(), actor);
    assert_eq!(neutral_fixture.author(), actor);
    assert!(matches!(
        neutral_fixture.routing(),
        WriteRouting::Explicit(relays) if relays.as_slice() == [super::support::relay_url()]
    ));
    assert!(matches!(
        ReplaceableEventEdit::new(kind, Some("addressable".to_owned()), vec![1]),
        Err(WriteIntentError::InvalidEvent(_))
    ));
}

fn assembly(
    keys: Keys,
    materializer: Arc<dyn ReplaceableEventMaterializer>,
) -> (
    Fava,
    Arc<MemoryEventCache>,
    Arc<MemoryWriteStore>,
    Arc<CountingSigner>,
    Arc<RecordingPublisher>,
) {
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(MemoryWriteStore::default());
    let signer = Arc::new(CountingSigner::new(keys));
    let publisher = Arc::new(RecordingPublisher::default());
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::clone(&publisher),
    )
    .materializers([materializer])
    .build()
    .unwrap();
    (fava, cache, store, signer, publisher)
}

async fn publish_terminal(
    fava: &Fava,
    edit: ReplaceableEventEdit,
    author: PublicKey,
) -> (Write, Receipt, Event) {
    let accepted = fava
        .by(author)
        .to([super::support::relay_url()])
        .expect("route validates")
        .publish(edit)
        .unwrap();
    let receipt = tokio::time::timeout(Duration::from_secs(1), accepted.settled(all()))
        .await
        .expect("terminal receipt wait is bounded")
        .unwrap();
    let EventValue::Signed(event) = receipt.current.event.clone() else {
        panic!("terminal semantic event must be signed")
    };
    assert_eq!(receipt.outcome, ReceiptOutcome::Complete);
    (accepted, receipt, event)
}

fn assert_stable(accepted: &Write, receipt: &Receipt) {
    let accepted_receipt = accepted
        .receipt()
        .expect("accepted receipt remains readable");
    assert_eq!(accepted.write_id(), receipt.write_id);
    assert_eq!(accepted.receipt_id(), receipt.receipt_id);
    assert_eq!(accepted_receipt.current.id(), receipt.current.id());
}

fn assert_preserved(event: &Event) {
    assert_eq!(event.content, "opaque");
    assert_eq!(
        event
            .tags
            .iter()
            .filter(|tag| tag.as_slice() == ["x", "unrelated", "bytes"])
            .count(),
        1
    );
}

fn assert_no_effects(
    store: &MemoryWriteStore,
    signer: &CountingSigner,
    publisher: &RecordingPublisher,
    expected_store_len: usize,
) {
    assert_eq!(store.len().unwrap(), expected_store_len);
    assert_eq!(signer.calls(), 0);
    assert!(publisher.attempts().is_empty());
}
