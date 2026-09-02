//! Naming the account work runs as, separately from who signs the event.
//!
//! A relay authenticates a connection; an event carries its own signature.
//! Those need not name the same account, and publishing one account's event
//! over another's connection is an ordinary thing to want. The verb that used
//! to sit here asserted authorship, and a marker trait refused any payload that
//! already had an author -- which refused exactly that case.

use std::sync::Arc;

use fava::{EventBuilder, Fava, Kind, Query};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::Authority;
use fava_signer_local::LocalSigner;
use fava_subscriptions_no_grouping::planner;
use fava_transport_testkit::FakeTransport;
use fava_write_store_memory::MemoryWriteStore;
use nostr::key::Keys;
use nostr::types::RelayUrl;

/// Records the author of every event handed to it, which is the fact these
/// tests are about.
#[derive(Default)]
struct RecordingPublisher {
    authors: std::sync::Mutex<Vec<nostr::key::PublicKey>>,
}

impl RecordingPublisher {
    fn authors(&self) -> Vec<nostr::key::PublicKey> {
        self.authors.lock().expect("not poisoned").clone()
    }
}

impl fava_publisher::Publisher for RecordingPublisher {
    fn publish<'a>(
        &'a self,
        attempt: fava_publisher::PublishAttempt,
        _transport: &'a dyn fava_transport::Transport,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = fava_publisher::PublishOutcome> + Send + 'a>,
    > {
        self.authors
            .lock()
            .expect("not poisoned")
            .push(attempt.event.pubkey);
        Box::pin(async {
            fava_publisher::PublishOutcome::Acknowledged {
                message: String::new(),
            }
        })
    }
}

fn assemble(signers: Vec<Keys>) -> (Fava, Arc<FakeTransport>, Arc<RecordingPublisher>) {
    let transport = Arc::new(FakeTransport::new());
    let publisher = Arc::new(RecordingPublisher::default());
    let mut builder = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::clone(&transport))
        .publisher(Arc::clone(&publisher))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()));
    for keys in signers {
        builder = builder.signer(Arc::new(LocalSigner::new(keys)));
    }
    (
        builder.build().expect("assembly is complete"),
        transport,
        publisher,
    )
}

/// The case the deleted marker trait existed to refuse: Bob's event, Alice's
/// connection.
#[tokio::test(flavor = "current_thread")]
async fn one_account_publishes_another_accounts_event() {
    let alice = Keys::generate();
    let bob = Keys::generate();
    let (fava, _transport, publisher) = assemble(vec![alice.clone(), bob.clone()]);
    let relay = RelayUrl::parse("wss://shared.example").expect("relay URL");

    let write = fava
        .to(vec![relay])
        .expect("explicit route")
        .with_account(alice.public_key())
        .publish(
            EventBuilder::new(Kind::TextNote)
                .content("gm")
                .by(bob.public_key()),
        )
        .expect("an authored payload is accepted under another account");

    let _ = write;
    for _ in 0..256 {
        tokio::task::yield_now().await;
    }
    assert_eq!(
        publisher.authors(),
        vec![bob.public_key()],
        "the payload's own author survives a selection naming someone else"
    );
}

/// A payload that states no author takes the selected account.
#[tokio::test(flavor = "current_thread")]
async fn the_selected_account_authors_a_payload_that_states_none() {
    let alice = Keys::generate();
    let (fava, _transport, publisher) = assemble(vec![alice.clone()]);
    let relay = RelayUrl::parse("wss://shared.example").expect("relay URL");

    let write = fava
        .to(vec![relay])
        .expect("explicit route")
        .with_account(alice.public_key())
        .publish(EventBuilder::new(Kind::TextNote).content("gm"))
        .expect("an authorless payload takes the selected account");

    let _ = write;
    for _ in 0..256 {
        tokio::task::yield_now().await;
    }
    assert_eq!(publisher.authors(), vec![alice.public_key()]);
}

/// A payload with no author and nothing to fall back on is refused before any
/// durable custody is taken.
#[tokio::test(flavor = "current_thread")]
async fn nothing_to_author_with_is_refused_before_custody() {
    let (fava, _transport, _publisher) = assemble(Vec::new());
    let relay = RelayUrl::parse("wss://shared.example").expect("relay URL");

    let refusal = fava
        .to(vec![relay])
        .expect("explicit route")
        .publish(EventBuilder::new(Kind::TextNote).content("gm"))
        .expect_err("an authorless payload with no account is refused");

    assert!(
        format!("{refusal}").contains("author"),
        "the refusal names what is missing, got {refusal}"
    );
    assert!(
        fava.open_receipts().expect("receipts read back").is_empty(),
        "nothing was taken into custody"
    );
}

