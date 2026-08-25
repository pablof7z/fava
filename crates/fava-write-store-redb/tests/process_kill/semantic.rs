use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::future::Future;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava::{
    Event, EventBuilder, Fava, FavaBuilder, Kind, MaterializationId, ReplaceableEventEdit,
    ReplaceableEventMaterializer, Tag, Timestamp, UnsignedEvent,
};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_query_standard::StandardQueryEvaluator;
use fava_signer_local::LocalSigner;
use fava_state::{CacheMutation, CachedEvent, RelayEvidence, RelaySessionKey};
use fava_transport::{
    BoundedReason, OpenRelaySession, RelaySessionFuture, Transport, TransportError,
    TransportFailure, TransportShutdownFuture,
};
use fava_write::{Receipt, ReceiptId, WriteIntent, WriteIntentError, WriteRouting};
use fava_write_store::WriteStore;
use fava_write_store_redb::RedbWriteStore;
use nostr::event::FinalizeEvent;
use nostr::key::Keys;

use super::{relay, session, unique_root, wait_for};

const SEMANTIC_BOUNDARY: &str = "FAVA_REDB_SEMANTIC_BOUNDARY";
const SEMANTIC_PATH: &str = "FAVA_REDB_SEMANTIC_PATH";
const SEMANTIC_MARKER: &str = "FAVA_REDB_SEMANTIC_MARKER";

#[test]
fn semantic_boundary_child() {
    let Ok(boundary) = env::var(SEMANTIC_BOUNDARY) else {
        return;
    };
    let path = PathBuf::from(env::var(SEMANTIC_PATH).expect("semantic child database path"));
    let marker = PathBuf::from(env::var(SEMANTIC_MARKER).expect("semantic child marker path"));
    let store = RedbWriteStore::open(path).expect("semantic child store opens");
    let base = signed_source(10, "base");
    let intent = if boundary == "terminal" {
        WriteIntent::edit_as(edit(), keys().public_key(), WriteRouting::Automatic)
            .expect("automatic semantic intent")
    } else {
        edit_intent()
    };
    let accepted = store
        .accept_materialized_edit(
            intent,
            materialization(11, "generation one"),
            matches!(boundary.as_str(), "successor" | "failed" | "retired").then_some(&base),
        )
        .expect("semantic child acceptance commits");
    match boundary.as_str() {
        "first" => {}
        "successor" | "retired" => {
            let successor = signed_source(20, "successor source");
            let created_at = if boundary == "successor" { 100 } else { 21 };
            store
                .install_materialization(
                    accepted.write_id,
                    accepted.receipt_id,
                    MaterializationId::from_u64(1),
                    Some(base.id),
                    materialization(created_at, "generation two"),
                    Some(&successor),
                )
                .expect("semantic successor commits");
        }
        "failed" => {
            let failed = signed_source(20, "failed source");
            store
                .record_materialization_failure(
                    accepted.write_id,
                    accepted.receipt_id,
                    MaterializationId::from_u64(1),
                    Some(base.id),
                    Some(&failed),
                    "child materializer failure".to_owned(),
                )
                .expect("semantic failure commits");
        }
        "terminal" => {
            store
                .apply_route(
                    accepted.write_id,
                    accepted.receipt_id,
                    accepted.current.publication.materialization_id,
                    accepted.current.id(),
                    &fava::RoutePlan {
                        revision: 2,
                        destinations: BTreeMap::new(),
                        coverage: BTreeMap::new(),
                        unresolved: BTreeSet::new(),
                        shortfalls: Vec::new(),
                        settled: true,
                    },
                )
                .expect("semantic terminal state commits");
        }
        "cancelled" => {
            store
                .cancel(accepted.receipt_id)
                .expect("semantic cancellation commits");
        }
        other => panic!("unknown semantic boundary {other}"),
    }
    fs::write(marker, b"semantic-commit-durable").expect("semantic marker writes");
    loop {
        std::thread::park();
    }
}

