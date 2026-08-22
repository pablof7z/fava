//! Public-facade evidence for pure multi-relay simple-group values.

use std::collections::BTreeSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava::{EventBuilder, Fava, Kind, Query, Tag, Timestamp, WriteRouting};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_query_standard::StandardQueryEvaluator;
use fava_routing::{RouteContribution, RoutePlan, RouteRequest, Router, RouterError, RouterSession};
use fava_signer::{Signer, SignerAvailability, SignerError};
use fava_simple_groups::{Group, GroupRecords};
use fava_state::{
    CacheMutation, CachedEvent, RelayAccess, RelayEvidence, RelaySessionKey, RelayUrl,
};
use fava_transport::{RelaySession, Transport, TransportError};
use fava_write::{Event, EventValue, PublicKey, UnsignedEvent};
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
use nostr::key::Keys;
use tokio::sync::watch;

#[tokio::test(flavor = "current_thread")]
async fn simple_group_content_preserves_local_visibility() {
    let keys = Keys::generate();
    let harness = Harness::new(Arc::new(ExactSigner::new(keys.clone())));
    let group = group();
    let query = group
        .events(
            Query::events()
                .limit(16)
                .expect("positive bound")
                .cache_only(),
        )
        .expect("group content query");
    let mut observation = harness.fava.observe(query).await.expect("query opens");
    assert!(observation.current().events.is_empty());

    let payload = EventBuilder::new(keys.public_key(), Kind::from_u16(9_007))
        .created_at(Timestamp::from(10))
        .content("accepted local content")
        .build()
        .expect("payload builds");
    let prepared = group.prepare(payload).expect("group context prepares");
    let id = prepared.id.expect("prepared id");
    let _write = harness
        .fava
        .to(group.hosts())
        .expect("exact hosts")
        .publish(prepared)
        .expect("local custody accepts");

    let snapshot = wait_for_snapshot(&mut observation, |current| !current.events.is_empty()).await;
    assert_eq!(snapshot.events.len(), 1);
    assert_eq!(snapshot.events[0].id(), id);
    assert!(snapshot.events[0].publication.is_some());
    assert!(snapshot.events[0].relay_evidence.is_empty());
    assert_eq!(harness.cache.len().expect("cache readable"), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn simple_group_records_require_actual_host_evidence() {
    let keys = Keys::generate();
    let harness = Harness::new(Arc::new(ExactSigner::new(keys.clone())));
    let group = group();
    let query = group
        .records(GroupRecords::all())
        .expect("record query")
        .cache_only();
    let mut observation = harness.fava.observe(query).await.expect("query opens");
    assert!(observation.current().events.is_empty());

    let local = signed_record(&Keys::generate(), 39_001, 10, "write-store only");
    let _write = harness
        .fava
        .to(group.hosts())
        .expect("exact hosts")
        .publish(local)
        .expect("local record custody accepts");
    tokio::task::yield_now().await;
    assert!(observation.current().events.is_empty());

    let served = signed_record(&keys, 39_000, 20, "served by A and B");
    for (host, observed_at) in [(host("a"), 21), (host("b"), 22)] {
        harness
            .cache
            .commit(vec![CacheMutation::Upsert(CachedEvent::new(
                served.clone(),
                evidence(host, observed_at),
            ))])
            .expect("relay evidence commits");
    }

    let snapshot = wait_for_snapshot(&mut observation, |current| {
        current.events.first().is_some_and(|record| record.relay_evidence.len() == 2)
    })
    .await;
    assert_eq!(snapshot.events.len(), 1);
    assert_eq!(snapshot.events[0].id(), served.id);
    let actual = snapshot.events[0]
        .relay_evidence
        .observations()
        .map(|observation| observation.session.relay.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, BTreeSet::from([host("a"), host("b")]));
    assert!(!actual.contains(&host("contacted-but-not-serving")));
}

#[tokio::test(flavor = "current_thread")]
async fn simple_group_arbitrary_kind_publication_uses_complete_exact_route() {
    let keys = Keys::generate();
    let signer = Arc::new(ExactSigner::new(keys.clone()));
    let harness = Harness::new(Arc::clone(&signer) as Arc<dyn Signer>);
    let group = Group::on(
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
    let prepared_once = group.prepare(payload.clone()).expect("first preparation");
    let prepared_twice = group.prepare(payload).expect("repeated preparation");

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

    let query = group
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
        .to(group.hosts())
        .expect("exact route")
        .publish(prepared_once)
        .expect("ordinary publication accepts");
    assert_ordinary_write(&write);
    let accepted = write.receipt().expect("accepted receipt");
    assert_eq!(accepted.routing, WriteRouting::Explicit(expected_hosts.clone()));

    wait_until(|| harness.publisher.attempts().len() == expected_hosts.len()).await;
    let receipt = write.receipt().expect("current receipt");
    assert_eq!(receipt.write_id, write.write_id());
    assert_eq!(receipt.receipt_id, write.receipt_id());
    assert_eq!(receipt.routing, WriteRouting::Explicit(expected_hosts.clone()));
    assert_eq!(receipt.desired_destinations.len(), expected_hosts.len());
    assert!(receipt.attempts.values().all(|attempts| *attempts == 1));
    let attempts = harness.publisher.attempts();
    let handed_off = attempts
        .iter()
        .map(|attempt| attempt.session.relay.clone())
        .collect::<Vec<_>>();
    for host in &expected_hosts {
        assert_eq!(handed_off.iter().filter(|actual| *actual == host).count(), 1);
    }
    assert!(attempts.iter().all(|attempt| {
        attempt.write_id == write.write_id()
            && attempt.receipt_id == write.receipt_id()
            && attempt.event.id == prepared_id
    }));
    let visible = wait_for_snapshot(&mut observation, |snapshot| {
        snapshot.events.iter().any(|record| record.id() == prepared_id)
    })
    .await;
    assert_eq!(visible.events.len(), 1);
    assert!(visible.events[0].publication.is_some());
}

fn assert_ordinary_write(_write: &fava::Write) {}

fn group() -> Group {
    Group::on(
        [host("a"), host("b"), host("contacted-but-not-serving")],
        "group-29",
    )
    .expect("group is valid")
}

fn host(name: &str) -> RelayUrl {
    RelayUrl::parse(&format!("wss://{name}.example")).expect("relay URL")
}

fn tag(cells: &[&str]) -> Tag {
    Tag::parse(cells.iter().copied()).expect("test tag")
}

fn signed_record(keys: &Keys, kind: u16, created_at: u64, content: &str) -> Event {
    NostrEventBuilder::new(Kind::from_u16(kind), content)
        .tags([tag(&["d", "group-29"])])
        .custom_created_at(Timestamp::from(created_at))
        .finalize(keys)
        .expect("record signs")
}

fn evidence(relay: RelayUrl, observed_at: u64) -> RelayEvidence {
    RelayEvidence::one(
        RelaySessionKey::new(relay, RelayAccess::public()),
        Timestamp::from(observed_at),
    )
}

async fn wait_for_snapshot(
    observation: &mut fava::Observation,
    predicate: impl Fn(&fava::QuerySnapshot) -> bool,
) -> Arc<fava::QuerySnapshot> {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let current = observation.current();
            if predicate(&current) {
                return current;
            }
            observation.changed().await.expect("observation remains open");
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
            .signers([signer])
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
        &self,
        event: UnsignedEvent,
        _cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + '_>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            event
                .finalize(&self.keys)
                .map_err(|error| SignerError::InvalidOutput(error.to_string()))
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
    fn name(&self) -> &str {
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
    fn open_session(
        &self,
        _key: RelaySessionKey,
    ) -> Pin<Box<dyn Future<Output = Result<Arc<dyn RelaySession>, TransportError>> + Send + '_>>
    {
        self.opens.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {
            Err(TransportError::ConnectionRefused(
                "spy transport must remain unopened".to_owned(),
            ))
        })
    }
}

async fn wait_until(predicate: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while !predicate() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("condition deadline");
}
