//! Public-facade evidence for the simple-group value, ordinary observations, and writes.

use std::sync::Arc;
use std::time::Duration;

use fava::{EventBuilder, EventValue, Fava, Kind, Query, SingleLetterTag, Tag, Timestamp, all};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_signer_local::LocalSigner;
use fava_simple_groups::{
    SavedGroupList, SimpleGroup, SimpleGroupMetadata, SimpleGroupStateEventKind, save_simple_group,
    saved_group_list_materializer,
};
use fava_signer::{Signer, SignerAvailability, SignerError};
use fava_simple_groups::{SavedRelay, SimpleGroup, SimpleGroupRecords, SimpleGroups};
use fava_state::{
    CacheMutation, CachedEvent, RelayAccess, RelayEvidence, RelaySessionKey, RelayUrl,
};
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
use nostr::key::Keys;

#[allow(dead_code)]
#[path = "support/semantic_write.rs"]
mod support;

use support::{RecordingPublisher, publication_builder};

#[test]
fn group_content_composition_stays_exact_through_the_public_facade() {
    let group = group();
    let h = SingleLetterTag::from_char('h').expect("lowercase h");
    let query = Query::events()
        .tag_values(h, ["another-group", "group-29"])
        .and_then(|query| group.events(query))
        .expect("group query composition");
    assert_eq!(
        query.selection().tag_values.get(&h),
        Some(&std::collections::BTreeSet::from(["group-29".to_owned()])),
    );

    let disjoint = Query::events()
        .tag_values(h, ["another-group"])
        .and_then(|query| group.events(query))
        .expect("disjoint group composition is match-nothing");
    assert_eq!(
        disjoint.selection().tag_values.get(&h),
        Some(&std::collections::BTreeSet::new()),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn prepared_content_uses_the_ordinary_observation_and_write_doors() {
    let keys = Keys::generate();
    let (fava, cache) = assembly(&keys);
    let group = group();
    let query = group
        .events(Query::events().cache_only())
        .expect("group query");
    let mut observation = fava.observe(query).await.expect("query opens");

    let draft = EventBuilder::new(keys.public_key(), Kind::from_u16(9_007))
        .created_at(Timestamp::from(10))
        .content("local group content")
        .build()
        .expect("payload builds");
    let prepared = simple_group
        .prepare(payload)
        .expect("group context prepares");
    let id = prepared.id.expect("prepared id");
    let _write = fava
        .to(group.relays())
        .expect("exact relay route")
        .publish(prepared)
        .expect("ordinary custody accepts");

    let current = wait_for(&mut observation, |snapshot| {
        snapshot.events.iter().any(|record| record.id() == id)
    })
    .await;
    assert_eq!(current.events.len(), 1);
    assert!(current.events[0].publication.is_some());
    assert!(current.events[0].relay_evidence.is_empty());
    assert!(cache.event(id).expect("cache readable").is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn state_query_returns_generic_records_for_event_local_decoding() {
    let keys = Keys::generate();
    let (fava, cache) = assembly(&keys);
    let group = group();
    let query = group
        .state_events([SimpleGroupStateEventKind::Metadata])
        .expect("state query")
        .cache_only();
    let mut observation = harness.fava.observe(query).await.expect("query opens");
    let left = signed_group_event(&keys, 39_000, 20, "", vec![tag(&["name", "A"])]);
    let right = signed_group_event(&keys, 39_000, 30, "", vec![tag(&["name", "B"])]);
    for (event, relay, observed_at) in [(left, host("a"), 40), (right, host("b"), 41)] {
        harness
            .cache
            .commit(vec![CacheMutation::Upsert(CachedEvent::new(
                event,
                evidence(relay, observed_at),
            ))])
            .expect("fork commits");
    }
    let current = wait_for_snapshot(&mut observation, |snapshot| snapshot.events.len() == 2).await;
    let a = SimpleGroup::on([host("a")], "group-29")
        .expect("A group")
        .project(&current)
        .expect("bounded A projection");
    let b = SimpleGroup::on([host("b")], "group-29")
        .expect("B group")
        .project(&current)
        .expect("bounded B projection");

    assert_eq!(
        a.metadata().next().and_then(|(_, value)| value.name()),
        Some("A")
    );
    assert_eq!(
        b.metadata().next().and_then(|(_, value)| value.name()),
        Some("B")
    );
    assert!(!a.metadata_differ());
    assert!(!b.metadata_differ());
}

#[tokio::test(flavor = "current_thread")]
async fn single_host_empty_is_no_positive_evidence() {
    let keys = Keys::generate();
    let harness = Harness::new(Arc::new(ExactSigner::new(keys.clone())));
    let multi = simple_group();
    let query = multi
        .records(SimpleGroupRecords::metadata())
        .expect("record query")
        .cache_only();
    let mut observation = harness.fava.observe(query).await.expect("query opens");
    let metadata = signed_group_event(&keys, 39_000, 20, "", vec![tag(&["name", "A"])]);
    harness
        .cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            metadata,
            evidence(host("a"), 30),
        ))])
        .expect("record commits");
    let current = wait_for_snapshot(&mut observation, |snapshot| snapshot.events.len() == 1).await;
    let empty = SimpleGroup::on([host("contacted-but-not-serving")], "group-29")
        .expect("single host")
        .project(&current)
        .expect("bounded empty projection");

    assert!(empty.metadata().next().is_none());
    assert!(empty.admins().next().is_none());
    assert!(empty.members().next().is_none());
    assert!(empty.at(&host("contacted-but-not-serving")).is_some());
}

