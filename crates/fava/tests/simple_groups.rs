//! Public-facade evidence for pure multi-relay simple-group values.
//!
//! Cohesion: one facade target shares the same custody, signer, router, publisher,
//! and transport spies across query, publication, refusal, and lifecycle descriptors.

use std::num::NonZeroUsize;
use std::collections::BTreeSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava::{
    EventBuilder, Fava, Kind, MaterializationId, Query, ReceiptOutcome, Tag, Timestamp,
    WriteRouting,
};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_query_standard::StandardQueryEvaluator;
use fava_routing::{
    RouteContribution, RoutePlan, RouteRequest, Router, RouterError, RouterSession,
};
use fava_signer::{Signer, SignerAvailability, SignerError};
use fava_simple_groups::{Group, GroupRecords, SavedRelay, SimpleGroups};
use fava_state::{
    CacheMutation, CachedEvent, RelayAccess, RelayEvidence, RelaySessionKey, RelayUrl,
};
use fava_transport::{
    BoundedReason, OpenRelaySession, RelaySessionFuture, TransportError,
    TransportFailure, TransportShutdownFuture, Transport,
};
use fava_write::{Event, EventValue, PublicKey, UnsignedEvent};
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
use nostr::key::Keys;
use tokio::sync::watch;

include!("simple_groups/saved.rs");

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
        current
            .events
            .first()
            .is_some_and(|record| record.relay_evidence.len() == 2)
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
async fn simple_group_snapshot_deduplicates_content_with_exact_provenance() {
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
        .expect("content query");
    let mut observation = harness.fava.observe(query).await.expect("query opens");
    let unique_a = signed_group_event(&keys, 9, 10, "unique A", vec![]);
    let shared = signed_group_event(&keys, 9, 11, "shared", vec![]);
    let unique_b = signed_group_event(&keys, 9, 12, "unique B", vec![]);
    for (event, relay, observed_at) in [
        (unique_a.clone(), host("a"), 20),
        (shared.clone(), host("a"), 21),
        (shared.clone(), host("b"), 22),
        (unique_b.clone(), host("b"), 23),
    ] {
        harness
            .cache
            .commit(vec![CacheMutation::Upsert(CachedEvent::new(
                event,
                evidence(relay, observed_at),
            ))])
            .expect("event evidence commits");
    }
    let current = wait_for_snapshot(&mut observation, |snapshot| {
        snapshot.events.len() == 3
            && snapshot
                .events
                .iter()
                .any(|record| record.id() == shared.id && record.relay_evidence.len() == 2)
    })
    .await;
    let projected = group.project(&current).expect("bounded projection");

    assert_eq!(
        projected
            .events()
            .iter()
            .map(fava::EventRecord::id)
            .collect::<Vec<_>>(),
        [unique_b.id, shared.id, unique_a.id]
    );
    let shared_record = projected
        .events()
        .iter()
        .find(|record| record.id() == shared.id)
        .expect("shared id retained once");
    assert_eq!(
        shared_record
            .relay_evidence
            .observations()
            .map(|item| item.session.relay.clone())
            .collect::<Vec<_>>(),
        [host("a"), host("b")]
    );
    observation.close();
    observation.close();
    assert!(observation.changed().await.is_err());
}