#[test]
fn semantic_first_generation_survives_sigkill() {
    if env::var(SEMANTIC_BOUNDARY).is_ok() {
        return;
    }
    let path = kill_at("first");
    let store = RedbWriteStore::open(path).expect("semantic store reopens");
    let receipt = receipt_one(&store);
    assert_eq!(receipt.write_id.as_u64(), 1);
    assert_eq!(receipt.receipt_id.as_u64(), 1);
    assert_eq!(
        receipt.current.publication.materialization_id,
        MaterializationId::from_u64(1)
    );
    assert_eq!(store.recover_materialized_edits().unwrap().len(), 1);
    assert_eq!(
        store.recover_materialized_edits().unwrap()[0].2,
        keys().public_key()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn semantic_successor_and_failed_source_resume_once() {
    if env::var(SEMANTIC_BOUNDARY).is_ok() {
        return;
    }
    let successor_path = kill_at("successor");
    let successor_store =
        Arc::new(RedbWriteStore::open(successor_path).expect("successor store reopens"));
    let successor = receipt_one(&successor_store);
    assert_eq!(
        successor.current.publication.materialization_id,
        MaterializationId::from_u64(2)
    );
    assert_eq!(successor.write_id.as_u64(), 1);
    assert_eq!(successor.receipt_id.as_u64(), 1);
    let newer_source = signed_source(30, "newer post-kill source");
    let newer_source_id = newer_source.id;
    let successor_cache = Arc::new(MemoryEventCache::default());
    successor_cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            newer_source,
            relay_evidence(),
        ))])
        .expect("newer source enters canonical cache");
    let successor_materializer = Arc::new(TestMaterializer::new(Kind::ContactList));
    let successor_fava = publication_builder(
        Arc::clone(&successor_cache),
        Arc::clone(&successor_store),
        Arc::clone(&successor_materializer),
    )
    .build()
    .expect("successor recovery assembles after materializer validation");
    let resumed = wait_for_generation(&successor_fava, ReceiptId::from_u64(1), 3).await;
    assert_eq!(
        resumed.current.publication.materialization_source,
        Some(newer_source_id)
    );
    wait_terminal(&successor_fava, ReceiptId::from_u64(1)).await;
    assert_eq!(successor_materializer.calls(), 1);
    let inert_materializer = Arc::new(TestMaterializer::new(Kind::ContactList));
    let _inert = publication_builder(
        successor_cache,
        successor_store,
        Arc::clone(&inert_materializer),
    )
    .build()
    .expect("settled successor store reassembles");
    tokio::task::yield_now().await;
    assert_eq!(inert_materializer.calls(), 0);

    let failed_path = kill_at("failed");
    let store = Arc::new(RedbWriteStore::open(failed_path).expect("failed store reopens"));
    let failed = receipt_one(&store);
    assert!(failed.current.publication.materialization_failure.is_some());
    let failed_source = signed_source(20, "failed source");
    let cache = Arc::new(MemoryEventCache::default());
    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            failed_source,
            relay_evidence(),
        ))])
        .expect("failed source enters canonical cache");

    let unsupported = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::new(TestMaterializer::new(Kind::MuteList)),
    )
    .build();
    assert!(unsupported.is_err(), "unsupported durable edit assembled");

    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList));
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&materializer),
    )
    .build()
    .expect("selected materializer assembles before recovery");
    let recovered = wait_for_generation(&fava, ReceiptId::from_u64(1), 2).await;
    assert!(
        recovered
            .current
            .publication
            .materialization_failure
            .is_none()
    );
    wait_terminal(&fava, ReceiptId::from_u64(1)).await;
    assert_eq!(materializer.calls(), 1);

    let second_materializer = Arc::new(TestMaterializer::new(Kind::ContactList));
    let _second = publication_builder(cache, store, Arc::clone(&second_materializer))
        .build()
        .expect("settled store reassembles");
    tokio::task::yield_now().await;
    assert_eq!(second_materializer.calls(), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn semantic_builder_refusal_after_sigkill_preserves_every_existing_identity() {
    if env::var(SEMANTIC_BOUNDARY).is_ok() {
        return;
    }
    let path = kill_at("first");
    let store = Arc::new(RedbWriteStore::open(path).expect("semantic store reopens"));
    let before = receipt_one(&store);
    let cache = Arc::new(MemoryEventCache::default());
    let source = signed_source(20, "post-kill source");
    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            source,
            relay_evidence(),
        ))])
        .expect("post-kill source enters canonical cache");
    let materializer = Arc::new(TestMaterializer::with_tag_count(Kind::ContactList, 2_001));
    let fava = Fava::builder()
        .event_cache(cache)
        .write_store(Arc::clone(&store))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .signer(Arc::new(LocalSigner::new(keys())))
        .publisher(Arc::new(PendingPublisher))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .materializer(Arc::clone(&materializer))
        .build()
        .expect("recovery assembles with the selected materializer mode");

    let observed = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(error) = materializer.observed_error() {
                return error;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("recovery invokes the materializer");
    assert_eq!(
        observed,
        WriteIntentError::TooManyTags {
            actual: 2_001,
            maximum: 2_000,
        }
    );
    let after = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let receipt = receipt_one(&store);
            if receipt
                .current
                .publication
                .materialization_failure
                .is_some()
            {
                return receipt;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("typed refusal is recorded against the existing generation");

    assert_eq!(after.write_id, before.write_id);
    assert_eq!(after.receipt_id, before.receipt_id);
    assert_eq!(after.current.id(), before.current.id());
    assert_eq!(
        after.current.publication.materialization_id,
        before.current.publication.materialization_id
    );
    assert_eq!(
        after.current.publication.materialization_source,
        before.current.publication.materialization_source
    );
    assert_eq!(
        after.current.publication.retired_materializations,
        before.current.publication.retired_materializations
    );
    assert_eq!(
        after.current.publication.materialization_id,
        MaterializationId::from_u64(1),
        "failed rematerialization installed a successor generation"
    );
    drop(fava);
}

#[test]
fn semantic_retired_and_terminal_work_stays_inert_after_sigkill() {
    if env::var(SEMANTIC_BOUNDARY).is_ok() {
        return;
    }
    let retired_path = kill_at("retired");
    let retired_store = RedbWriteStore::open(retired_path).expect("retired store reopens");
    let before = receipt_one(&retired_store);
    let late_source = signed_source(30, "late retired source");
    assert!(
        retired_store
            .install_materialization(
                before.write_id,
                before.receipt_id,
                MaterializationId::from_u64(1),
                before.current.publication.materialization_source,
                materialization(31, "late retired completion"),
                Some(&late_source),
            )
            .is_err()
    );
    assert_eq!(receipt_one(&retired_store), before);

    for boundary in ["terminal", "cancelled"] {
        let path = kill_at(boundary);
        let store = RedbWriteStore::open(path).expect("terminal store reopens");
        let receipt = receipt_one(&store);
        assert!(receipt.is_terminal());
        assert!(store.recover_materialized_edits().unwrap().is_empty());
        assert!(
            store
                .record_materialization_failure(
                    receipt.write_id,
                    receipt.receipt_id,
                    receipt.current.publication.materialization_id,
                    receipt.current.publication.materialization_source,
                    None,
                    "late after process death".to_owned(),
                )
                .is_err()
        );
        assert_eq!(receipt_one(&store), receipt);
    }
}

fn kill_at(boundary: &str) -> PathBuf {
    let root = unique_root(&format!("semantic-{boundary}"));
    fs::create_dir_all(&root).expect("semantic boundary directory");
    let database = root.join("writes.redb");
    let marker = root.join("semantic-committed.marker");
    let mut child = Command::new(env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "semantic::semantic_boundary_child",
            "--nocapture",
        ])
        .env(SEMANTIC_BOUNDARY, boundary)
        .env(SEMANTIC_PATH, &database)
        .env(SEMANTIC_MARKER, &marker)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("semantic boundary child starts");
    wait_for(&marker, &mut child);
    child.kill().expect("semantic SIGKILL succeeds");
    let status = child.wait().expect("semantic child reaped");
    assert!(!status.success(), "{boundary} child must be hard-killed");
    database
}

