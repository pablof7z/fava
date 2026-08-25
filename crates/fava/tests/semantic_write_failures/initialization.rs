use std::sync::{Arc, Barrier};

use fava::{
    EventBuilder, EventValue, Kind, MaterializationId, ReplaceableEventEdit, Timestamp,
    WriteRouting,
};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_state::{CacheMutation, CachedEvent};
use fava_write::WriteIntent;
use fava_write_store::WriteStore;
use nostr::key::Keys;

use super::faults::FaultingWriteStore;
use super::support::{
    BlockingSigner, RecordingPublisher, TestMaterializer, publication_builder, relay_evidence,
    relay_url, signed_source,
};

fn edit(change: u8) -> ReplaceableEventEdit {
    ReplaceableEventEdit::new(Kind::ContactList, None, vec![change]).unwrap()
}

fn intent(author: fava::PublicKey, change: u8) -> WriteIntent {
    WriteIntent::edit_as(
        edit(change),
        author,
        WriteRouting::explicit([relay_url()]).unwrap(),
    )
    .unwrap()
}

#[tokio::test(flavor = "current_thread")]
async fn recovery_revalidates_generation_and_complete_custody_before_materializing() {
    let keys = Keys::generate();
    let store = Arc::new(FaultingWriteStore::new());
    let first = store
        .accept_materialized_edit(
            intent(keys.public_key(), 1),
            EventBuilder::new(keys.public_key(), Kind::ContactList)
                .created_at(Timestamp::from(1))
                .content("edit")
                .build()
                .unwrap(),
            None,
        )
        .unwrap();
    let cache = Arc::new(MemoryEventCache::default());
    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            signed_source(&keys, Kind::ContactList, 10, "recovery source", &[]),
            relay_evidence(),
        ))])
        .unwrap();
    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList));
    let barrier = Arc::new(Barrier::new(2));
    store.pause_after_next_receipt_read(Arc::clone(&barrier));
    let builder = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::new(BlockingSigner::new(keys.public_key())),
        Arc::new(RecordingPublisher::default()),
    )
    .materializer(Arc::clone(&materializer));
    let runtime = tokio::runtime::Handle::current();
    let build = std::thread::spawn(move || {
        let _entered = runtime.enter();
        builder.build()
    });

    barrier.wait();
    let current = store.receipt(first.receipt_id).unwrap().unwrap();
    let EventValue::Unsigned(current_event) = current.current.event.clone() else {
        panic!("pre-signature recovery custody remains unsigned");
    };
    let second_event = EventBuilder::new(keys.public_key(), Kind::ContactList)
        .created_at(Timestamp::from(current_event.created_at.as_secs() + 1))
        .content(format!("{}|edit", current_event.content))
        .build()
        .unwrap();
    let reservation = store.reserve_active(&edit(2), keys.public_key()).unwrap();
    store
        .accept_reserved_materialized_edit(
            reservation,
            intent(keys.public_key(), 2),
            second_event,
            Some(&EventValue::Unsigned(current_event)),
            None,
        )
        .unwrap();
    barrier.wait();

    let fava = build.join().unwrap().expect("recovery assembles");
    let recovered = fava.receipt(first.receipt_id).unwrap().unwrap();
    assert_eq!(
        recovered.current.publication.materialization_id,
        MaterializationId::from_u64(3)
    );
    let EventValue::Unsigned(recovered) = recovered.current.event else {
        panic!("blocking signer keeps recovery unsigned");
    };
    assert_eq!(recovered.content, "recovery source|edit|edit");
    assert_eq!(
        materializer.calls().len(),
        2,
        "recovery invoked a stale one-edit sequence before exact revalidation"
    );
}
