use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava::{
    EditApplier, EditApplierSink, Event, EventBuilder, EventEdit, EventValue, Fava, FavaBuilder,
    Kind, PublicKey, Receipt, ReceiptId, RelayUrl, RevisionId, Timestamp, UnsignedEvent,
    WriteIntentError, WriteRouting,
};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_publication::Publication;
use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_query::{Query, QueryEvaluator, QuerySnapshot, QuerySource};
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_routing::{
    RouteContribution, RouteDestination, RoutePlan, RouteRequest, Router, RouterError,
    RouterSession,
};
use fava_session::Session;
use fava_signer::{Signer, SignerAvailability, SignerError};
use fava_signer_local::LocalSigner;
use fava_state::RelayEvent;
use fava_transport::{
    BoundedText, OpenRelaySession, RelaySessionFuture, Transport, TransportError, TransportFailure,
    TransportShutdownFuture,
};
use fava_write::WriteIntent;
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent, Tag};
use nostr::key::Keys;
use tokio::sync::watch;

pub fn intent(author: PublicKey, kind: Kind) -> WriteIntent {
    let edit = EventEdit::new(kind, None, vec![1]).expect("bounded edit");
    WriteIntent::edit_as(
        edit,
        author,
        WriteRouting::explicit([relay_url()]).expect("explicit route validates"),
    )
    .expect("semantic intent validates")
}

pub fn automatic_intent(author: PublicKey, kind: Kind) -> WriteIntent {
    let edit = EventEdit::new(kind, None, vec![1]).expect("bounded edit");
    WriteIntent::edit_as(edit, author, WriteRouting::Automatic).expect("semantic intent validates")
}

pub fn assembly(
    store: Arc<MemoryWriteStore>,
    keys: Keys,
    appliers: Vec<Arc<TestApplier>>,
) -> (
    Fava,
    Arc<MemoryEventCache>,
    Arc<MemoryWriteStore>,
    Arc<CountingSigner>,
    Arc<RecordingPublisher>,
) {
    assembly_with_cache(Arc::new(MemoryEventCache::default()), store, keys, appliers)
}

pub fn assembly_with_cache(
    cache: Arc<MemoryEventCache>,
    store: Arc<MemoryWriteStore>,
    keys: Keys,
    appliers: Vec<Arc<TestApplier>>,
) -> (
    Fava,
    Arc<MemoryEventCache>,
    Arc<MemoryWriteStore>,
    Arc<CountingSigner>,
    Arc<RecordingPublisher>,
) {
    let signer = Arc::new(CountingSigner::new(keys));
    let publisher = Arc::new(RecordingPublisher::default());
    let erased = appliers
        .into_iter()
        .map(|applier| applier as Arc<dyn EditApplier>);
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::clone(&publisher),
    )
    .appliers(erased)
    .build()
    .expect("semantic publication assembly");
    (fava, cache, store, signer, publisher)
}

pub fn publication_builder<S, W>(
    cache: Arc<MemoryEventCache>,
    store: Arc<W>,
    signer: Arc<S>,
    publisher: Arc<RecordingPublisher>,
) -> FavaBuilder
where
    S: Signer + 'static,
    W: WriteStore + 'static,
{
    Fava::builder()
        .event_cache(cache)
        .write_store(store)
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .signer(signer)
        .publisher(publisher)
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
}

pub fn publication_owner<S>(
    cache: Arc<MemoryEventCache>,
    store: Arc<MemoryWriteStore>,
    signer: Arc<S>,
    publisher: Arc<RecordingPublisher>,
    appliers: Vec<Arc<dyn EditApplier>>,
    routers: Vec<Arc<dyn Router>>,
) -> Publication
where
    S: Signer + 'static,
{
    let event_source: Arc<dyn QuerySource> = cache;
    let evaluator: Arc<dyn QueryEvaluator> = Arc::new(StandardQueryEvaluator);
    let signer: Arc<dyn Signer> = signer;
    let session = Session::new([signer]).expect("signer session");
    Publication::new(
        store,
        event_source,
        evaluator,
        appliers,
        session,
        publisher,
        Arc::new(StandardDeliveryPolicy::default()),
        Arc::new(NoopTransport),
        routers,
        None,
    )
    .expect("publication provider assembly")
}

