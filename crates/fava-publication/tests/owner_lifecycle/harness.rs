//! Deterministic providers for publication owner-level evidence.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava_delivery::{DeliveryDecision, DeliveryFacts, DeliveryPolicy};
use fava_publication::Publication;
use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_routing::{
    RouteContribution, RoutePlan, RouteRequest, Router, RouterError, RouterSession,
};
use fava_session::Session;
use fava_signer::{Signer, SignerAvailability, SignerError};
use fava_transport::{
    BoundedReason, OpenRelaySession, RelaySessionFuture, Transport, TransportError,
    TransportFailure, TransportShutdownFuture,
};
use fava_write::{
    Event, EventBuilder, Kind, PublicKey, Receipt, ReceiptId, RelayDeliveryOutcome, UnsignedEvent,
    WriteIntent, WriteRouting,
};
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::FinalizeEvent;
use nostr::key::Keys;
use nostr::key::SecretKey;
use nostr::types::RelayUrl;
use tokio::sync::{Notify, watch};

/// Deterministic secret material for the single test author.
#[must_use]
pub fn author_keys() -> Keys {
    Keys::new(SecretKey::from_slice(&[7_u8; 32]).expect("constant secret key is valid"))
}

#[must_use]
pub fn author() -> PublicKey {
    author_keys().public_key()
}

#[must_use]
pub fn relay_url(url: &str) -> RelayUrl {
    RelayUrl::parse(url).expect("test relay url is valid")
}

#[must_use]
pub fn relay(url: &str) -> RelaySessionKey {
    RelaySessionKey {
        relay: relay_url(url),
        access: RelayAccess::Public,
    }
}

#[must_use]
pub fn unsigned_note() -> UnsignedEvent {
    EventBuilder::new(author(), Kind::TextNote)
        .content("owner-level evidence")
        .build()
        .expect("test note builds")
}

#[must_use]
pub fn signed_note() -> Event {
    unsigned_note()
        .finalize(&author_keys())
        .expect("test note signs")
}

/// One assembled publication owner plus the store it commits to.
pub(crate) struct Harness {
    pub publication: Publication,
    pub store: Arc<MemoryWriteStore>,
    pub session: Session,
}

/// Explicit provider selection for one owner assembly.
pub(crate) struct HarnessBuilder {
    signers: Vec<Arc<dyn Signer>>,
    publisher: Arc<dyn Publisher>,
    delivery: Arc<dyn DeliveryPolicy>,
    routers: Vec<Arc<dyn Router>>,
}

impl Default for HarnessBuilder {
    fn default() -> Self {
        Self {
            signers: Vec::new(),
            publisher: Arc::new(NeverPublisher),
            delivery: Arc::new(fava_delivery_standard::StandardDeliveryPolicy::default()),
            routers: Vec::new(),
        }
    }
}

impl HarnessBuilder {
    #[must_use]
    pub fn signer(mut self, signer: Arc<dyn Signer>) -> Self {
        self.signers.push(signer);
        self
    }

    #[must_use]
    pub fn publisher(mut self, publisher: Arc<dyn Publisher>) -> Self {
        self.publisher = publisher;
        self
    }

    #[must_use]
    pub fn delivery(mut self, delivery: Arc<dyn DeliveryPolicy>) -> Self {
        self.delivery = delivery;
        self
    }

    #[must_use]
    pub fn router(mut self, router: Arc<dyn Router>) -> Self {
        self.routers.push(router);
        self
    }

    #[must_use]
    pub fn build(self) -> Harness {
        let store = Arc::new(MemoryWriteStore::default());
        let session = Session::new(self.signers).expect("test signer attachments are unique");
        let publication = Publication::new(
            store.clone(),
            store.clone(),
            Arc::new(StandardQueryEvaluator),
            Vec::new(),
            session.clone(),
            self.publisher,
            self.delivery,
            Arc::new(RefusingTransport),
            self.routers,
        )
        .expect("test providers assemble");
        Harness {
            publication,
            store,
            session,
        }
    }
}

impl Harness {
    pub fn publish_unsigned(&self, routing: WriteRouting) -> ReceiptId {
        let intent = WriteIntent::event(unsigned_note(), routing).expect("test intent is valid");
        self.publication
            .accept(intent)
            .expect("acceptance commits")
            .receipt_id
    }

    pub fn publish_signed(&self, routing: WriteRouting) -> ReceiptId {
        let intent = WriteIntent::presigned(signed_note(), routing).expect("test intent is valid");
        self.publication
            .accept(intent)
            .expect("acceptance commits")
            .receipt_id
    }

    pub fn receipt(&self, receipt_id: ReceiptId) -> Receipt {
        self.store
            .receipt(receipt_id)
            .expect("store reads")
            .expect("receipt exists")
    }

    /// Await a receipt predicate under a bounded deadline.
    pub async fn until<F>(&self, receipt_id: ReceiptId, predicate: F) -> Option<Receipt>
    where
        F: Fn(&Receipt) -> bool,
    {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if let Ok(Some(receipt)) = self.store.receipt(receipt_id)
                    && predicate(&receipt)
                {
                    return receipt;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .ok()
    }
}

/// Router whose live `open` always refuses.
pub(crate) struct RefusingRouter;

impl Router for RefusingRouter {
    fn name(&self) -> &'static str {
        "refusing"
    }

