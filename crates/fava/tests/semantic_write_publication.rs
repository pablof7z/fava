//! Public-facade evidence for semantic materialization and publication.

use std::collections::BTreeSet;
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava::{
    Event, EventBuilder, EventCoordinate, EventValue, Fava, FavaBuilder, Kind, MaterializationId,
    PublicKey, ReceiptOutcome, RelayUrl, ReplaceableEventEdit, ReplaceableEventMaterializer,
    Timestamp, UnsignedEvent, WriteIntent, WriteIntentError, WriteRouting,
};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_query_standard::StandardQueryEvaluator;
use fava_signer::{Signer, SignerAvailability, SignerError};
use fava_signer_local::LocalSigner;
use fava_state::{CacheMutation, CachedEvent, RelayAccess, RelayEvidence, RelaySessionKey};
use fava_transport::{RelaySession, Transport, TransportError};
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent, Tag};
use nostr::key::Keys;
use tokio::sync::watch;

const EDIT_FORMAT: u32 = 7;

#[tokio::test(flavor = "current_thread")]
async fn first_value_edit_publishes_through_public_fava() {
    let keys = Keys::generate();
    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList, EDIT_FORMAT));
    let (fava, cache, store, signer, publisher) = assembly(
        Arc::new(MemoryWriteStore::default()),
        keys.clone(),
        vec![materializer.clone()],
    );
    let mut observation = fava
        .observe(
            fava::Query::events()
                .authors([keys.public_key()])
                .kind(Kind::ContactList)
                .cache_only(),
        )
        .await
        .expect("semantic query opens");

    let accepted = fava
        .publish(intent(keys.public_key(), Kind::ContactList, EDIT_FORMAT))
        .expect("first semantic value accepts");
    let visible = tokio::time::timeout(Duration::from_secs(1), observation.changed())
        .await
        .expect("local materialization arrives")
        .expect("observation stays open");
    let receipt = fava
        .wait_terminal(accepted.receipt_id)
        .await
        .expect("ordinary receipt settles");

    assert_eq!(accepted.write_id, receipt.write_id);
    assert_eq!(accepted.receipt_id, receipt.receipt_id);
    assert_eq!(receipt.outcome, ReceiptOutcome::Complete);
    assert_eq!(
        receipt.current.publication.materialization_id,
        MaterializationId::from_u64(1)
    );
    assert_eq!(visible.events.len(), 1);
    assert_eq!(visible.events[0].event.author(), keys.public_key());
    assert_eq!(visible.events[0].event.kind(), Kind::ContactList);
    assert_eq!(materializer.calls().len(), 1);
    assert!(materializer.calls()[0].source.is_none());
    assert_eq!(signer.calls(), 1);
    assert_eq!(publisher.attempts().len(), 1);
    assert_eq!(publisher.attempts()[0].receipt_id, accepted.receipt_id);
    assert!(cache.is_empty().expect("cache remains readable"));
    assert_eq!(store.len().expect("store remains readable"), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn materializer_selection_bounds_refuse_before_custody() {
    let keys = Keys::generate();

    let (empty, _, empty_store, empty_signer, empty_publisher) = assembly(
        Arc::new(MemoryWriteStore::default()),
        keys.clone(),
        Vec::new(),
    );
    assert!(
        empty
            .publish(intent(keys.public_key(), Kind::ContactList, EDIT_FORMAT))
            .is_err()
    );
    assert_no_effects(&empty_store, &empty_signer, &empty_publisher, 0);

    let selected = Arc::new(TestMaterializer::new(Kind::ContactList, EDIT_FORMAT));
    let (unsupported, _, unsupported_store, unsupported_signer, unsupported_publisher) = assembly(
        Arc::new(MemoryWriteStore::default()),
        keys.clone(),
        vec![selected],
    );
    assert!(
        unsupported
            .publish(intent(keys.public_key(), Kind::Custom(10_003), EDIT_FORMAT,))
            .is_err()
    );
    assert_no_effects(
        &unsupported_store,
        &unsupported_signer,
        &unsupported_publisher,
        0,
    );

    let duplicate = publication_builder(
        Arc::new(MemoryEventCache::default()),
        Arc::new(MemoryWriteStore::default()),
        Arc::new(CountingSigner::new(keys.clone())),
        Arc::new(RecordingPublisher::default()),
    )
    .materializers([
        Arc::new(TestMaterializer::new(Kind::ContactList, 1))
            as Arc<dyn ReplaceableEventMaterializer>,
        Arc::new(TestMaterializer::new(Kind::ContactList, 2)),
    ])
    .build();
    assert!(duplicate.is_err());

    let overflow = publication_builder(
        Arc::new(MemoryEventCache::default()),
        Arc::new(MemoryWriteStore::default()),
        Arc::new(CountingSigner::new(keys.clone())),
        Arc::new(RecordingPublisher::default()),
    )
    .materializers((0..65).map(|offset| {
        Arc::new(TestMaterializer::new(
            Kind::Custom(10_000 + offset),
            EDIT_FORMAT,
        )) as Arc<dyn ReplaceableEventMaterializer>
    }))
    .build();
    assert!(overflow.is_err());

    let bounded_store = Arc::new(MemoryWriteStore::bounded(NonZeroUsize::new(1).unwrap()));
    let (bounded, _, bounded_store, bounded_signer, bounded_publisher) = assembly(
        bounded_store,
        keys.clone(),
        vec![Arc::new(TestMaterializer::new(
            Kind::ContactList,
            EDIT_FORMAT,
        ))],
    );
    bounded
        .accept_event(EventValue::Unsigned(
            EventBuilder::new(keys.public_key(), Kind::TextNote)
                .created_at(Timestamp::from(1))
                .build()
                .unwrap(),
        ))
        .expect("one existing active write occupies capacity");
    assert!(
        bounded
            .publish(intent(keys.public_key(), Kind::ContactList, EDIT_FORMAT))
            .is_err()
    );
    assert_no_effects(&bounded_store, &bounded_signer, &bounded_publisher, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn first_value_receives_exact_injected_timestamp() {
    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let source = signed_source(
        &keys,
        Kind::ContactList,
        u64::MAX - 1,
        "remote base",
        &["remote"],
    );
    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            source.clone(),
            relay_evidence(),
        ))])
        .expect("source enters canonical cache");
    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList, EDIT_FORMAT));
    let (fava, _, _, _, publisher) = assembly_with_cache(
        cache,
        Arc::new(MemoryWriteStore::default()),
        keys,
        vec![materializer.clone()],
    );

    let accepted = fava
        .publish(intent(source.pubkey, Kind::ContactList, EDIT_FORMAT))
        .expect("source-backed edit accepts");
    let receipt = fava
        .wait_terminal(accepted.receipt_id)
        .await
        .expect("publication settles");
    let calls = materializer.calls();

    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].source.as_ref().map(|event| event.id),
        Some(source.id)
    );
    assert_eq!(calls[0].created_at, Timestamp::max());
    assert_eq!(receipt.current.event.created_at(), Timestamp::max());
    assert_eq!(publisher.attempts()[0].event.created_at, Timestamp::max());
}

