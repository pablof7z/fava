//! Public evidence that current-account publication resolves authors before custody.

use std::sync::Arc;

use fava::{EventBuilder, Fava, Kind, PublishError, RelayUrl};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_signer::Signer;
use fava_signer_local::LocalSigner;
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::key::Keys;

#[allow(dead_code)]
#[path = "support/semantic_write.rs"]
mod support;

use support::{BlockingSigner, NoopTransport, RecordingPublisher};

#[tokio::test(flavor = "current_thread")]
async fn missing_current_account_refuses_before_custody() {
    let alice = Keys::generate();
    let store = Arc::new(MemoryWriteStore::default());
    let fava = assembly(
        Arc::clone(&store),
        [Arc::new(LocalSigner::new(alice)) as Arc<dyn Signer>],
    );

    let result = fava.publish(EventBuilder::new(Kind::TextNote).content("no current account"));

    assert!(matches!(result, Err(PublishError::MissingAuthor)));
    assert_eq!(
        PublishError::MissingAuthor.to_string(),
        "authorless publication requires a current account selection or explicit author scope"
    );
    assert_eq!(store.len().expect("store remains readable"), 0);
    assert!(
        fava.open_receipts()
            .expect("receipts remain readable")
            .is_empty()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn accepted_author_does_not_follow_current_account() {
    let alice_key = Keys::generate().public_key();
    let bob_key = Keys::generate().public_key();
    let alice_signer = Arc::new(BlockingSigner::new(alice_key));
    let bob_signer = Arc::new(BlockingSigner::new(bob_key));
    let store = Arc::new(MemoryWriteStore::default());
    let fava = assembly(
        Arc::clone(&store),
        [
            Arc::clone(&alice_signer) as Arc<dyn Signer>,
            Arc::clone(&bob_signer) as Arc<dyn Signer>,
        ],
    );

    fava.select_account(alice_key).expect("Alice selects");
    let alice_write = fava
        .to([relay("alice")])
        .expect("explicit route validates")
        .publish(EventBuilder::new(Kind::TextNote).content("accepted as Alice"))
        .expect("current-account builder accepts");
    fava.select_account(bob_key).expect("Bob selects");
    let bob_write = fava
        .publish(EventBuilder::new(Kind::TextNote).content("accepted as Bob"))
        .expect("later current-account builder accepts");

    for _ in 0..100 {
        if alice_signer.calls() == 1 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_ne!(alice_write.write_id(), bob_write.write_id());
    assert_ne!(alice_write.receipt_id(), bob_write.receipt_id());
    assert_eq!(alice_signer.calls(), 1);
    assert_eq!(bob_signer.calls(), 0, "Bob has no route-backed signer work");
    assert_eq!(
        alice_write
            .receipt()
            .expect("Alice receipt")
            .current
            .event
            .author(),
        alice_key
    );
    assert_eq!(
        bob_write
            .receipt()
            .expect("Bob receipt")
            .current
            .event
            .author(),
        bob_key
    );
    assert_eq!(store.len().expect("store remains readable"), 2);
}

fn assembly(
    store: Arc<MemoryWriteStore>,
    signers: impl IntoIterator<Item = Arc<dyn Signer>>,
) -> Fava {
    Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(store)
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .signers(signers)
        .publisher(Arc::new(RecordingPublisher::default()))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .build()
        .expect("publication assembly")
}

fn relay(name: &str) -> RelayUrl {
    RelayUrl::parse(&format!("wss://{name}.example")).expect("relay URL")
}