    fn preview(
        &self,
        _request: &RouteRequest,
        _upstream: &RoutePlan,
    ) -> Result<RouteContribution, RouterError> {
        Ok(RouteContribution::default())
    }

    fn open(
        &self,
        _request: RouteRequest,
        _upstream: watch::Receiver<Arc<RoutePlan>>,
    ) -> Result<Box<dyn RouterSession>, RouterError> {
        Err(RouterError::Refused(
            "router refuses to open a live session".to_owned(),
        ))
    }
}

/// Signer that answers immediately and ignores cancellation.
pub(crate) struct ImmediateSigner {
    keys: Keys,
}

impl Default for ImmediateSigner {
    fn default() -> Self {
        Self {
            keys: author_keys(),
        }
    }
}

impl Signer for ImmediateSigner {
    fn public_key(&self) -> PublicKey {
        self.keys.public_key()
    }

    fn availability(&self) -> SignerAvailability {
        SignerAvailability::Available
    }

    fn sign_event(
        self: Arc<Self>,
        event: UnsignedEvent,
        _cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + 'static>> {
        Box::pin(async move {
            event
                .finalize(&self.keys)
                .map_err(|error| SignerError::InvalidOutput(error.to_string()))
        })
    }
}

/// Signer answering only after the test releases it, ignoring cancellation.
pub(crate) struct GatedSigner {
    keys: Keys,
    release: Notify,
    started: Notify,
}

impl Default for GatedSigner {
    fn default() -> Self {
        Self {
            keys: author_keys(),
            release: Notify::new(),
            started: Notify::new(),
        }
    }
}

impl GatedSigner {
    pub fn release(&self) {
        self.release.notify_waiters();
    }

    pub async fn started(&self) {
        self.started.notified().await;
    }
}

impl Signer for GatedSigner {
    fn public_key(&self) -> PublicKey {
        self.keys.public_key()
    }

    fn availability(&self) -> SignerAvailability {
        SignerAvailability::Available
    }

    fn sign_event(
        self: Arc<Self>,
        event: UnsignedEvent,
        _cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + 'static>> {
        Box::pin(async move {
            let waiting = self.release.notified();
            tokio::pin!(waiting);
            self.started.notify_waiters();
            waiting.await;
            event
                .finalize(&self.keys)
                .map_err(|error| SignerError::InvalidOutput(error.to_string()))
        })
    }
}

/// Publisher returning one scripted outcome for every attempt.
pub(crate) struct ScriptedPublisher {
    outcome: PublishOutcome,
    attempts: Mutex<u32>,
}

impl ScriptedPublisher {
    #[must_use]
    pub const fn new(outcome: PublishOutcome) -> Self {
        Self {
            outcome,
            attempts: Mutex::new(0),
        }
    }

    #[must_use]
    pub fn attempts(&self) -> u32 {
        *self
            .attempts
            .lock()
            .expect("attempt counter is not poisoned")
    }
}

impl Publisher for ScriptedPublisher {
    fn publish<'a>(
        &'a self,
        _attempt: PublishAttempt,
        _transport: &'a dyn Transport,
    ) -> Pin<Box<dyn Future<Output = PublishOutcome> + Send + 'a>> {
        Box::pin(async move {
            *self
                .attempts
                .lock()
                .expect("attempt counter is not poisoned") += 1;
            self.outcome.clone()
        })
    }
}

struct NeverPublisher;

impl Publisher for NeverPublisher {
    fn publish<'a>(
        &'a self,
        _attempt: PublishAttempt,
        _transport: &'a dyn Transport,
    ) -> Pin<Box<dyn Future<Output = PublishOutcome> + Send + 'a>> {
        Box::pin(async move {
            PublishOutcome::NotHandedOff {
                reason: "test publisher never hands off".to_owned(),
            }
        })
    }
}

/// Delivery policy recording every durable fact it was asked to decide.
#[derive(Default)]
pub(crate) struct RecordingPolicy {
    seen: Mutex<Vec<RelayDeliveryOutcome>>,
}

impl RecordingPolicy {
    #[must_use]
    pub fn seen(&self) -> Vec<RelayDeliveryOutcome> {
        self.seen
            .lock()
            .expect("policy log is not poisoned")
            .clone()
    }
}

impl DeliveryPolicy for RecordingPolicy {
    fn decide(&self, facts: DeliveryFacts<'_>) -> DeliveryDecision {
        self.seen
            .lock()
            .expect("policy log is not poisoned")
            .push(facts.outcome.clone());
        if facts.attempts == 0 && matches!(facts.outcome, RelayDeliveryOutcome::Pending) {
            DeliveryDecision::AttemptNow
        } else {
            DeliveryDecision::Settled
        }
    }
}

struct RefusingTransport;

impl Transport for RefusingTransport {
    fn acquire_session(&self, _request: OpenRelaySession) -> RelaySessionFuture<'_> {
        Box::pin(async move {
            Err(TransportError::ConnectionRefused(
                TransportFailure::Disconnected {
                    detail: BoundedReason::new("test transport opens no sessions"),
                },
            ))
        })
    }

    fn holders(&self, _key: &RelaySessionKey) -> Option<std::num::NonZeroUsize> {
        None
    }

    fn shutdown(&self, _deadline: Duration) -> TransportShutdownFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}