#[tokio::test(flavor = "current_thread")]
async fn simple_group_arbitrary_kind_publication_uses_complete_exact_route() {
    let keys = Keys::generate();
    let signer = Arc::new(ExactSigner::new(keys.clone()));
    let harness = Harness::new(Arc::clone(&signer) as Arc<dyn Signer>);
    let simple_group = SimpleGroup::on(
        [host("z"), host("a"), host("z"), host("m")],
        "publication-group",
    )
    .expect("group normalizes duplicate hosts");
    let expected_hosts = vec![host("z"), host("a"), host("m")];
    let payload = EventBuilder::new(keys.public_key(), Kind::from_u16(50_029))
        .created_at(Timestamp::from(50_029))
        .content("kind-blind payload")
        .build()
        .expect("payload builds");
    let prepared_once = simple_group
        .prepare(payload.clone())
        .expect("first preparation");
    let prepared_twice = simple_group.prepare(payload).expect("repeated preparation");

    assert_eq!(
        serde_json::to_vec(&prepared_once).expect("prepared event encodes"),
        serde_json::to_vec(&prepared_twice).expect("repeated event encodes")
    );
    assert_eq!(
        prepared_once
            .tags
            .iter()
            .filter(|tag| tag.as_slice().first().map(String::as_str) == Some("h"))
            .count(),
        1
    );
    assert_eq!(signer.calls(), 0);
    assert!(harness.publisher.attempts().is_empty());
    assert_eq!(harness.router.calls.load(Ordering::SeqCst), 0);
    assert_eq!(harness.store.len().expect("store readable"), 0);

    let query = simple_group
        .events(
            Query::events()
                .limit(8)
                .expect("positive bound")
                .cache_only(),
        )
        .expect("group content query");
    let mut observation = harness.fava.observe(query).await.expect("query opens");
    let prepared_id = prepared_once.id.expect("prepared id");
    let write = harness
        .fava
        .to(simple_group.hosts())
        .expect("exact route")
        .publish(prepared_once)
        .expect("ordinary publication accepts");
    assert_ordinary_write(&write);
    let accepted = write.receipt().expect("accepted receipt");
    assert_eq!(
        accepted.routing,
        WriteRouting::Explicit(expected_hosts.clone())
    );

    wait_until(|| harness.publisher.attempts().len() == expected_hosts.len()).await;
    let receipt = write.receipt().expect("current receipt");
    assert_eq!(receipt.write_id, write.write_id());
    assert_eq!(receipt.receipt_id, write.receipt_id());
    assert_eq!(
        receipt.routing,
        WriteRouting::Explicit(expected_hosts.clone())
    );
    assert_eq!(receipt.desired_destinations.len(), expected_hosts.len());
    assert!(receipt.attempts.values().all(|attempts| *attempts == 1));
    let attempts = harness.publisher.attempts();
    let handed_off = attempts
        .iter()
        .map(|attempt| attempt.session.relay.clone())
        .collect::<Vec<_>>();
    for host in &expected_hosts {
        assert_eq!(
            handed_off.iter().filter(|actual| *actual == host).count(),
            1
        );
    }
    assert!(attempts.iter().all(|attempt| {
        attempt.write_id == write.write_id()
            && attempt.receipt_id == write.receipt_id()
            && attempt.event.id == prepared_id
    }));
    let visible = wait_for_snapshot(&mut observation, |snapshot| {
        snapshot
            .events
            .iter()
            .any(|record| record.id() == prepared_id)
    })
    .await;
    assert_eq!(visible.events.len(), 1);
    assert!(visible.events[0].publication.is_some());
}