/// Reads and writes take the same door: one selection, both paths.
#[tokio::test(flavor = "current_thread")]
async fn one_selection_serves_a_read_and_a_write() {
    let alice = Keys::generate();
    let (fava, transport, publisher) = assemble(vec![alice.clone()]);
    let relay = RelayUrl::parse("wss://shared.example").expect("relay URL");

    let _observation = fava
        .with_account(alice.public_key())
        .observe(
            Query::events()
                .only_from_relays([relay.clone()])
                .expect("relay selection"),
        )
        .await
        .expect("live query opens");
    let _write = fava
        .to(vec![relay.clone()])
        .expect("explicit route")
        .with_account(alice.public_key())
        .publish(EventBuilder::new(Kind::TextNote).content("gm"))
        .expect("the write is accepted");
    for _ in 0..256 {
        tokio::task::yield_now().await;
    }

    assert!(
        transport
            .relay(&relay, &Authority::As(alice.public_key()))
            .is_some(),
        "the read ran over the selected account's session"
    );
    assert_eq!(
        publisher.authors(),
        vec![alice.public_key()],
        "the write ran as the selected account"
    );
}

/// One selection names the account for reads as well as writes.
#[tokio::test(flavor = "current_thread")]
async fn a_query_under_a_selection_uses_that_accounts_authority() {
    let alice = Keys::generate();
    let (fava, transport, _publisher) = assemble(vec![alice.clone()]);
    let relay = RelayUrl::parse("wss://shared.example").expect("relay URL");

    let query = Query::events()
        .only_from_relays([relay.clone()])
        .expect("relay selection")
        .with_relay_access(Authority::As(alice.public_key()));
    let _observation = fava.observe(query).await.expect("live query opens");
    for _ in 0..64 {
        tokio::task::yield_now().await;
    }

    assert!(
        transport
            .relay(&relay, &Authority::As(alice.public_key()))
            .is_some(),
        "the read ran over the selected account's session"
    );
}

/// A write accepted under an account routes to that account's sessions, not to
/// public ones.
///
/// `RouteRequest::access()` returned public for every write, so this was
/// unreachable however the application named its account.
#[tokio::test(flavor = "current_thread")]
async fn an_authenticated_write_routes_under_its_own_authority() {
    let alice = Keys::generate();
    let (fava, _transport, _publisher) = assemble(vec![alice.clone()]);
    let relay = RelayUrl::parse("wss://shared.example").expect("relay URL");

    let write = fava
        .to(vec![relay.clone()])
        .expect("explicit route")
        .with_account(alice.public_key())
        .publish(EventBuilder::new(Kind::TextNote).content("gm"))
        .expect("the write is accepted");

    let receipt = fava
        .receipt(write.receipt_id())
        .expect("the receipt reads back")
        .expect("the receipt exists");
    assert_eq!(
        receipt.access,
        Authority::As(alice.public_key()),
        "the write recorded the authority it was accepted under"
    );
    // A destination is a bare relay now: the receipt's authority is the only
    // record of what it was accepted under, so the destination set is simply
    // the explicit route, not a per-destination access to compare.
    assert_eq!(
        receipt.destinations().keys().collect::<Vec<_>>(),
        vec![&relay],
        "every destination is the explicit route named",
    );
}

/// A write with no selection stays public work, unchanged.
#[tokio::test(flavor = "current_thread")]
async fn a_write_with_no_selection_is_public_work() {
    let alice = Keys::generate();
    let (fava, _transport, _publisher) = assemble(vec![alice.clone()]);
    let relay = RelayUrl::parse("wss://shared.example").expect("relay URL");

    let write = fava
        .to(vec![relay])
        .expect("explicit route")
        .publish(
            EventBuilder::new(Kind::TextNote)
                .content("gm")
                .by(alice.public_key()),
        )
        .expect("the write is accepted");

    let receipt = fava
        .receipt(write.receipt_id())
        .expect("the receipt reads back")
        .expect("the receipt exists");
    assert_eq!(
        receipt.access,
        Authority::Unauthenticated,
        "a current account may author work without making the connection authenticated"
    );
}