fn receipt_one(store: &RedbWriteStore) -> Receipt {
    store
        .receipt(ReceiptId::from_u64(1))
        .expect("receipt read")
        .expect("receipt one survives")
}

fn edit() -> ReplaceableEventEdit {
    ReplaceableEventEdit::new(Kind::ContactList, None, vec![1]).expect("semantic edit")
}

fn edit_intent() -> WriteIntent {
    WriteIntent::edit_as(
        edit(),
        keys().public_key(),
        WriteRouting::explicit([relay()]).expect("explicit route validates"),
    )
    .expect("semantic intent")
}

fn materialization(created_at: u64, body: &str) -> UnsignedEvent {
    EventBuilder::new(keys().public_key(), Kind::ContactList)
        .created_at(Timestamp::from(created_at))
        .content(body)
        .build()
        .expect("semantic materialization")
}

fn signed_source(created_at: u64, body: &str) -> Event {
    materialization(created_at, body)
        .finalize(&keys())
        .expect("semantic source signs")
}

fn keys() -> Keys {
    Keys::parse("0000000000000000000000000000000000000000000000000000000000000001")
        .expect("fixed semantic key")
}

fn relay_evidence() -> RelayEvidence {
    RelayEvidence::one(session(), Timestamp::from(1))
}

struct TestMaterializer {
    kind: Kind,
    calls: AtomicU64,
    tag_count: usize,
    observed_error: Mutex<Option<WriteIntentError>>,
}

