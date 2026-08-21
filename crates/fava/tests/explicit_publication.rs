//! Public-facade evidence for durable explicit-route publication behavior.

use std::collections::BTreeSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava::{
    EventBuilder, EventValue, Fava, Query, ReceiptOutcome, RelayDeliveryOutcome, WriteIntent,
    WriteRouting,
};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_query_standard::StandardQueryEvaluator;
use fava_signer::{Signer, SignerAvailability, SignerError};
use fava_signer_local::LocalSigner;
use fava_state::{RelaySessionKey, RelayUrl};
use fava_transport::{RelaySession, Transport, TransportError};
use fava_write::{Event, Kind, PublicKey, UnsignedEvent};
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
use nostr::key::Keys;
use tokio::sync::watch;

#[tokio::test(flavor = "current_thread")]
async fn accepted_unsigned_event_is_visible_before_ok_and_cache_waits_for_echo() {
    let keys = Keys::generate();
    let relay = relay("accept");
    let publisher = Arc::new(GatedPublisher::new(PublishOutcome::Acknowledged {
        message: "stored".to_owned(),
    }));
    let (fava, cache) = assembly(
        Arc::new(MemoryWriteStore::default()),
        Arc::new(LocalSigner::new(keys.clone())),
        Arc::clone(&publisher),
    );
    let mut receipt_changes = fava.receipt_changes();
    let mut observation = fava
        .observe(Query::events().kind(Kind::TextNote).cache_only())
        .await
        .expect("query opens");
    let event = EventBuilder::new(keys.public_key(), Kind::TextNote)
        .content("optimistic")
        .build()
        .expect("event builds");
    let event_id = event.id.expect("event id");

    let accepted = fava
        .publish(
            WriteIntent::event(
                event,
                WriteRouting::Explicit(BTreeSet::from([relay.clone()])),
            )
            .expect("intent validates"),
        )
        .expect("acceptance commits");

    assert!(matches!(accepted.current.event, EventValue::Unsigned(_)));
    let accepted_receipt = receipt_changes
        .recv()
        .await
        .expect("acceptance transition delivered")
        .1
        .expect("acceptance is not removal");
    assert!(matches!(
        accepted_receipt.current.event,
        EventValue::Unsigned(_)
    ));
    let visible = observation.changed().await.expect("local write appears");
    assert_eq!(visible.events.len(), 1);
    assert!(visible.events[0].relay_evidence.is_empty());
    assert!(cache.event(event_id).expect("cache readable").is_none());
    wait_until(|| publisher.calls() == 1).await;
    let signed = receipt_changes
        .recv()
        .await
        .expect("signature transition delivered")
        .1
        .expect("signature is not removal");
    assert!(matches!(signed.current.event, EventValue::Signed(_)));
    let attempting = receipt_changes
        .recv()
        .await
        .expect("attempt transition delivered")
        .1
        .expect("attempt is not removal");
    assert!(
        attempting
            .destinations()
            .values()
            .all(|outcome| matches!(outcome, RelayDeliveryOutcome::Attempting))
    );
    publisher.release();
    let committed = receipt_changes
        .recv()
        .await
        .expect("outcome transition delivered")
        .1
        .expect("outcome is not removal");
    let receipt = fava
        .wait_terminal(accepted.receipt_id)
        .await
        .expect("receipt settles");
    assert_eq!(committed, receipt);
    assert_eq!(receipt.outcome, ReceiptOutcome::Complete);
    assert_eq!(
        receipt.destinations().values().next(),
        Some(&RelayDeliveryOutcome::Acknowledged {
            message: "stored".to_owned()
        })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn mixed_relay_results_remain_exact_under_one_terminal_receipt() {
    let keys = Keys::generate();
    let publisher = Arc::new(OutcomePublisher::default());
    let (fava, _) = assembly(
        Arc::new(MemoryWriteStore::default()),
        Arc::new(LocalSigner::new(keys.clone())),
        Arc::clone(&publisher),
    );
    let event = NostrEventBuilder::new(Kind::TextNote, "mixed")
        .finalize(&keys)
        .expect("event signs");
    let relays = BTreeSet::from([relay("accept"), relay("reject"), relay("unreachable")]);
    let accepted = fava
        .publish(WriteIntent::presigned(event, WriteRouting::Explicit(relays)).unwrap())
        .expect("acceptance commits");

    let receipt = fava
        .wait_terminal(accepted.receipt_id)
        .await
        .expect("mixed receipt settles");
    assert_eq!(receipt.outcome, ReceiptOutcome::Complete);
    assert!(receipt.destinations().values().any(|outcome| matches!(
        outcome,
        RelayDeliveryOutcome::Acknowledged { message } if message == "accepted exactly"
    )));
    assert!(receipt.destinations().values().any(|outcome| matches!(
        outcome,
        RelayDeliveryOutcome::Rejected { message } if message == "blocked exactly"
    )));
    assert!(receipt.destinations().values().any(|outcome| matches!(
        outcome,
        RelayDeliveryOutcome::GivenUp { reason } if reason.contains("connection refused exactly")
    )));
}

#[tokio::test(flavor = "current_thread")]
async fn pre_handoff_cancel_retracts_query_and_is_idempotent_and_removable() {
    let keys = Keys::generate();
    let signer = Arc::new(BlockingSigner::new(keys.public_key()));
    let publisher = Arc::new(OutcomePublisher::default());
    let (fava, _) = assembly(
        Arc::new(MemoryWriteStore::default()),
        Arc::clone(&signer),
        Arc::clone(&publisher),
    );
    let mut receipt_changes = fava.receipt_changes();
    let mut observation = fava
        .observe(Query::events().kind(Kind::TextNote).cache_only())
        .await
        .expect("query opens");
    let event = EventBuilder::new(keys.public_key(), Kind::TextNote)
        .content("cancel")
        .build()
        .unwrap();
    let accepted = fava
        .publish(
            WriteIntent::event(
                event,
                WriteRouting::Explicit(BTreeSet::from([relay("blocked")])),
            )
            .unwrap(),
        )
        .expect("acceptance commits");
    assert_eq!(receipt_changes.recv().await.unwrap().0, accepted.receipt_id);
    assert_eq!(observation.changed().await.unwrap().events.len(), 1);
    wait_until(|| signer.calls() == 1).await;

    let cancelled = fava
        .cancel_publication(accepted.receipt_id)
        .expect("cancellation commits")
        .expect("receipt exists");
    assert_eq!(cancelled.outcome, ReceiptOutcome::Cancelled);
    assert_eq!(
        receipt_changes.recv().await.unwrap(),
        (accepted.receipt_id, Some(cancelled.clone()))
    );
    assert!(observation.changed().await.unwrap().events.is_empty());
    assert_eq!(publisher.calls(), 0);
    assert_eq!(
        fava.cancel_publication(accepted.receipt_id)
            .unwrap()
            .unwrap()
            .outcome,
        ReceiptOutcome::Cancelled
    );
    assert!(fava.remove_receipt(accepted.receipt_id).unwrap());
    assert_eq!(
        receipt_changes.recv().await.unwrap(),
        (accepted.receipt_id, None)
    );
    assert!(fava.receipt(accepted.receipt_id).unwrap().is_none());
    assert!(
        fava.cancel_publication(accepted.receipt_id)
            .unwrap()
            .is_none()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn unsigned_write_without_its_author_signer_remains_inspectable() {
    let keys = Keys::generate();
    let fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .publisher(Arc::new(OutcomePublisher::default()))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .build()
        .unwrap();
    let event = EventBuilder::new(keys.public_key(), Kind::TextNote)
        .build()
        .unwrap();
    let accepted = fava
        .publish(
            WriteIntent::event(
                event,
                WriteRouting::Explicit(BTreeSet::from([relay("missing-signer")])),
            )
            .unwrap(),
        )
        .unwrap();
    tokio::task::yield_now().await;

    let open = fava.open_receipts().unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].receipt_id, accepted.receipt_id);
    assert!(matches!(open[0].current.event, EventValue::Unsigned(_)));
}

fn assembly<S, P>(
    store: Arc<MemoryWriteStore>,
    signer: Arc<S>,
    publisher: Arc<P>,
) -> (Fava, Arc<MemoryEventCache>)
where
    S: Signer + 'static,
    P: Publisher + 'static,
{
    let cache = Arc::new(MemoryEventCache::default());
    let fava = Fava::builder()
        .event_cache(Arc::clone(&cache))
        .write_store(store)
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .signer(signer)
        .publisher(publisher)
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .build()
        .expect("publication assembly");
    (fava, cache)
}

fn relay(name: &str) -> RelayUrl {
    RelayUrl::parse(&format!("wss://{name}.example")).unwrap()
}

async fn wait_until(predicate: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while !predicate() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("condition deadline elapsed");
}

struct NoopTransport;

impl Transport for NoopTransport {
    fn open_session(
        &self,
        _key: RelaySessionKey,
    ) -> Pin<Box<dyn Future<Output = Result<Arc<dyn RelaySession>, TransportError>> + Send + '_>>
    {
        Box::pin(async {
            Err(TransportError::ConnectionRefused(
                "not used by test publisher".to_owned(),
            ))
        })
    }
}

struct GatedPublisher {
    outcome: PublishOutcome,
    calls: AtomicU64,
    gate: watch::Sender<bool>,
}

impl GatedPublisher {
    fn new(outcome: PublishOutcome) -> Self {
        let (gate, _) = watch::channel(false);
        Self {
            outcome,
            calls: AtomicU64::new(0),
            gate,
        }
    }

    fn calls(&self) -> u64 {
        self.calls.load(Ordering::SeqCst)
    }

    fn release(&self) {
        self.gate.send_replace(true);
    }
}

impl Publisher for GatedPublisher {
    fn publish<'a>(
        &'a self,
        _attempt: PublishAttempt,
        _transport: &'a dyn Transport,
    ) -> Pin<Box<dyn Future<Output = PublishOutcome> + Send + 'a>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut gate = self.gate.subscribe();
        Box::pin(async move {
            if !*gate.borrow() {
                let _ = gate.changed().await;
            }
            self.outcome.clone()
        })
    }
}