#[tokio::test(flavor = "current_thread")]
async fn single_host_group_is_explicit_fork_choice() {
    let keys = Keys::generate();
    let harness = Harness::new(Arc::new(ExactSigner::new(keys.clone())));
    let multi = group();
    let query = multi
        .records(GroupRecords::metadata())
        .expect("record query")
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
    let a = Group::on([host("a")], "group-29")
        .expect("A group")
        .project(&current)
        .expect("bounded A projection");
    let b = Group::on([host("b")], "group-29")
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
    let multi = group();
    let query = multi
        .records(GroupRecords::metadata())
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
    let empty = Group::on([host("contacted-but-not-serving")], "group-29")
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
    let group = group();
    let keys = Keys::generate();
    let valid_signer = Arc::new(ExactSigner::new(keys.clone()));
    let valid = Harness::new(Arc::clone(&valid_signer) as Arc<dyn Signer>);
    let signed = NostrEventBuilder::new(Kind::from_u16(50_029), "signed exact bytes")
        .tags([
            tag(&["x", "before"]),
            tag(&["h", "group-29"]),
            tag(&["x", "after"]),
        ])
        .custom_created_at(Timestamp::from(88))
        .finalize(&keys)
        .expect("valid event signs");
    let original_bytes = serde_json::to_vec(&signed).expect("signed event encodes");
    let original_id = signed.id;
    let original_signature = signed.sig;
    let prepared = group.prepare(signed).expect("valid context passes purely");

    assert_eq!(serde_json::to_vec(&prepared).unwrap(), original_bytes);
    assert_eq!(prepared.id, original_id);
    assert_eq!(prepared.sig, original_signature);
    let _write = valid
        .fava
        .to(group.hosts())
        .expect("exact route")
        .publish(prepared)
        .expect("presigned custody accepts");
    wait_until(|| valid.publisher.attempts().len() == 3).await;
    assert_eq!(valid_signer.calls(), 0);
    assert!(valid.publisher.attempts().iter().all(|attempt| {
        serde_json::to_vec(&attempt.event).expect("attempt event encodes") == original_bytes
            && attempt.event.id == original_id
            && attempt.event.sig == original_signature
    }));

    let invalid_keys = Keys::generate();
    let invalid_signer = Arc::new(ExactSigner::new(invalid_keys.clone()));
    let invalid = Harness::new(Arc::clone(&invalid_signer) as Arc<dyn Signer>);
    let rows = [
        ("missing", Vec::new()),
        ("missing-value", vec![tag(&["h"])]),
        ("present-empty", vec![tag(&["h", ""])]),
        (
            "duplicate-adjacent",
            vec![tag(&["h", "group-29"]), tag(&["h", "group-29"])],
        ),
        ("contradictory", vec![tag(&["h", "other-group"])]),
    ];
    for (label, tags) in rows {
        let event = NostrEventBuilder::new(Kind::from_u16(50_029), label)
            .tags(tags)
            .custom_created_at(Timestamp::from(90))
            .finalize(&invalid_keys)
            .expect("hostile context still signs");
        let result = group.prepare(event);
        if let Ok(admitted) = result.as_ref() {
            let _ = invalid
                .fava
                .to(group.hosts())
                .expect("route remains valid")
                .publish(admitted.clone());
        }
        assert!(result.is_err(), "{label} must refuse before facade custody");
        assert_eq!(invalid.store.len().expect("store readable"), 0, "{label}");
        assert_eq!(invalid_signer.calls(), 0, "{label}");
        assert!(invalid.publisher.attempts().is_empty(), "{label}");
        assert_eq!(invalid.router.calls.load(Ordering::SeqCst), 0, "{label}");
        assert_eq!(invalid.transport.opens.load(Ordering::SeqCst), 0, "{label}");
        assert!(
            invalid
                .transport
                .frames
                .lock()
                .expect("frames lock")
                .is_empty()
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn simple_group_uses_ordinary_lifecycle_isolation() {
    let keys = Keys::generate();
    let signer = Arc::new(BlockingSigner::new(keys.public_key()));
    let harness = Harness::new(Arc::clone(&signer) as Arc<dyn Signer>);
    let group = group();
    let query = group
        .events(
            Query::events()
                .limit(8)
                .expect("positive bound")
                .cache_only(),
        )
        .expect("group content query");
    let mut observation = harness.fava.observe(query).await.expect("query opens");
    let first = group
        .prepare(
            EventBuilder::new(keys.public_key(), Kind::from_u16(50_029))
                .created_at(Timestamp::from(101))
                .content("first operation")
                .build()
                .expect("first builds"),
        )
        .expect("first prepares");
    let second = group
        .prepare(
            EventBuilder::new(keys.public_key(), Kind::from_u16(50_029))
                .created_at(Timestamp::from(102))
                .content("second operation")
                .build()
                .expect("second builds"),
        )
        .expect("second prepares");
    let first_id = first.id.expect("first id");
    let second_id = second.id.expect("second id");
    assert_ne!(first_id, second_id);
    let first_write = harness
        .fava
        .to(group.hosts())
        .expect("first route")
        .publish(first)
        .expect("first custody");
    let second_write = harness
        .fava
        .to(group.hosts())
        .expect("second route")
        .publish(second)
        .expect("second custody");
    assert_ne!(first_write.write_id(), second_write.write_id());
    assert_ne!(first_write.receipt_id(), second_write.receipt_id());
    wait_until(|| signer.calls() == 2).await;
    let both = wait_for_snapshot(&mut observation, |snapshot| snapshot.events.len() == 2).await;
    assert!(both.events.iter().any(|record| record.id() == first_id));
    assert!(both.events.iter().any(|record| record.id() == second_id));

    let cancelled = harness
        .fava
        .cancel_publication(first_write.receipt_id())
        .expect("first cancellation commits")
        .expect("first receipt exists");
    assert_eq!(cancelled.outcome, ReceiptOutcome::Cancelled);
    let remaining =
        wait_for_snapshot(&mut observation, |snapshot| snapshot.events.len() == 1).await;
    assert_eq!(remaining.events[0].id(), second_id);
    let second_receipt = second_write.receipt().expect("second remains readable");
    assert_eq!(second_receipt.outcome, ReceiptOutcome::Open);
    assert!(matches!(
        second_receipt.current.event,
        EventValue::Unsigned(_)
    ));
    assert!(harness.publisher.attempts().is_empty());

    harness
        .fava
        .cancel_publication(second_write.receipt_id())
        .expect("second cancellation commits");
    observation.close();
    observation.close();
    assert!(observation.changed().await.is_err());
}

fn signed_record(keys: &Keys, kind: u16, created_at: u64, content: &str) -> Event {
    NostrEventBuilder::new(Kind::from_u16(kind), content)
        .tags([tag(&["d", "group-29"])])
        .custom_created_at(Timestamp::from(created_at))
        .finalize(keys)
        .expect("record signs")
}

fn signed_group_event(
    keys: &Keys,
    kind: u16,
    created_at: u64,
    content: &str,
    rows: Vec<Tag>,
) -> Event {
    NostrEventBuilder::new(Kind::from_u16(kind), content)
        .tags(
            std::iter::once(tag(&[if kind >= 39_000 { "d" } else { "h" }, "group-29"])).chain(rows),
        )
        .custom_created_at(Timestamp::from(created_at))
        .finalize(keys)
        .expect("group event signs")
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