impl TestMaterializer {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            calls: AtomicU64::new(0),
            tag_count: 0,
            observed_error: Mutex::new(None),
        }
    }

    fn with_tag_count(kind: Kind, tag_count: usize) -> Self {
        Self {
            kind,
            calls: AtomicU64::new(0),
            tag_count,
            observed_error: Mutex::new(None),
        }
    }

    fn calls(&self) -> u64 {
        self.calls.load(Ordering::SeqCst)
    }

    fn observed_error(&self) -> Option<WriteIntentError> {
        self.observed_error.lock().unwrap().clone()
    }
}

impl ReplaceableEventMaterializer for TestMaterializer {
    fn kind(&self) -> Kind {
        self.kind
    }

    fn supports(&self, edit: &ReplaceableEventEdit) -> bool {
        self.kind == edit.kind()
    }

    fn materialize(
        &self,
        _edit: &ReplaceableEventEdit,
        author: fava::PublicKey,
        source: Option<&Event>,
        created_at: Timestamp,
    ) -> Result<UnsignedEvent, WriteIntentError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let result = EventBuilder::new(author, Kind::ContactList)
            .created_at(created_at)
            .content(source.map_or("edit", |event| event.content.as_str()))
            .tags((0..self.tag_count).map(|index| {
                Tag::parse(["x", &index.to_string()]).expect("ordinary materializer tag")
            }))
            .build()
            .map_err(WriteIntentError::from);
        if let Err(error) = &result {
            *self.observed_error.lock().unwrap() = Some(error.clone());
        }
        result
    }
}

fn publication_builder(
    cache: Arc<MemoryEventCache>,
    store: Arc<RedbWriteStore>,
    materializer: Arc<TestMaterializer>,
) -> FavaBuilder {
    Fava::builder()
        .event_cache(cache)
        .write_store(store)
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .signer(Arc::new(LocalSigner::new(keys())))
        .publisher(Arc::new(AcknowledgingPublisher))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .materializer(materializer)
}

struct AcknowledgingPublisher;

impl Publisher for AcknowledgingPublisher {
    fn publish<'a>(
        &'a self,
        _attempt: PublishAttempt,
        _transport: &'a dyn Transport,
    ) -> Pin<Box<dyn Future<Output = PublishOutcome> + Send + 'a>> {
        Box::pin(async {
            PublishOutcome::Acknowledged {
                message: "stored".to_owned(),
            }
        })
    }
}

struct PendingPublisher;

impl Publisher for PendingPublisher {
    fn publish<'a>(
        &'a self,
        _attempt: PublishAttempt,
        _transport: &'a dyn Transport,
    ) -> Pin<Box<dyn Future<Output = PublishOutcome> + Send + 'a>> {
        Box::pin(std::future::pending())
    }
}

struct NoopTransport;

impl Transport for NoopTransport {
    fn acquire_session(&self, request: OpenRelaySession) -> RelaySessionFuture<'_> {
        let _ = request;
        Box::pin(async {
            Err(TransportError::ConnectionRefused(
                TransportFailure::Disconnected {
                    detail: BoundedReason::new("publisher does not use transport"),
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

async fn wait_for_generation(fava: &Fava, receipt_id: ReceiptId, generation: u64) -> Receipt {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let receipt = fava
                .receipt(receipt_id)
                .expect("receipt read")
                .expect("receipt retained");
            if receipt.current.publication.materialization_id
                == MaterializationId::from_u64(generation)
            {
                return receipt;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("recovery advances once")
}

async fn wait_terminal(fava: &Fava, receipt_id: ReceiptId) -> Receipt {
    let mut changes = fava.receipt_changes();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(receipt) = fava.receipt(receipt_id).expect("receipt read")
                && receipt.is_terminal()
            {
                return receipt;
            }
            match changes.recv().await {
                Ok((changed_id, Some(receipt)))
                    if changed_id == receipt_id && receipt.is_terminal() =>
                {
                    return receipt;
                }
                Ok((changed_id, None)) if changed_id == receipt_id => {
                    panic!("recovered receipt removed before terminal state")
                }
                Ok(_) => {}
                Err(error) => panic!("receipt change delivery failed: {error}"),
            }
        }
    })
    .await
    .expect("recovered publication settles")
}