#[derive(Default)]
struct OutcomePublisher {
    calls: AtomicU64,
}

impl OutcomePublisher {
    fn calls(&self) -> u64 {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Publisher for OutcomePublisher {
    fn publish<'a>(
        &'a self,
        attempt: PublishAttempt,
        _transport: &'a dyn Transport,
    ) -> Pin<Box<dyn Future<Output = PublishOutcome> + Send + 'a>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            let relay = attempt.session.relay.as_str();
            if relay.contains("accept") {
                PublishOutcome::Acknowledged {
                    message: "accepted exactly".to_owned(),
                }
            } else if relay.contains("reject") {
                PublishOutcome::Rejected {
                    message: "blocked exactly".to_owned(),
                }
            } else {
                PublishOutcome::NotHandedOff {
                    reason: "connection refused exactly".to_owned(),
                }
            }
        })
    }
}

struct BlockingSigner {
    public_key: PublicKey,
    calls: AtomicU64,
    retained: Mutex<Vec<UnsignedEvent>>,
}

impl BlockingSigner {
    fn new(public_key: PublicKey) -> Self {
        Self {
            public_key,
            calls: AtomicU64::new(0),
            retained: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> u64 {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Signer for BlockingSigner {
    fn public_key(&self) -> PublicKey {
        self.public_key
    }

    fn availability(&self) -> SignerAvailability {
        SignerAvailability::Available
    }

    fn sign_event(
        &self,
        event: UnsignedEvent,
        mut cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + '_>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.retained.lock().unwrap().push(event);
        Box::pin(async move {
            let _ = cancel.changed().await;
            Err(SignerError::Cancelled)
        })
    }
}