pub fn assert_no_effects(
    store: &MemoryWriteStore,
    signer: &CountingSigner,
    publisher: &RecordingPublisher,
    expected_store_len: usize,
) {
    assert_eq!(
        fava_write_store::WriteStore::recover_open(store)
            .expect("store readable")
            .len(),
        expected_store_len
    );
    assert_eq!(signer.calls(), 0);
    assert!(publisher.attempts().is_empty());
}

#[derive(Clone)]
pub struct ApplierCall {
    pub author: PublicKey,
    pub identifier: Option<String>,
    pub source: Option<EventValue>,
    pub created_at: Timestamp,
}

pub struct TestApplier {
    kind: Kind,
    calls: Mutex<Vec<ApplierCall>>,
}

impl TestApplier {
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            calls: Mutex::new(Vec::new()),
        }
    }

    pub fn calls(&self) -> Vec<ApplierCall> {
        self.calls.lock().unwrap().clone()
    }
}

impl EditApplier for TestApplier {
    fn kind(&self) -> Kind {
        self.kind
    }

    fn supports(&self, edit: &EventEdit) -> bool {
        edit.kind() == self.kind
    }

    fn apply(
        &self,
        edit: &EventEdit,
        author: PublicKey,
        source: Option<&EventValue>,
        created_at: Timestamp,
    ) -> Result<UnsignedEvent, WriteIntentError> {
        self.calls.lock().unwrap().push(ApplierCall {
            author,
            identifier: edit.identifier().map(ToOwned::to_owned),
            source: source.cloned(),
            created_at,
        });
        let source_content = source.map(|source| match source {
            EventValue::Unsigned(event) => event.content.as_str(),
            EventValue::Signed(event) => event.content.as_str(),
        });
        let mut builder = EventBuilder::new(self.kind)
            .created_at(created_at)
            .content(
                source_content.map_or_else(|| "edit".to_owned(), |value| format!("{value}|edit")),
            )
            .by(author);
        if let Some(source) = source {
            for tag in source.tags().iter().cloned() {
                builder = builder.tag(tag);
            }
        }
        if let Some(identifier) = edit.identifier() {
            builder = builder.tag(Tag::identifier(identifier));
        }
        builder.build().map_err(WriteIntentError::from)
    }
}

/// A minimal [`EditApplierSink`] that captures the applier an enabling call
/// (`with_nip02()`, `with_bookmarks()`, ...) registers, without needing a
/// full `FavaBuilder`. Lets a test recover the real, otherwise-private
/// protocol applier through the public enabling call instead of substituting
/// a `TestApplier` for it.
#[derive(Default)]
pub struct CaptureSink {
    pub captured: Option<Arc<dyn EditApplier>>,
}

impl EditApplierSink for CaptureSink {
    fn accept(mut self, applier: Arc<dyn EditApplier>) -> Self {
        self.captured = Some(applier);
        self
    }
}

/// A plain function pointer mirroring `Enable`, but over [`CaptureSink`]
/// instead of `FavaBuilder`: every call site is a zero-capture closure like
/// `|sink| sink.with_nip02()`.
pub type Capture = fn(CaptureSink) -> CaptureSink;

/// Recover the real applier an enabling call registers.
pub fn captured_applier(capture: Capture) -> Arc<dyn EditApplier> {
    capture(CaptureSink::default())
        .captured
        .expect("enabling call registers exactly one applier")
}

pub struct CountingSigner {
    inner: Arc<LocalSigner>,
    calls: AtomicU64,
}

impl CountingSigner {
    pub fn new(keys: Keys) -> Self {
        Self {
            inner: Arc::new(LocalSigner::new(keys)),
            calls: AtomicU64::new(0),
        }
    }

