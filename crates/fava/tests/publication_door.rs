//! Public tracer for the application-facing synchronous publication door.

use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use fava::{EventBuilder, EventValue, Fava, Kind, PublishError, Query, ReceiptOutcome, Timestamp};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_signer::Signer;
use fava_signer_local::LocalSigner;
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
use nostr::key::Keys;

#[allow(dead_code)]
#[path = "support/semantic_write.rs"]
mod support;

use support::{BlockingSigner, NoopTransport, RecordingPublisher};

#[tokio::test(flavor = "current_thread")]
async fn publish_payload_forms_share_one_door_and_unscoped_edit_refuses() {
    let keys = Keys::generate();
    let store = Arc::new(MemoryWriteStore::default());
    let fava = assembly(
        Arc::clone(&store),
        [Arc::new(LocalSigner::new(keys.clone())) as Arc<dyn Signer>],
        [fava_nip02::materializer()],
    );

    let unsigned = EventBuilder::new(keys.public_key(), Kind::TextNote)
        .content("unsigned")
        .build()
        .expect("unsigned event builds");
    let signed = NostrEventBuilder::new(Kind::TextNote, "signed")
        .finalize(&keys)
        .expect("event signs");

    let unsigned_write = fava.publish(unsigned).expect("unsigned payload accepts");
    let signed_write = fava.publish(signed).expect("signed payload accepts");
    let edit_error = fava
        .publish(fava_nip02::follow(Keys::generate().public_key()).expect("edit builds"))
        .expect_err("unscoped edit has no selected author");

    assert_ne!(unsigned_write.write_id(), signed_write.write_id());
    assert!(matches!(edit_error, PublishError::MissingAuthor));
    assert_eq!(store.len().expect("store remains readable"), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn publish_returns_after_local_acceptance() {
    let keys = Keys::generate();
    let store = Arc::new(MemoryWriteStore::default());
    let signer = Arc::new(BlockingSigner::new(keys.public_key()));
    let fava = assembly(
        Arc::clone(&store),
        [Arc::clone(&signer) as Arc<dyn Signer>],
        std::iter::empty(),
    );
    let event = EventBuilder::new(keys.public_key(), Kind::TextNote)
        .content("accepted before downstream progress")
        .build()
        .expect("event builds");
    let event_id = event.id.expect("event is finalized");

    let runtime = tokio::runtime::Handle::current();
    let publish_fava = fava.clone();
    let (sent, received) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let _runtime = runtime.enter();
        sent.send(publish_fava.publish(event))
            .expect("test receiver remains open");
    });
    let write = received
        .recv_timeout(Duration::from_millis(250))
        .expect("publish returns after local acceptance")
        .expect("payload accepts");

    assert_eq!(signer.calls(), 0, "downstream signer has not advanced");
    let observation = fava
        .observe(
            Query::events()
                .kinds([Kind::TextNote])
                .expect("one kind is bounded")
                .cache_only(),
        )
        .await
        .expect("local observation opens");
    assert!(
        observation
            .current()
            .events
            .iter()
            .any(|record| record.event.id() == Some(event_id))
    );
    let receipt = write.receipt().expect("accepted receipt is readable");
    assert_eq!(receipt.write_id, write.write_id());
    assert_eq!(receipt.receipt_id, write.receipt_id());
    assert!(matches!(receipt.current.event, EventValue::Unsigned(_)));
    assert_eq!(receipt.outcome, ReceiptOutcome::Open);
    assert_eq!(store.len().expect("store remains readable"), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn equivalent_publications_have_distinct_custody_identities() {
    let keys = Keys::generate();
    let store = Arc::new(MemoryWriteStore::default());
    let fava = assembly(Arc::clone(&store), std::iter::empty(), std::iter::empty());
    let event = NostrEventBuilder::new(Kind::TextNote, "same signed event")
        .finalize(&keys)
        .expect("event signs");

    let first = fava.publish(event.clone()).expect("first payload accepts");
    let second = fava.publish(event).expect("second payload accepts");

    assert_ne!(first.write_id(), second.write_id());
    assert_ne!(first.receipt_id(), second.receipt_id());
    assert_eq!(store.len().expect("store remains readable"), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn invalid_payload_refuses_without_custody() {
    let keys = Keys::generate();
    let store = Arc::new(MemoryWriteStore::default());
    let fava = assembly(
        Arc::clone(&store),
        [Arc::new(LocalSigner::new(keys.clone())) as Arc<dyn Signer>],
        std::iter::empty(),
    );
    let expired = EventBuilder::new(keys.public_key(), Kind::TextNote)
        .created_at(Timestamp::from(1))
        .tag(nostr::event::Tag::expiration(Timestamp::from(2)))
        .build()
        .expect("expired event body is structurally valid");

    assert!(fava.publish(expired).is_err());
    assert_eq!(store.len().expect("store remains readable"), 0);
    assert!(
        fava.open_receipts()
            .expect("receipts remain readable")
            .is_empty()
    );
    let observation = fava
        .observe(
            Query::events()
                .kinds([Kind::TextNote])
                .expect("one kind is bounded")
                .cache_only(),
        )
        .await
        .expect("local observation opens");
    assert!(observation.current().events.is_empty());
}

fn assembly(
    store: Arc<MemoryWriteStore>,
    signers: impl IntoIterator<Item = Arc<dyn Signer>>,
    materializers: impl IntoIterator<Item = Arc<dyn fava::ReplaceableEventMaterializer>>,
) -> Fava {
    let cache = Arc::new(MemoryEventCache::default());
    let publisher = Arc::new(RecordingPublisher::default());
    Fava::builder()
        .event_cache(cache)
        .write_store(store)
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .signers(signers)
        .materializers(materializers)
        .publisher(publisher)
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .build()
        .expect("ordinary publication assembly")
}
