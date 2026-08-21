use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use fava::{
    Event, EventBuilder, EventCoordinate, Fava, FavaBuilder, Kind, MaterializationId,
    ReplaceableEventEdit, ReplaceableEventMaterializer, Timestamp, UnsignedEvent, WriteIntent,
    WriteIntentError, WriteRouting,
};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_query_standard::StandardQueryEvaluator;
use fava_signer_local::LocalSigner;
use fava_state::{CacheMutation, CachedEvent, RelayEvidence};
use fava_transport::{RelaySession, Transport, TransportError};
use fava_write::{Receipt, ReceiptId};
use fava_write_store::WriteStore;
use fava_write_store_redb::RedbWriteStore;
use nostr::event::FinalizeEvent;
use nostr::key::Keys;

use super::{relay, session, unique_root, wait_for};

const SEMANTIC_BOUNDARY: &str = "FAVA_REDB_SEMANTIC_BOUNDARY";
const SEMANTIC_PATH: &str = "FAVA_REDB_SEMANTIC_PATH";
const SEMANTIC_MARKER: &str = "FAVA_REDB_SEMANTIC_MARKER";
const EDIT_FORMAT: u32 = 7;

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
        WriteIntent::edit(edit(), WriteRouting::Automatic).expect("automatic semantic intent")
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
            store
                .install_materialization(
                    accepted.write_id,
                    accepted.receipt_id,
                    MaterializationId::from_u64(1),
                    Some(base.id),
                    materialization(21, "generation two"),
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
}

#[tokio::test(flavor = "current_thread")]
async fn semantic_successor_and_failed_source_resume_once() {
    if env::var(SEMANTIC_BOUNDARY).is_ok() {
        return;
    }
    let successor_path = kill_at("successor");
    let successor_store = RedbWriteStore::open(successor_path).expect("successor store reopens");
    let successor = receipt_one(&successor_store);
    assert_eq!(
        successor.current.publication.materialization_id,
        MaterializationId::from_u64(2)
    );
    assert_eq!(successor.write_id.as_u64(), 1);
    assert_eq!(successor.receipt_id.as_u64(), 1);

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
    fava.wait_terminal(ReceiptId::from_u64(1))
        .await
        .expect("recovered publication settles");
    assert_eq!(materializer.calls(), 1);

    let second_materializer = Arc::new(TestMaterializer::new(Kind::ContactList));
    let _second = publication_builder(cache, store, Arc::clone(&second_materializer))
        .build()
        .expect("settled store reassembles");
    tokio::task::yield_now().await;
    assert_eq!(second_materializer.calls(), 0);
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
    let actor = keys().public_key();
    ReplaceableEventEdit::new(
        actor,
        EventCoordinate::Replaceable {
            author: actor,
            kind: Kind::ContactList,
            identifier: None,
        },
        EDIT_FORMAT,
        vec![1],
        vec![2],
    )
    .expect("semantic edit")
}

fn edit_intent() -> WriteIntent {
    WriteIntent::edit(edit(), WriteRouting::Explicit(BTreeSet::from([relay()])))
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
}

impl TestMaterializer {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            calls: AtomicU64::new(0),
        }
    }

    fn calls(&self) -> u64 {
        self.calls.load(Ordering::SeqCst)
    }
}

impl ReplaceableEventMaterializer for TestMaterializer {
    fn kind(&self) -> Kind {
        self.kind
    }

    fn supports(&self, edit: &ReplaceableEventEdit) -> bool {
        self.kind == Kind::ContactList && edit.format() == EDIT_FORMAT
    }

    fn materialize(
        &self,
        edit: &ReplaceableEventEdit,
        source: Option<&Event>,
        created_at: Timestamp,
    ) -> Result<UnsignedEvent, WriteIntentError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        EventBuilder::new(edit.actor(), Kind::ContactList)
            .created_at(created_at)
            .content(source.map_or("edit", |event| event.content.as_str()))
            .build()
            .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))
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

struct NoopTransport;

impl Transport for NoopTransport {
    fn open_session(
        &self,
        _key: fava_state::RelaySessionKey,
    ) -> Pin<Box<dyn Future<Output = Result<Arc<dyn RelaySession>, TransportError>> + Send + '_>>
    {
        Box::pin(async {
            Err(TransportError::ConnectionRefused(
                "publisher does not use transport".to_owned(),
            ))
        })
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