    pub fn calls(&self) -> u64 {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Signer for CountingSigner {
    fn public_key(&self) -> PublicKey {
        self.inner.public_key()
    }

    fn availability(&self) -> SignerAvailability {
        self.inner.availability()
    }

    fn sign_event(
        self: Arc<Self>,
        event: UnsignedEvent,
        cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + 'static>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Arc::clone(&self.inner).sign_event(event, cancel)
    }
}

pub struct BlockingSigner {
    public_key: PublicKey,
    calls: AtomicU64,
}

#[allow(dead_code)] // Shared support: only failure/capability suites select this signer.
pub(super) struct UnavailableSigner {
    public_key: PublicKey,
}

#[allow(dead_code)]
impl UnavailableSigner {
    pub const fn new(public_key: PublicKey) -> Self {
        Self { public_key }
    }
}

impl Signer for UnavailableSigner {
    fn public_key(&self) -> PublicKey {
        self.public_key
    }

    fn availability(&self) -> SignerAvailability {
        SignerAvailability::Unavailable
    }

    fn sign_event(
        self: Arc<Self>,
        _event: UnsignedEvent,
        _cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + 'static>> {
        Box::pin(std::future::ready(Err(SignerError::Unavailable(
            "test signer parked".to_owned(),
        ))))
    }
}

pub(super) struct WindowSigner {
    keys: Keys,
    calls: Mutex<Vec<fava_write::EventId>>,
    permits: tokio::sync::Semaphore,
}

impl WindowSigner {
    pub fn new(keys: Keys) -> Self {
        Self {
            keys,
            calls: Mutex::new(Vec::new()),
            permits: tokio::sync::Semaphore::new(0),
        }
    }

    pub fn calls(&self) -> Vec<fava_write::EventId> {
        self.calls.lock().unwrap().clone()
    }

    pub fn release_one(&self) {
        self.permits.add_permits(1);
    }
}

impl Signer for WindowSigner {
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
        self.calls
            .lock()
            .unwrap()
            .push(event.id.expect("applied event id"));
        Box::pin(async move {
            let permit = self
                .permits
                .acquire()
                .await
                .map_err(|_| SignerError::Unavailable("signer window closed".to_owned()))?;
            permit.forget();
            event
                .finalize(&self.keys)
                .map_err(|error| SignerError::InvalidOutput(error.to_string()))
        })
    }
}

impl BlockingSigner {
    pub fn new(public_key: PublicKey) -> Self {
        Self {
            public_key,
            calls: AtomicU64::new(0),
        }
    }

    pub fn calls(&self) -> u64 {
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
        self: Arc<Self>,
        _event: UnsignedEvent,
        mut cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + 'static>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            let _ = cancel.changed().await;
            Err(SignerError::Cancelled)
        })
    }
}

#[derive(Default)]
pub struct RecordingPublisher {
    attempts: Mutex<Vec<PublishAttempt>>,
}

impl RecordingPublisher {
    pub fn attempts(&self) -> Vec<PublishAttempt> {
        self.attempts.lock().unwrap().clone()
    }
}

impl Publisher for RecordingPublisher {
    fn publish<'a>(
        &'a self,
        attempt: PublishAttempt,
        _transport: &'a dyn Transport,
    ) -> Pin<Box<dyn Future<Output = PublishOutcome> + Send + 'a>> {
        self.attempts.lock().unwrap().push(attempt);
        Box::pin(async {
            PublishOutcome::Acknowledged {
                message: "stored".to_owned(),
            }
        })
    }
}

pub struct CountingRouter {
    relay: RelayUrl,
    previews: AtomicU64,
    opens: AtomicU64,
}

impl CountingRouter {
    pub fn new(relay: RelayUrl) -> Self {
        Self {
            relay,
            previews: AtomicU64::new(0),
            opens: AtomicU64::new(0),
        }
    }

    pub fn previews(&self) -> u64 {
        self.previews.load(Ordering::SeqCst)
    }

    pub fn opens(&self) -> u64 {
        self.opens.load(Ordering::SeqCst)
    }

    fn contribution(&self, request: &RouteRequest) -> RouteContribution {
        RouteContribution {
            destinations: vec![RouteDestination::new(
                RelaySessionKey {
                    relay: self.relay.clone(),
                    access: RelayAccess::Public,
                },
                request.targets(),
                "semantic test route",
            )],
            coverage: BTreeMap::default(),
            unresolved: BTreeSet::default(),
            shortfalls: Vec::new(),
        }
    }
}

