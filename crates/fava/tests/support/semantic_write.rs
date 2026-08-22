use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava::{
    Event, EventBuilder, Fava, FavaBuilder, Kind, MaterializationId, PublicKey, Receipt, ReceiptId,
    RelayUrl, ReplaceableEventEdit, ReplaceableEventMaterializer, Timestamp, UnsignedEvent,
    WriteIntentError, WriteRouting,
};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_query_standard::StandardQueryEvaluator;
use fava_routing::{
    RouteContribution, RouteDestination, RoutePlan, RouteRequest, Router, RouterError,
    RouterSession,
};
use fava_signer::{Signer, SignerAvailability, SignerError};
use fava_signer_local::LocalSigner;
use fava_state::{RelayAccess, RelayEvidence, RelaySessionKey};
use fava_transport::{RelaySession, Transport, TransportError};
use fava_write::WriteIntent;
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent, Tag};
use nostr::key::Keys;
use tokio::sync::watch;

pub fn intent(author: PublicKey, kind: Kind) -> WriteIntent {
    let edit = ReplaceableEventEdit::new(kind, None, vec![1]).expect("bounded edit");
    WriteIntent::edit_as(
        edit,
        author,
        WriteRouting::explicit([relay_url()]).expect("explicit route validates"),
    )
    .expect("semantic intent validates")
}

pub fn automatic_intent(author: PublicKey, kind: Kind) -> WriteIntent {
    let edit = ReplaceableEventEdit::new(kind, None, vec![1]).expect("bounded edit");
    WriteIntent::edit_as(edit, author, WriteRouting::Automatic).expect("semantic intent validates")
}

pub fn assembly(
    store: Arc<MemoryWriteStore>,
    keys: Keys,
    materializers: Vec<Arc<TestMaterializer>>,
) -> (
    Fava,
    Arc<MemoryEventCache>,
    Arc<MemoryWriteStore>,
    Arc<CountingSigner>,
    Arc<RecordingPublisher>,
) {
    assembly_with_cache(
        Arc::new(MemoryEventCache::default()),
        store,
        keys,
        materializers,
    )
}

pub fn assembly_with_cache(
    cache: Arc<MemoryEventCache>,
    store: Arc<MemoryWriteStore>,
    keys: Keys,
    materializers: Vec<Arc<TestMaterializer>>,
) -> (
    Fava,
    Arc<MemoryEventCache>,
    Arc<MemoryWriteStore>,
    Arc<CountingSigner>,
    Arc<RecordingPublisher>,
) {
    let signer = Arc::new(CountingSigner::new(keys));
    let publisher = Arc::new(RecordingPublisher::default());
    let erased = materializers
        .into_iter()
        .map(|materializer| materializer as Arc<dyn ReplaceableEventMaterializer>);
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::clone(&publisher),
    )
    .materializers(erased)
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
pub struct MaterializerCall {
    pub author: PublicKey,
    pub identifier: Option<String>,
    pub source: Option<Event>,
    pub created_at: Timestamp,
}

pub struct TestMaterializer {
    kind: Kind,
    calls: Mutex<Vec<MaterializerCall>>,
}

impl TestMaterializer {
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            calls: Mutex::new(Vec::new()),
        }
    }

    pub fn calls(&self) -> Vec<MaterializerCall> {
        self.calls.lock().unwrap().clone()
    }
}

impl ReplaceableEventMaterializer for TestMaterializer {
    fn kind(&self) -> Kind {
        self.kind
    }

    fn supports(&self, edit: &ReplaceableEventEdit) -> bool {
        edit.kind() == self.kind
    }

    fn materialize(
        &self,
        edit: &ReplaceableEventEdit,
        author: PublicKey,
        source: Option<&Event>,
        created_at: Timestamp,
    ) -> Result<UnsignedEvent, WriteIntentError> {
        self.calls.lock().unwrap().push(MaterializerCall {
            author,
            identifier: edit.identifier().map(ToOwned::to_owned),
            source: source.cloned(),
            created_at,
        });
        let mut builder = EventBuilder::new(author, self.kind)
            .created_at(created_at)
            .content(match source {
                Some(source) => format!("{}|edit", source.content),
                None => "edit".to_owned(),
            });
        if let Some(source) = source {
            for tag in source.tags.iter().cloned() {
                builder = builder.tag(tag);
            }
        }
        if let Some(identifier) = edit.identifier() {
            builder = builder.tag(Tag::identifier(identifier));
        }
        builder
            .build()
            .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))
    }
}

pub struct CountingSigner {
    inner: LocalSigner,
    calls: AtomicU64,
}

impl CountingSigner {
    pub fn new(keys: Keys) -> Self {
        Self {
            inner: LocalSigner::new(keys),
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
        &self,
        event: UnsignedEvent,
        cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + '_>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.inner.sign_event(event, cancel)
    }
}

pub struct BlockingSigner {
    public_key: PublicKey,
    calls: AtomicU64,
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
        &self,
        _event: UnsignedEvent,
        mut cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + '_>> {
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
                RelaySessionKey::new(self.relay.clone(), RelayAccess::public()),
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

    fn preview(
        &self,
        request: &RouteRequest,
        _upstream: &RoutePlan,
    ) -> Result<RouteContribution, RouterError> {
        self.previews.fetch_add(1, Ordering::SeqCst);
        Ok(self.contribution(request))
    }

    fn open(
        &self,
        request: RouteRequest,
        _upstream: watch::Receiver<Arc<RoutePlan>>,
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

    fn next_change(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<RouteContribution, RouterError>> + Send + '_>> {
        Box::pin(std::future::pending())
    }

    fn close(&mut self) {}
}

pub struct NoopTransport;

impl Transport for NoopTransport {
    fn open_session(
        &self,
        _key: RelaySessionKey,
    ) -> Pin<Box<dyn Future<Output = Result<Arc<dyn RelaySession>, TransportError>> + Send + '_>>
    {
        Box::pin(async {
            Err(TransportError::ConnectionRefused(
                "not used by recording publisher".to_owned(),
            ))
        })
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

pub fn relay_evidence() -> RelayEvidence {
    RelayEvidence::one(
        RelaySessionKey::new(relay_url(), RelayAccess::public()),
        Timestamp::from(1),
    )
}

pub fn relay_url() -> RelayUrl {
    RelayUrl::parse("wss://semantic.example").expect("relay url")
}

pub async fn wait_for_materialization(
    fava: &Fava,
    receipt_id: ReceiptId,
    generation: u64,
) -> Receipt {
    tokio::time::timeout(Duration::from_secs(1), async {
        let mut changes = fava.receipt_changes();
        loop {
            let receipt = fava
                .receipt(receipt_id)
                .expect("receipt read")
                .expect("receipt exists");
            if receipt.current.publication.materialization_id
                == MaterializationId::from_u64(generation)
            {
                return receipt;
            }
            changes.recv().await.expect("receipt changes remain open");
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "materialization {generation} did not advance: {:?}",
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
