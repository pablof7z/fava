//! Public-facade evidence for signing an artifact without publishing it.

use std::sync::Arc;

use fava::{EventBuilder, Fava, Kind};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_signer_local::LocalSigner;
use fava_write_store_memory::MemoryWriteStore;
use nostr::key::Keys;

#[allow(dead_code)]
#[path = "support/semantic_write.rs"]
mod support;

use support::{NoopTransport, RecordingPublisher};

#[tokio::test(flavor = "current_thread")]
async fn registered_author_signs_without_a_publication() {
    let author = Keys::generate();
    let publisher = Arc::new(RecordingPublisher::default());
    let fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .signer(Arc::new(LocalSigner::new(author.clone())))
        .publisher(Arc::clone(&publisher))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .build()
        .expect("Fava assembly");
    let unsigned = EventBuilder::new(Kind::TextNote)
        .content("signed without a relay write")
        .by(author.public_key())
        .build()
        .expect("unsigned event builds");

    let signed = fava.sign(unsigned).await.expect("Fava signs exact author");

    assert_eq!(signed.pubkey, author.public_key());
    assert!(signed.verify().is_ok(), "Fava signature verifies");
    assert!(
        publisher.attempts().is_empty(),
        "direct signing does not publish"
    );
}
