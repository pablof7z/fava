//! Public evidence for inert signer and explicit-relay publication scopes.

use std::collections::BTreeSet;
use std::sync::Arc;

use fava::{
    EventBuilder, Fava, Kind, PublishError, Query, RelayUrl, WriteIntentError, WriteRouting,
};
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
use nostr::key::Keys;

#[allow(dead_code)]
#[path = "support/semantic_write.rs"]
mod support;

use support::{CountingSigner, NoopTransport, RecordingPublisher};

#[tokio::test(flavor = "current_thread")]
async fn signer_and_relay_scopes_compose_in_both_orders() {
    let author = Keys::generate();
    let target = Keys::generate().public_key();
    let store = Arc::new(MemoryWriteStore::default());
    let (fava, _, _) = assembly(Arc::clone(&store), author.clone());
    let first_route = [relay("first"), relay("second")];
    let second_route = [relay("second"), relay("first")];

    let first = fava
        .by(author.public_key())
        .to(first_route.clone())
        .expect("explicit route validates")
        .publish(fava_nip02::follow(target).expect("first edit builds"))
        .expect("by then to accepts");
    let second = fava
        .to(second_route.clone())
        .expect("explicit route validates")
        .by(author.public_key())
        .publish(fava_nip02::follow(target).expect("second edit builds"))
        .expect("to then by accepts");

    assert_ne!(first.write_id(), second.write_id());
    assert_ne!(first.receipt_id(), second.receipt_id());
    for (write, expected) in [
        (&first, BTreeSet::from(first_route)),
        (&second, BTreeSet::from(second_route)),
    ] {
        let receipt = write.receipt().expect("receipt remains readable");
        assert_eq!(receipt.current.event.author(), author.public_key());
        assert_eq!(receipt.routing, WriteRouting::Explicit(expected));
    }
    assert_eq!(store.len().expect("store remains readable"), 2);

    let observation = fava
        .observe(
            Query::events()
                .authors([author.public_key()])
                .kind(Kind::ContactList)
                .cache_only(),
        )
        .await
        .expect("semantic observation opens");
    assert_eq!(
        observation.current().events.len(),
        1,
        "equivalent edits keep one semantic winner while custody identities stay distinct"
    );
}

#[test]
fn relay_scope_publishes_unsigned_and_presigned_payloads() {
    let author = Keys::generate();
    let store = Arc::new(MemoryWriteStore::default());
    let (fava, _, _) = assembly(Arc::clone(&store), author.clone());
    let destination = relay("payloads");
    let unsigned = EventBuilder::new(author.public_key(), Kind::TextNote)
        .content("unsigned")
        .build()
        .expect("unsigned event builds");
    let presigned = NostrEventBuilder::new(Kind::TextNote, "presigned")
        .finalize(&author)
        .expect("event signs");

    fava.to([destination.clone()])
        .expect("route validates")
        .publish(unsigned)
        .expect("unsigned payload accepts");
    fava.to([destination])
        .expect("route validates")
        .publish(presigned)
        .expect("presigned payload accepts");

    assert_eq!(store.len().expect("store remains readable"), 2);
}

#[test]
fn publication_scopes_are_inert_before_valid_payload() {
    let author = Keys::generate();
    let store = Arc::new(MemoryWriteStore::default());
    let (fava, signer, publisher) = assembly(Arc::clone(&store), author.clone());

    drop(fava.by(author.public_key()));
    drop(fava.to([relay("dropped")]).expect("route validates"));
    assert_no_effects(&store, &signer, &publisher);

    let empty = fava.to(std::iter::empty::<RelayUrl>());
    assert!(matches!(
        empty,
        Err(PublishError::Intent(WriteIntentError::EmptyExplicitRelays))
    ));
    assert_no_effects(&store, &signer, &publisher);

    let too_many = fava.to((0..257).map(|index| relay(&format!("bounded-{index}"))));
    assert!(matches!(
        too_many,
        Err(PublishError::Intent(
            WriteIntentError::TooManyExplicitRelays {
                actual: 257,
                maximum: 256
            }
        ))
    ));
    assert_no_effects(&store, &signer, &publisher);
}

fn assembly(
    store: Arc<MemoryWriteStore>,
    author: Keys,
) -> (Fava, Arc<CountingSigner>, Arc<RecordingPublisher>) {
    let cache = Arc::new(MemoryEventCache::default());
    let signer = Arc::new(CountingSigner::new(author));
    let publisher = Arc::new(RecordingPublisher::default());
    let fava = Fava::builder()
        .event_cache(cache)
        .write_store(store)
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .signer(Arc::clone(&signer))
        .materializers([fava_nip02::materializer()])
        .publisher(Arc::clone(&publisher))
        .delivery_policy(Arc::new(
            fava_delivery_standard::StandardDeliveryPolicy::default(),
        ))
        .build()
        .expect("publication assembly");
    (fava, signer, publisher)
}

fn assert_no_effects(
    store: &MemoryWriteStore,
    signer: &CountingSigner,
    publisher: &RecordingPublisher,
) {
    assert_eq!(store.len().expect("store remains readable"), 0);
    assert!(
        store
            .recover_open()
            .expect("receipts remain readable")
            .is_empty()
    );
    assert_eq!(signer.calls(), 0);
    assert!(publisher.attempts().is_empty());
}

fn relay(name: &str) -> RelayUrl {
    RelayUrl::parse(&format!("wss://{name}.example")).expect("relay URL")
}