impl Router for CountingRouter {
    fn name(&self) -> &'static str {
        "semantic-test"
    }

    fn queries(&self, _: &RouteRequest, _: &RoutePlan) -> Result<Vec<Query>, RouterError> {
        Ok(Vec::new())
    }

    fn preview(
        &self,
        request: &RouteRequest,
        _upstream: &RoutePlan,
        _inputs: &[QuerySnapshot],
    ) -> Result<RouteContribution, RouterError> {
        self.previews.fetch_add(1, Ordering::SeqCst);
        Ok(self.contribution(request))
    }

    fn open(
        &self,
        request: RouteRequest,
        _upstream: Arc<RoutePlan>,
        _inputs: Vec<QuerySnapshot>,
    ) -> Result<Box<dyn RouterSession>, RouterError> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(StaticRouterSession(self.contribution(&request))))
    }
}

struct StaticRouterSession(RouteContribution);

impl RouterSession for StaticRouterSession {
    fn current(&self) -> RouteContribution {
        self.0.clone()
    }

    fn replace(
        &mut self,
        _: Arc<RoutePlan>,
        inputs: Vec<QuerySnapshot>,
    ) -> Result<RouteContribution, RouterError> {
        if inputs.is_empty() {
            Ok(self.current())
        } else {
            Err(RouterError::Refused("unexpected router input".to_owned()))
        }
    }

    fn close(&mut self) {}
}

pub struct NoopTransport;

impl Transport for NoopTransport {
    fn acquire_session(&self, request: OpenRelaySession) -> RelaySessionFuture<'_> {
        let _ = request;
        Box::pin(async {
            Err(TransportError::ConnectionRefused(
                TransportFailure::Disconnected {
                    detail: BoundedText::new("not used by recording publisher"),
                },
            ))
        })
    }

    fn authentication_requests(
        &self,
    ) -> tokio::sync::broadcast::Receiver<std::sync::Arc<dyn fava_transport::RelaySession>> {
        // This double never carries a relay's demand.
        tokio::sync::broadcast::Sender::new(1).subscribe()
    }

    fn holders(&self, _key: &RelaySessionKey) -> Option<NonZeroUsize> {
        None
    }

    fn shutdown(&self, _deadline: Duration) -> TransportShutdownFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}

pub fn signed_source(
    keys: &Keys,
    kind: Kind,
    created_at: u64,
    content: &str,
    tags: &[&str],
) -> Event {
    let mut builder =
        NostrEventBuilder::new(kind, content).custom_created_at(Timestamp::from(created_at));
    for value in tags {
        builder = builder.tag(Tag::parse(["t", *value]).expect("test tag"));
    }
    builder.finalize(keys).expect("source signs")
}

pub fn relay_session() -> RelaySessionKey {
    RelaySessionKey {
        relay: relay_url(),
        access: RelayAccess::Public,
    }
}

pub fn relay_occurrence() -> (RelaySessionKey, Timestamp) {
    (relay_session(), Timestamp::from(1))
}

pub fn relay_event(event: Event, occurrence: (RelaySessionKey, Timestamp)) -> RelayEvent {
    RelayEvent::new(event, occurrence.0, occurrence.1)
}

pub fn relay_url() -> RelayUrl {
    RelayUrl::parse("wss://semantic.example").expect("relay url")
}

pub async fn wait_for_revision(fava: &Fava, receipt_id: ReceiptId, generation: u64) -> Receipt {
    tokio::time::timeout(Duration::from_secs(1), async {
        let mut changes = fava.receipt_changes();
        loop {
            let receipt = fava
                .receipt(receipt_id)
                .expect("receipt read")
                .expect("receipt exists");
            if receipt.current.publication.revision_id
                == RevisionId::try_from(generation).expect("nonzero revision identity")
            {
                return receipt;
            }
            changes.recv().await.expect("receipt changes remain open");
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "revision {generation} did not advance: {:?}",
            fava.receipt(receipt_id)
        )
    })
}

pub async fn wait_for_signer(signer: &BlockingSigner, calls: u64) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while signer.calls() != calls {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("signer generation starts");
}

pub async fn assert_no_receipt_change(store: &MemoryWriteStore) {
    let mut changes = store.receipt_changes();
    assert!(
        tokio::time::timeout(Duration::from_millis(25), changes.recv())
            .await
            .is_err(),
        "inert source must not commit a receipt change"
    );
}
