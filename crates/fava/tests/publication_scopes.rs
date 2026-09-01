//! Public evidence for inert signer and explicit-relay publication scopes.

use std::sync::Arc;

use fava::{
    EventBuilder, Fava, Kind, PublishError, Query, RelayUrl, WriteIntentError, WriteRouting,
};
use fava_event_cache_memory::MemoryEventCache;
use fava_nip02::Nip02;
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

use support::{CountingSigner, NoopTransport, RecordingPublisher};

#[tokio::test(flavor = "current_thread")]
async fn signer_and_relay_scopes_compose_in_both_orders() {
    let author = Keys::generate();
    let target = Keys::generate().public_key();
    let store = Arc::new(MemoryWriteStore::default());
    let (fava, _, _, second_author) = assembly(Arc::clone(&store), author.clone());
    let first_route = [relay("first"), relay("second")];
    let second_route = [relay("second"), relay("first")];

    let first_scope = fava
        .with_account(second_author.public_key())
        .to(first_route.clone())
        .expect("explicit route validates");
    let second_scope = fava
        .to(second_route.clone())
        .expect("explicit route validates")
        .with_account(author.public_key());
    let first = first_scope
        .publish(fava_nip02::follow(target).expect("first edit builds"))
        .expect("by then to accepts");
    let second = second_scope
        .publish(fava_nip02::follow(target).expect("second edit builds"))
        .expect("to then by accepts");

    assert_ne!(first.write_id(), second.write_id());
    assert_ne!(first.receipt_id(), second.receipt_id());
    for (write, expected_author, expected_route) in [
        (&first, second_author.public_key(), first_route.to_vec()),
        (&second, author.public_key(), second_route.to_vec()),
    ] {
        let receipt = write.receipt().expect("receipt remains readable");
        assert_eq!(receipt.current.event.author(), expected_author);
        assert_eq!(receipt.routing, WriteRouting::Explicit(expected_route));
    }
    assert_eq!(store.len().expect("store remains readable"), 2);

    let observation = fava
        .observe(
            Query::events()
                .kinds([Kind::ContactList])
                .expect("one kind is bounded")
                .authors([author.public_key()])
                .expect("one author is bounded")
                .cache_only(),
        )
        .await
        .expect("semantic observation opens");
    assert_eq!(
        observation.current().events.len(),
        1,
        "the first author retains one semantic winner"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn authorless_builder_composes_with_signer_and_relay_scopes_in_both_orders() {
    let author = Keys::generate();
    let store = Arc::new(MemoryWriteStore::default());
    let (fava, _, _, second_author) = assembly(Arc::clone(&store), author.clone());
    let first_route = [relay("authorless-first"), relay("authorless-second")];
    let second_route = [relay("authorless-second"), relay("authorless-first")];

    let first = fava
        .with_account(second_author.public_key())
        .to(first_route.clone())
        .expect("explicit route validates")
        .publish(EventBuilder::new(Kind::TextNote).content("by then to"))
        .expect("by then to accepts an authorless builder");
    let second = fava
        .to(second_route.clone())
        .expect("explicit route validates")
        .with_account(author.public_key())
        .publish(EventBuilder::new(Kind::TextNote).content("to then by"))
        .expect("to then by accepts an authorless builder");

    assert_ne!(first.write_id(), second.write_id());
    for (write, expected_author, expected_route) in [
        (&first, second_author.public_key(), first_route.to_vec()),
        (&second, author.public_key(), second_route.to_vec()),
    ] {
        let receipt = write.receipt().expect("receipt remains readable");
        assert_eq!(receipt.current.event.author(), expected_author);
        assert_eq!(receipt.routing, WriteRouting::Explicit(expected_route));
    }
    assert_eq!(store.len().expect("store remains readable"), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn authorless_builder_without_an_author_scope_refuses_with_no_effects() {
    let author = Keys::generate();
    let store = Arc::new(MemoryWriteStore::default());
    let (fava, signer, publisher, _) = assembly(Arc::clone(&store), author.clone());

    let unscoped = fava.publish(EventBuilder::new(Kind::TextNote).content("no scope at all"));
    assert!(matches!(unscoped, Err(PublishError::MissingAuthor)));
    assert_no_effects(&store, &signer, &publisher);

    let relay_scoped_only = fava
        .to([relay("authorless-refusal")])
        .expect("route validates")
        .publish(EventBuilder::new(Kind::TextNote).content("relay scope but no author scope"));
    assert!(matches!(
        relay_scoped_only,
        Err(PublishError::MissingAuthor)
    ));
    assert_no_effects(&store, &signer, &publisher);
}

#[tokio::test(flavor = "current_thread")]
async fn a_cloned_authorless_builder_publishes_under_a_different_author() {
    let author = Keys::generate();
    let store = Arc::new(MemoryWriteStore::default());
    let (fava, _, _, second_author) = assembly(Arc::clone(&store), author.clone());
    let builder = EventBuilder::new(Kind::TextNote).content("constructed exactly once");

    let first = fava
        .with_account(author.public_key())
        .publish(builder.clone())
        .expect("first author accepts the constructed value");
    let second = fava
        .with_account(second_author.public_key())
        .publish(builder)
        .expect("second author accepts the same constructed value, unreconstructed");

    assert_ne!(first.write_id(), second.write_id());
    assert_eq!(
        first
            .receipt()
            .expect("first receipt remains readable")
            .current
            .event
            .author(),
        author.public_key()
    );
    assert_eq!(
        second
            .receipt()
            .expect("second receipt remains readable")
            .current
            .event
            .author(),
        second_author.public_key()
    );
    assert_eq!(store.len().expect("store remains readable"), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn authorless_builder_route_conflicts_with_narrowed_scope_in_either_order() {
    let author = Keys::generate();
    let store = Arc::new(MemoryWriteStore::default());
    let (fava, signer, publisher, _) = assembly(Arc::clone(&store), author.clone());
    let embedded = relay("authorless-conflict-embedded");

    let by_then_to = fava
        .with_account(author.public_key())
        .to([relay("authorless-conflict-facade-a")])
        .expect("facade route validates")
        .publish(
            EventBuilder::new(Kind::TextNote)
                .content("authorless builder route")
                .to_relays([embedded.clone()])
                .expect("embedded route validates"),
        );
    assert!(matches!(
        by_then_to,
        Err(PublishError::Intent(
            WriteIntentError::ConflictingExplicitRoutes
        ))
    ));
    assert_no_effects(&store, &signer, &publisher);

    let to_then_by = fava
        .to([relay("authorless-conflict-facade-b")])
        .expect("facade route validates")
        .with_account(author.public_key())
        .publish(
            EventBuilder::new(Kind::TextNote)
                .content("authorless builder route")
                .to_relays([embedded.clone()])
                .expect("embedded route validates"),
        );
    assert!(matches!(
        to_then_by,
        Err(PublishError::Intent(
            WriteIntentError::ConflictingExplicitRoutes
        ))
    ));
    assert_no_effects(&store, &signer, &publisher);

    let automatic = fava
        .with_account(author.public_key())
        .publish(
            EventBuilder::new(Kind::TextNote)
                .content("authorless builder route")
                .to_relays([embedded.clone()])
                .expect("embedded route validates"),
        )
        .expect("authorless builder route publishes directly under an author scope alone");
    assert_eq!(
        automatic
            .receipt()
            .expect("automatic receipt remains readable")
            .routing,
        WriteRouting::Explicit(vec![embedded])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn explicit_route_normalizes_duplicates_without_reordering() {
    let author = Keys::generate();
    let store = Arc::new(MemoryWriteStore::default());
    let (fava, _, _, _) = assembly(Arc::clone(&store), author.clone());
    let first = relay("first-normalized");
    let second = relay("second-normalized");
    let event = EventBuilder::new(Kind::TextNote)
        .content("ordered route")
        .by(author.public_key())
        .build()
        .expect("event builds");

    let write = fava
        .to([first.clone(), second.clone(), first.clone()])
        .expect("duplicate route normalizes")
        .publish(event)
        .expect("payload accepts");
    let receipt = write.receipt().expect("receipt remains readable");

    assert_eq!(receipt.routing, WriteRouting::Explicit(vec![first, second]));
    assert_eq!(receipt.destinations().len(), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn equivalent_explicitly_authored_edits_have_distinct_custody_identities() {
    let author = Keys::generate();
    let target = Keys::generate().public_key();
    let store = Arc::new(MemoryWriteStore::default());
    let (fava, _, _, _) = assembly(Arc::clone(&store), author.clone());
    let edit = fava_nip02::follow(target).expect("edit builds");

    let first = fava
        .with_account(author.public_key())
        .publish(edit.clone())
        .expect("first edit accepts");
    let second = fava
        .with_account(author.public_key())
        .publish(edit)
        .expect("equivalent edit accepts separately");

    assert_ne!(first.write_id(), second.write_id());
    assert_ne!(first.receipt_id(), second.receipt_id());
    assert_eq!(store.len().expect("store remains readable"), 2);
    let observation = fava
        .observe(
            Query::events()
                .kinds([Kind::ContactList])
                .expect("one kind is bounded")
                .authors([author.public_key()])
                .expect("one author is bounded")
                .cache_only(),
        )
        .await
        .expect("semantic observation opens");
    assert_eq!(observation.current().events.len(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn relay_scope_publishes_unsigned_and_presigned_payloads() {
    let author = Keys::generate();
    let store = Arc::new(MemoryWriteStore::default());
    let (fava, _, _, _) = assembly(Arc::clone(&store), author.clone());
    let destination = relay("payloads");
    let unsigned = EventBuilder::new(Kind::TextNote)
        .content("unsigned")
        .by(author.public_key())
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

#[tokio::test(flavor = "current_thread")]
async fn builder_automatic_and_explicit_routing_are_exact_and_conflicts_have_no_effects() {
    let author = Keys::generate();
    let store = Arc::new(MemoryWriteStore::default());
    let (fava, signer, publisher, _) = assembly(Arc::clone(&store), author.clone());
    let embedded = relay("embedded");
    let builder = EventBuilder::new(Kind::TextNote)
        .content("builder route")
        .to_relays([embedded.clone()])
        .expect("embedded route validates")
        .by(author.public_key());

    let conflict = fava
        .to([relay("facade")])
        .expect("facade route validates")
        .publish(builder);
    assert!(matches!(
        conflict,
        Err(PublishError::Intent(
            WriteIntentError::ConflictingExplicitRoutes
        ))
    ));
    assert_no_effects(&store, &signer, &publisher);

    let automatic = fava
        .publish(
            EventBuilder::new(Kind::TextNote)
                .content("automatic builder route")
                .by(author.public_key()),
        )
        .expect("plain builder uses automatic routing");
    let automatic_receipt = automatic.receipt().expect("automatic receipt");
    assert_eq!(automatic_receipt.routing, WriteRouting::Automatic);
    assert_eq!(
        automatic_receipt.current.event.author(),
        author.public_key(),
        "Fava::publish on an AuthoredEventBuilder carries the builder's own author"
    );

    let write = fava
        .publish(
            EventBuilder::new(Kind::TextNote)
                .content("builder route")
                .to_relays([embedded.clone()])
                .expect("embedded route validates")
                .by(author.public_key()),
        )
        .expect("builder route publishes directly");
    assert_eq!(
        write.receipt().expect("receipt").routing,
        WriteRouting::Explicit(vec![embedded])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn publication_scopes_are_inert_before_valid_payload() {
    let author = Keys::generate();
    let store = Arc::new(MemoryWriteStore::default());
    let (fava, signer, publisher, _) = assembly(Arc::clone(&store), author.clone());

    drop(fava.with_account(author.public_key()));
    drop(fava.to([relay("dropped")]).expect("route validates"));
    assert_no_effects(&store, &signer, &publisher);

    match fava.to(Vec::<RelayUrl>::new()) {
        Err(PublishError::Intent(WriteIntentError::EmptyExplicitRelays)) => {}
        Err(error) => panic!("unexpected empty-route refusal: {error}"),
        Ok(scope) => {
            let event = EventBuilder::new(Kind::TextNote)
                .content("must never enter custody")
                .by(author.public_key())
                .build()
                .expect("deliberate-break payload builds");
            assert!(matches!(
                scope.publish(event),
                Err(PublishError::Intent(WriteIntentError::EmptyExplicitRelays))
            ));
        }
    }
    assert_no_effects(&store, &signer, &publisher);

    let too_many = fava.to((0..257)
        .map(|index| relay(&format!("bounded-{index}")))
        .collect::<Vec<_>>());
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

    let raw_too_many = fava.to(vec![relay("raw-bounded"); 1_025]);
    assert!(matches!(
        raw_too_many,
        Err(PublishError::Intent(
            WriteIntentError::TooManyRawExplicitRelays {
                actual: 1_025,
                maximum: 1_024
            }
        ))
    ));
    assert_no_effects(&store, &signer, &publisher);
}

fn assembly(
    store: Arc<MemoryWriteStore>,
    author: Keys,
) -> (Fava, Arc<CountingSigner>, Arc<RecordingPublisher>, Keys) {
    let cache = Arc::new(MemoryEventCache::default());
    let signer = Arc::new(CountingSigner::new(author));
    let second_author = Keys::generate();
    let second_signer = Arc::new(LocalSigner::new(second_author.clone()));
    let publisher = Arc::new(RecordingPublisher::default());
    let fava = Fava::builder()
        .event_cache(cache)
        .write_store(store)
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .signers([
            Arc::clone(&signer) as Arc<dyn Signer>,
            second_signer as Arc<dyn Signer>,
        ])
        .with_nip02()
        .publisher(Arc::clone(&publisher))
        .delivery_policy(Arc::new(
            fava_delivery_standard::StandardDeliveryPolicy::default(),
        ))
        .build()
        .expect("publication assembly");
    (fava, signer, publisher, second_author)
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
