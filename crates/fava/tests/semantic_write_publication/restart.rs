use std::sync::Arc;

use fava::{EventBuilder, EventValue, Kind, MaterializationId, ReplaceableEventEdit, Timestamp};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_state::{CacheMutation, CachedEvent};
use fava_write::WriteIntent;
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::key::Keys;

use super::support::{
    BlockingSigner, RecordingPublisher, TestMaterializer, publication_builder, relay_evidence,
    relay_url, signed_source, wait_for_materialization,
};

fn edit(change: u8) -> ReplaceableEventEdit {
    ReplaceableEventEdit::new(Kind::ContactList, None, vec![change]).unwrap()
}

fn content(event: &EventValue) -> &str {
    match event {
        EventValue::Unsigned(event) => &event.content,
        EventValue::Signed(event) => &event.content,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn memory_restart_reconciles_before_immediate_edit_and_late_source_replays_all_edits() {
    let keys = Keys::generate();
    let store = Arc::new(MemoryWriteStore::default());
    let first = store
        .accept_materialized_edit(
            WriteIntent::edit_as(
                edit(1),
                keys.public_key(),
                fava::WriteRouting::explicit([relay_url()]).unwrap(),
            )
            .unwrap(),
            EventBuilder::new(keys.public_key(), Kind::ContactList)
                .created_at(Timestamp::from(1))
                .content("edit")
                .build()
                .unwrap(),
            None,
        )
        .expect("pre-restart custody commits");
    let cache = Arc::new(MemoryEventCache::default());
    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            signed_source(&keys, Kind::ContactList, 10, "restart source", &[]),
            relay_evidence(),
        ))])
        .unwrap();
    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList));
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::new(BlockingSigner::new(keys.public_key())),
        Arc::new(RecordingPublisher::default()),
    )
    .materializer(Arc::clone(&materializer))
    .build()
    .expect("memory recovery reconciles before exposing the facade");

    let reconciled = fava.receipt(first.receipt_id).unwrap().unwrap();
    assert_eq!(
        reconciled.current.publication.materialization_id,
        MaterializationId::from_u64(2)
    );
    assert_eq!(content(&reconciled.current.event), "restart source|edit");

    // A current-thread runtime cannot poll the spawned recovery runner between
    // build and this synchronous admission. This is the deterministic restart
    // barrier: the facade itself must already be reconciled.
    let second = fava
        .by(keys.public_key())
        .to([relay_url()])
        .unwrap()
        .publish(edit(2))
        .expect("immediate same-coordinate edit composes after reconciliation");
    let composed = second.receipt().unwrap();
    assert_eq!(second.write_id(), first.write_id);
    assert_eq!(second.receipt_id(), first.receipt_id);
    assert_eq!(
        composed.current.publication.materialization_id,
        MaterializationId::from_u64(3)
    );
    assert_eq!(content(&composed.current.event), "restart source|edit|edit");

    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            signed_source(&keys, Kind::ContactList, 20, "late source", &[]),
            relay_evidence(),
        ))])
        .unwrap();
    let replayed = wait_for_materialization(&fava, first.receipt_id, 4).await;
    assert_eq!(content(&replayed.current.event), "late source|edit|edit");
}