#[tokio::test(flavor = "current_thread")]
async fn simple_group_presigned_context_refuses_before_custody() {
    let simple_group = simple_group();
    let keys = Keys::generate();
    let valid_signer = Arc::new(ExactSigner::new(keys.clone()));
    let valid = Harness::new(Arc::clone(&valid_signer) as Arc<dyn Signer>);
    let signed = NostrEventBuilder::new(Kind::from_u16(50_029), "signed exact bytes")
        .tags([
            tag(&["d", "group-29"]),
            tag(&["name", "Facade group"]),
            tag(&["private"]),
        ])
        .custom_created_at(Timestamp::from(20))
        .finalize(&keys)
        .expect("valid event signs");
    let original_bytes = serde_json::to_vec(&signed).expect("signed event encodes");
    let original_id = signed.id;
    let original_signature = signed.sig;
    let prepared = simple_group
        .prepare(signed)
        .expect("valid context passes purely");

    let current = wait_for(&mut observation, |snapshot| !snapshot.events.is_empty()).await;
    let metadata = SimpleGroupMetadata::from_event(&current.events[0].event)
        .expect("ordinary event value decodes");
    assert_eq!(metadata.id(), "group-29");
    assert_eq!(metadata.name(), Some("Facade group"));
    assert!(metadata.is_private());
}

#[tokio::test(flavor = "current_thread")]
async fn saved_group_edit_materializes_through_the_ordinary_semantic_write_lifecycle() {
    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(MemoryWriteStore::default());
    let publisher = Arc::new(RecordingPublisher::default());
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::new(LocalSigner::new(keys.clone())),
        publisher,
    )
    .materializers([saved_group_list_materializer()])
    .build()
    .expect("facade assembly");
    let group = group();
    let edit = save_simple_group(&group, Some("Photos")).expect("bounded saved-group edit");
    let write = fava
        .by(keys.public_key())
        .to(group.relays())
        .expect("explicit route")
        .publish(edit)
        .expect("semantic custody accepts");
    let receipt = write.settled(all()).await.expect("write settles");

    assert!(matches!(receipt.current.event, EventValue::Signed(_)));
    let list =
        SavedGroupList::from_event(&receipt.current.event).expect("materialized list decodes");
    assert_eq!(list.author(), keys.public_key());
    assert_eq!(list.simple_groups().len(), group.relays().count());
    for (entry, relay) in list.simple_groups().iter().zip(group.relays()) {
        let saved = entry.as_ref().expect("saved group entry");
        assert_eq!(saved.id(), "group-29");
        assert_eq!(saved.display_name(), Some("Photos"));
        assert_eq!(saved.relay(), &relay);
    }
}

fn assembly(keys: &Keys) -> (Fava, Arc<MemoryEventCache>) {
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(MemoryWriteStore::default());
    let publisher = Arc::new(RecordingPublisher::default());
    let fava = publication_builder(
        Arc::clone(&cache),
        store,
        Arc::new(LocalSigner::new(keys.clone())),
        publisher,
    )
    .build()
    .expect("facade assembly");
    (fava, cache)
}

fn group() -> SimpleGroup {
    SimpleGroup::from_relays(
        "group-29",
        relay("a"),
        vec![relay("b"), relay("contacted-but-not-serving")],
    )
}

fn relay(name: &str) -> RelayUrl {
    RelayUrl::parse(&format!("wss://{name}.example")).expect("relay URL")
}

fn tag(values: &[&str]) -> Tag {
    Tag::parse(values.iter().copied()).expect("tag")
}

fn evidence(relay: RelayUrl, observed_at: u64) -> RelayEvidence {
    RelayEvidence::one(
        RelaySessionKey::new(relay, RelayAccess::public()),
        Timestamp::from(observed_at),
    )
}

async fn wait_for(
    observation: &mut fava::Observation,
    predicate: impl Fn(&fava::QuerySnapshot) -> bool,
) -> Arc<fava::QuerySnapshot> {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let current = observation.current();
            if predicate(&current) {
                return current;
            }
            observation
                .changed()
                .await
                .expect("observation remains open");
        }
    })
    .await
    .expect("snapshot deadline")
}