fn intent(actor: PublicKey, kind: Kind, format: u32) -> WriteIntent {
    let coordinate = EventCoordinate::Replaceable {
        author: actor,
        kind,
        identifier: None,
    };
    let edit = ReplaceableEventEdit::new(actor, coordinate, format, vec![1], vec![2])
        .expect("bounded edit");
    WriteIntent::edit(edit, WriteRouting::Explicit(BTreeSet::from([relay_url()])))
        .expect("semantic intent validates")
}

fn assembly(
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

fn assembly_with_cache(
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

fn publication_builder(
    cache: Arc<MemoryEventCache>,
    store: Arc<MemoryWriteStore>,
    signer: Arc<CountingSigner>,
    publisher: Arc<RecordingPublisher>,
) -> FavaBuilder {
    Fava::builder()
        .event_cache(cache)
        .write_store(store)
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .signer(signer)
        .publisher(publisher)
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
}

fn assert_no_effects(
    store: &MemoryWriteStore,
    signer: &CountingSigner,
    publisher: &RecordingPublisher,
    expected_store_len: usize,
) {
    assert_eq!(store.len().expect("store readable"), expected_store_len);
    assert_eq!(signer.calls(), 0);
    assert!(publisher.attempts().is_empty());
}

#[derive(Clone)]
struct MaterializerCall {
    source: Option<Event>,
    created_at: Timestamp,
}

struct TestMaterializer {
    kind: Kind,
    format: u32,
    calls: Mutex<Vec<MaterializerCall>>,
}

impl TestMaterializer {
    fn new(kind: Kind, format: u32) -> Self {
        Self {
            kind,
            format,
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<MaterializerCall> {
        self.calls.lock().unwrap().clone()
    }
}

impl ReplaceableEventMaterializer for TestMaterializer {
    fn kind(&self) -> Kind {
        self.kind
    }

    fn supports(&self, edit: &ReplaceableEventEdit) -> bool {
        edit.format() == self.format
            && matches!(
                edit.coordinate(),
                EventCoordinate::Replaceable { kind, .. } if *kind == self.kind
            )
    }

    fn materialize(
        &self,
        edit: &ReplaceableEventEdit,
        source: Option<&Event>,
        created_at: Timestamp,
    ) -> Result<UnsignedEvent, WriteIntentError> {
        self.calls.lock().unwrap().push(MaterializerCall {
            source: source.cloned(),
            created_at,
        });
        let mut builder = EventBuilder::new(edit.actor(), self.kind)
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
        builder
            .build()
            .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))
    }
}

struct CountingSigner {
    inner: LocalSigner,
    calls: AtomicU64,
}

impl CountingSigner {
    fn new(keys: Keys) -> Self {
        Self {
            inner: LocalSigner::new(keys),
            calls: AtomicU64::new(0),
        }
    }

    fn calls(&self) -> u64 {
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

#[derive(Default)]
struct RecordingPublisher {
    attempts: Mutex<Vec<PublishAttempt>>,
}

impl RecordingPublisher {
    fn attempts(&self) -> Vec<PublishAttempt> {
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

struct NoopTransport;

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

fn signed_source(keys: &Keys, kind: Kind, created_at: u64, content: &str, tags: &[&str]) -> Event {
    let mut builder =
        NostrEventBuilder::new(kind, content).custom_created_at(Timestamp::from(created_at));
    for value in tags {
        builder = builder.tag(Tag::parse(["t", *value]).expect("test tag"));
    }
    builder.finalize(keys).expect("source signs")
}

fn relay_evidence() -> RelayEvidence {
    RelayEvidence::one(
        RelaySessionKey::new(relay_url(), RelayAccess::public()),
        Timestamp::from(1),
    )
}

fn relay_url() -> RelayUrl {
    RelayUrl::parse("wss://semantic.example").expect("relay url")
}