struct Harness {
    fava: Fava,
    cache: Arc<MemoryEventCache>,
    store: Arc<MemoryWriteStore>,
    publisher: Arc<SpyPublisher>,
    router: Arc<SpyRouter>,
    transport: Arc<SpyTransport>,
}

impl Harness {
    fn new(signer: Arc<dyn Signer>) -> Self {
        Self::new_with_materializers(signer, [])
    }

    fn new_with_materializers(
        signer: Arc<dyn Signer>,
        materializers: impl IntoIterator<Item = Arc<dyn fava::ReplaceableEventMaterializer>>,
    ) -> Self {
        Self::new_with_signers_and_materializers([signer], materializers)
    }

    fn new_with_signers_and_materializers(
        signers: impl IntoIterator<Item = Arc<dyn Signer>>,
        materializers: impl IntoIterator<Item = Arc<dyn fava::ReplaceableEventMaterializer>>,
    ) -> Self {
        let cache = Arc::new(MemoryEventCache::default());
        let store = Arc::new(MemoryWriteStore::default());
        let publisher = Arc::new(SpyPublisher::default());
        let router = Arc::new(SpyRouter::default());
        let transport = Arc::new(SpyTransport::default());
        let fava = Fava::builder()
            .event_cache(Arc::clone(&cache))
            .write_store(Arc::clone(&store))
            .query_evaluator(Arc::new(StandardQueryEvaluator))
            .transport(Arc::clone(&transport))
            .router(Arc::clone(&router))
            .signers(signers)
            .materializers(materializers)
            .publisher(Arc::clone(&publisher))
            .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
            .build()
            .expect("complete facade assembly");
        Self {
            fava,
            cache,
            store,
            publisher,
            router,
            transport,
        }
    }
}

struct ExactSigner {
    keys: Keys,
    calls: AtomicU64,
}

impl ExactSigner {
    fn new(keys: Keys) -> Self {
        Self {
            keys,
            calls: AtomicU64::new(0),
        }
    }

    fn calls(&self) -> u64 {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Signer for ExactSigner {
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
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            event
                .finalize(&self.keys)
                .map_err(|error| SignerError::InvalidOutput(error.to_string()))
        })
    }
}

struct BlockingSigner {
    public_key: PublicKey,
    calls: AtomicU64,
}

impl BlockingSigner {
    fn new(public_key: PublicKey) -> Self {
        Self {
            public_key,
            calls: AtomicU64::new(0),
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
struct SpyPublisher {
    attempts: Mutex<Vec<PublishAttempt>>,
}

impl SpyPublisher {
    fn attempts(&self) -> Vec<PublishAttempt> {
        self.attempts.lock().expect("publisher lock").clone()
    }
}

impl Publisher for SpyPublisher {
    fn publish<'a>(
        &'a self,
        attempt: PublishAttempt,
        _transport: &'a dyn Transport,
    ) -> Pin<Box<dyn Future<Output = PublishOutcome> + Send + 'a>> {
        self.attempts.lock().expect("publisher lock").push(attempt);
        Box::pin(async {
            PublishOutcome::Acknowledged {
                message: "accepted exactly".to_owned(),
            }
        })
    }
}

#[derive(Default)]
struct SpyRouter {
    calls: AtomicU64,
}

impl Router for SpyRouter {
    fn name(&self) -> &'static str {
        "spy-router"
    }

    fn preview(
        &self,
        _request: &RouteRequest,
        _upstream: &RoutePlan,
    ) -> Result<RouteContribution, RouterError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(RouterError::Refused("unexpected router preview".to_owned()))
    }

    fn open(
        &self,
        _request: RouteRequest,
        _upstream: watch::Receiver<Arc<RoutePlan>>,
    ) -> Result<Box<dyn RouterSession>, RouterError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(RouterError::Refused("unexpected router open".to_owned()))
    }
}

#[derive(Default)]
struct SpyTransport {
    opens: AtomicU64,
    frames: Mutex<Vec<String>>,
}

impl Transport for SpyTransport {
    fn acquire_session(&self, request: OpenRelaySession) -> RelaySessionFuture<'_> {
        let _ = request;
        self.opens.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {
            Err(TransportError::ConnectionRefused(
                TransportFailure::Disconnected {
                    detail: BoundedReason::new("spy transport must remain unopened"),
                },
            ))
        })
    }

    fn holders(&self, _key: &RelaySessionKey) -> Option<NonZeroUsize> {
        None
    }

    fn shutdown(&self, _deadline: Duration) -> TransportShutdownFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}
